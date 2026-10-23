//! API keys for `POST /v1/events/authenticated`.
//!
//! A request carries `Authorization: Bearer sk_<kid>_<secret>` (see
//! [`CollectorKey`]). The key's `collector_keys` row, created from the
//! dashboard, names its project; the request's org and project come from
//! that row, not from headers. The collector only reads `collector_keys`,
//! `projects` and `organizations`.
//!
//! Settings, from environment variables:
//!
//! - `DATABASE_URL`: the dashboard's Postgres; only `SELECT` is needed. While
//!   unset, the authenticated endpoint refuses every request with a
//!   configuration error.
//! - `KEY_CACHE_TTL_SECS`: how long a looked-up key (or its absence) is
//!   reused before it is read again; defaults to 60, `0` disables caching.
//!   A revoked key keeps working for up to this long; expiry is exact.

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use canonical_event::CollectorKey;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use subtle::ConstantTimeEq;

use crate::{Scope, result};

const DATABASE_URL_ENV: &str = "DATABASE_URL";
const KEY_CACHE_TTL_ENV: &str = "KEY_CACHE_TTL_SECS";
const DEFAULT_KEY_CACHE_TTL: Duration = Duration::from_secs(60);
/// Past this many cached lookups, expired ones are dropped, and if that is
/// not enough, the whole cache: requests with random kids cannot grow it
/// without bound.
const MAX_CACHED_KEYS: usize = 10_000;
const MAX_DB_CONNECTIONS: u32 = 5;
const DB_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);
const AUTHORIZATION_HEADER: &str = "authorization";
const BEARER_SCHEME: &str = "Bearer";

/// The org and project an authenticated request is for, from its key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyScope {
    pub org_id: String,
    pub project_id: String,
}

impl KeyScope {
    pub fn scope(&self) -> Scope<'_> {
        Scope {
            org_id: &self.org_id,
            project_id: &self.project_id,
        }
    }
}

/// What the collector needs from a key's row.
#[derive(Debug, Clone, sqlx::FromRow)]
struct KeyRow {
    org_id: String,
    project_id: String,
    hashed_secret: Vec<u8>,
    expires_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
}

struct CachedLookup {
    row: Option<KeyRow>,
    fetched_at: Instant,
}

/// Checks API keys against `collector_keys`, caching lookups by kid.
pub struct KeyStore {
    pool: Option<PgPool>,
    ttl: Duration,
    cache: RwLock<HashMap<String, CachedLookup>>,
}

impl KeyStore {
    /// A store on `DATABASE_URL` (see the module docs). The pool connects
    /// lazily, so the collector starts while the database is down; a
    /// malformed URL is an error.
    pub fn from_env() -> result::Result<Self> {
        let pool = non_empty_env(DATABASE_URL_ENV)
            .map(|url| {
                PgPoolOptions::new()
                    .max_connections(MAX_DB_CONNECTIONS)
                    .acquire_timeout(DB_ACQUIRE_TIMEOUT)
                    .connect_lazy(&url)
                    .map_err(|error| {
                        result::AppError::Config(format!(
                            "invalid `{}`: {}",
                            DATABASE_URL_ENV, error
                        ))
                    })
            })
            .transpose()?;
        let ttl = key_cache_ttl_from(non_empty_env(KEY_CACHE_TTL_ENV).as_deref())?;
        Ok(Self::new(pool, ttl))
    }

    fn new(pool: Option<PgPool>, ttl: Duration) -> Self {
        Self {
            pool,
            ttl,
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Authenticates the request's bearer key; returns the org and project
    /// it is for.
    pub async fn authenticate(&self, headers: &axum::http::HeaderMap) -> result::Result<KeyScope> {
        let token = extract_bearer_token(headers).ok_or(result::AppError::MissingCredentials)?;
        let key = CollectorKey::parse(&token).ok_or(result::AppError::InvalidToken)?;
        let row = self
            .lookup(key.kid())
            .await?
            .ok_or(result::AppError::InvalidToken)?;
        check_key(&key, &row, Utc::now())
    }

    /// `Ok(())` when keys can be looked up: `DATABASE_URL` is set and the
    /// database answers.
    pub async fn health_check(&self) -> result::Result<()> {
        sqlx::query("SELECT 1").execute(self.pool()?).await?;
        Ok(())
    }

    fn pool(&self) -> result::Result<&PgPool> {
        self.pool.as_ref().ok_or_else(|| {
            result::AppError::Config(format!(
                "`{}` environment variable must be set to authenticate API keys",
                DATABASE_URL_ENV
            ))
        })
    }

    /// The row of key `kid`, from the cache while fresh, else the database.
    /// `None` when there is no usable row: no such key, or its project or
    /// organization is deleted.
    async fn lookup(&self, kid: &str) -> result::Result<Option<KeyRow>> {
        if let Some(cached) = self.cache.read().unwrap().get(kid)
            && cached.fetched_at.elapsed() < self.ttl
        {
            return Ok(cached.row.clone());
        }

        let row = sqlx::query_as::<_, KeyRow>(
            r#"
            SELECT o.id AS org_id, k.project_id, k.hashed_secret, k.expires_at, k.revoked_at
            FROM collector_keys k
            JOIN projects p ON p.id = k.project_id AND p.deleted_at IS NULL
            JOIN organizations o ON o.id = p.organization_id AND o.deleted_at IS NULL
            WHERE k.kid = $1
            "#,
        )
        .bind(kid)
        .fetch_optional(self.pool()?)
        .await?;

        if !self.ttl.is_zero() {
            let mut cache = self.cache.write().unwrap();
            if cache.len() >= MAX_CACHED_KEYS {
                cache.retain(|_, cached| cached.fetched_at.elapsed() < self.ttl);
                if cache.len() >= MAX_CACHED_KEYS {
                    cache.clear();
                }
            }
            cache.insert(
                kid.to_owned(),
                CachedLookup {
                    row: row.clone(),
                    fetched_at: Instant::now(),
                },
            );
        }
        Ok(row)
    }
}

/// `key`'s scope when its secret matches `row` and the key is neither
/// revoked nor expired at `now`.
fn check_key(key: &CollectorKey, row: &KeyRow, now: DateTime<Utc>) -> result::Result<KeyScope> {
    let matches: bool = key
        .hashed_secret()
        .as_slice()
        .ct_eq(row.hashed_secret.as_slice())
        .into();
    if !matches || row.revoked_at.is_some() || row.expires_at.is_some_and(|at| at <= now) {
        return Err(result::AppError::InvalidToken);
    }
    Ok(KeyScope {
        org_id: row.org_id.clone(),
        project_id: row.project_id.clone(),
    })
}

fn extract_bearer_token(headers: &axum::http::HeaderMap) -> Option<String> {
    let value = headers.get(AUTHORIZATION_HEADER)?.to_str().ok()?;
    let (scheme, token) = value.trim().split_once(' ')?;
    if !scheme.eq_ignore_ascii_case(BEARER_SCHEME) {
        return None;
    }
    let token = token.trim();
    (!token.is_empty()).then(|| token.to_owned())
}

fn key_cache_ttl_from(raw: Option<&str>) -> result::Result<Duration> {
    let Some(raw) = raw else {
        return Ok(DEFAULT_KEY_CACHE_TTL);
    };
    raw.parse::<u64>().map(Duration::from_secs).map_err(|_| {
        result::AppError::Config(format!(
            "`{}` must be a whole number of seconds, got `{}`",
            KEY_CACHE_TTL_ENV, raw
        ))
    })
}

fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use canonical_event::collector_key::{KID_BYTES, SECRET_BYTES};

    use super::*;

    fn key() -> CollectorKey {
        CollectorKey::new([7; KID_BYTES], [42; SECRET_BYTES])
    }

    fn row_for(key: &CollectorKey) -> KeyRow {
        KeyRow {
            org_id: "acme-k3x9qa".to_owned(),
            project_id: "web-shop-9x2k1a".to_owned(),
            hashed_secret: key.hashed_secret().to_vec(),
            expires_at: None,
            revoked_at: None,
        }
    }

    fn headers(authorization: &str) -> axum::http::HeaderMap {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(AUTHORIZATION_HEADER, authorization.parse().unwrap());
        headers
    }

    #[test]
    fn extracts_bearer_token() {
        assert_eq!(
            extract_bearer_token(&headers("Bearer abc123")),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn bearer_token_can_be_empty() {
        assert_eq!(extract_bearer_token(&headers("Bearer   ")), None);
    }

    #[test]
    fn bearer_scheme_is_case_insensitive() {
        assert_eq!(
            extract_bearer_token(&headers("bEaReR abc123")),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn non_bearer_scheme_returns_none() {
        assert_eq!(extract_bearer_token(&headers("Basic dXNlcjpwYXNz")), None);
    }

    #[test]
    fn missing_authorization_header_returns_none() {
        assert_eq!(extract_bearer_token(&axum::http::HeaderMap::new()), None);
    }

    #[test]
    fn matching_key_resolves_its_project() {
        let key = key();
        assert_eq!(
            check_key(&key, &row_for(&key), Utc::now()).unwrap(),
            KeyScope {
                org_id: "acme-k3x9qa".to_owned(),
                project_id: "web-shop-9x2k1a".to_owned(),
            }
        );
    }

    #[test]
    fn wrong_secret_is_refused() {
        let other = CollectorKey::new([7; KID_BYTES], [43; SECRET_BYTES]);
        assert!(matches!(
            check_key(&other, &row_for(&key()), Utc::now()),
            Err(result::AppError::InvalidToken)
        ));
    }

    #[test]
    fn revoked_key_is_refused() {
        let key = key();
        let row = KeyRow {
            revoked_at: Some(Utc::now()),
            ..row_for(&key)
        };
        assert!(matches!(
            check_key(&key, &row, Utc::now()),
            Err(result::AppError::InvalidToken)
        ));
    }

    #[test]
    fn key_works_until_it_expires() {
        let key = key();
        let expires_at = Utc::now();
        let row = KeyRow {
            expires_at: Some(expires_at),
            ..row_for(&key)
        };
        assert!(check_key(&key, &row, expires_at - chrono::Duration::seconds(1)).is_ok());
        assert!(matches!(
            check_key(&key, &row, expires_at),
            Err(result::AppError::InvalidToken)
        ));
    }

    #[tokio::test]
    async fn malformed_key_is_refused_before_any_lookup() {
        // No database: reaching the lookup would be a configuration error.
        let store = KeyStore::new(None, DEFAULT_KEY_CACHE_TTL);
        for authorization in ["Bearer not-a-key", "Bearer sk_0707_2a2a"] {
            assert!(matches!(
                store.authenticate(&headers(authorization)).await,
                Err(result::AppError::InvalidToken)
            ));
        }
        assert!(matches!(
            store.authenticate(&axum::http::HeaderMap::new()).await,
            Err(result::AppError::MissingCredentials)
        ));
    }

    #[tokio::test]
    async fn without_database_url_keys_are_a_configuration_error() {
        let store = KeyStore::new(None, DEFAULT_KEY_CACHE_TTL);
        let bearer = format!("Bearer {}", key());
        assert!(matches!(
            store.authenticate(&headers(&bearer)).await,
            Err(result::AppError::Config(_))
        ));
        assert!(matches!(
            store.health_check().await,
            Err(result::AppError::Config(_))
        ));
    }

    #[test]
    fn key_cache_ttl_defaults_and_parses_seconds() {
        assert_eq!(key_cache_ttl_from(None).unwrap(), DEFAULT_KEY_CACHE_TTL);
        assert_eq!(key_cache_ttl_from(Some("0")).unwrap(), Duration::ZERO);
        assert_eq!(
            key_cache_ttl_from(Some("300")).unwrap(),
            Duration::from_secs(300)
        );
        assert!(matches!(
            key_cache_ttl_from(Some("1m")),
            Err(result::AppError::Config(_))
        ));
    }
}
