//! Collector keys: the authenticated endpoint's credentials.
//!
//! A request carries `Authorization: Bearer sk_<kid>_<secret>`, where `kid`
//! is a public key id (16 lowercase hex characters) and `secret` 32 random
//! bytes as 64 lowercase hex characters. Keys are created outside the
//! collector, which only reads them from `collector_keys`
//! (`collector_keys.sql`): a row per key, holding `sha256` of the secret's
//! bytes and the org and workspace the key is for.
//!
//! Authenticating a request:
//! 1. parse the key, or 401
//! 2. look its `kid` up, in the cache and then the database
//! 3. 401 unless the row exists and `sha256(secret)` matches its hash
//!    (compared in constant time)
//! 4. 401 if the key is revoked or has expired
//! 5. the request's org and workspace are the key's
//!
//! Lookups are cached per `kid` for `COLLECTOR_KEYS_CACHE_SECS` (default
//! 30s) — misses too, so a flood of unknown keys doesn't reach the
//! database. Revoking a key therefore takes effect within that time; expiry
//! is checked against the clock on every request.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use subtle::ConstantTimeEq;
use tokio::sync::RwLock;

use crate::result::{AppError, Result};

const DATABASE_URL_ENV: &str = "COLLECTOR_KEYS_DATABASE_URL";
const CACHE_SECS_ENV: &str = "COLLECTOR_KEYS_CACHE_SECS";
const DEFAULT_CACHE: Duration = Duration::from_secs(30);
/// Cached lookups are dropped wholesale beyond this; plenty for real keys,
/// and a bound on what a stream of made-up kids can make us hold.
const MAX_CACHED: usize = 10_000;

const AUTHORIZATION_HEADER: &str = "authorization";
const KEY_PREFIX: &str = "sk_";
const KID_LEN: usize = 16;
const SECRET_LEN: usize = 32;

/// The org and workspace a key is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyScope {
    pub org_id: String,
    pub workspace_id: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct KeyRow {
    org_id: String,
    workspace_id: String,
    hashed_secret: Vec<u8>,
    expires_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
}

struct Cached {
    fetched: Instant,
    row: Option<KeyRow>,
}

pub struct Authenticator {
    pool: PgPool,
    ttl: Duration,
    cache: RwLock<HashMap<String, Cached>>,
}

impl Authenticator {
    /// From `COLLECTOR_KEYS_DATABASE_URL` (and `COLLECTOR_KEYS_CACHE_SECS`);
    /// `None` when the URL is unset, which leaves the authenticated endpoint
    /// unavailable. Connects lazily, so the collector starts even while the
    /// database is down.
    pub fn from_env() -> Result<Option<Self>> {
        let Some(url) = non_empty_env(DATABASE_URL_ENV) else {
            return Ok(None);
        };
        let ttl = match non_empty_env(CACHE_SECS_ENV) {
            None => DEFAULT_CACHE,
            Some(raw) => Duration::from_secs(raw.parse().map_err(|_| {
                AppError::Config(format!(
                    "`{CACHE_SECS_ENV}` must be a number of seconds, got `{raw}`"
                ))
            })?),
        };
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(3))
            .connect_lazy(&url)
            .map_err(|e| AppError::Config(format!("`{DATABASE_URL_ENV}`: {e}")))?;
        Ok(Some(Self::new(pool, ttl)))
    }

    pub fn new(pool: PgPool, ttl: Duration) -> Self {
        Self {
            pool,
            ttl,
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// The scope of the request's key, or why it has none.
    pub async fn authenticate(&self, headers: &axum::http::HeaderMap) -> Result<KeyScope> {
        let token = bearer_token(headers).ok_or(AppError::MissingCredentials)?;
        let (kid, secret) = parse_key(token).ok_or(AppError::InvalidToken)?;
        let row = self.lookup(kid).await?.ok_or(AppError::InvalidToken)?;
        verify(&row, &secret, Utc::now())?;
        Ok(KeyScope {
            org_id: row.org_id,
            workspace_id: row.workspace_id,
        })
    }

    /// Whether `collector_keys` can be read.
    pub async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1 FROM collector_keys LIMIT 1")
            .execute(&self.pool)
            .await
            .map(drop)
            .map_err(key_store_error)
    }

    async fn lookup(&self, kid: &str) -> Result<Option<KeyRow>> {
        if let Some(cached) = self.cache.read().await.get(kid)
            && cached.fetched.elapsed() < self.ttl
        {
            return Ok(cached.row.clone());
        }
        let row = sqlx::query_as::<_, KeyRow>(
            "SELECT org_id, workspace_id, hashed_secret, expires_at, revoked_at
             FROM collector_keys WHERE kid = $1",
        )
        .bind(kid)
        .fetch_optional(&self.pool)
        .await
        .map_err(key_store_error)?;

        let mut cache = self.cache.write().await;
        if cache.len() >= MAX_CACHED {
            cache.clear();
        }
        cache.insert(
            kid.to_owned(),
            Cached {
                fetched: Instant::now(),
                row: row.clone(),
            },
        );
        Ok(row)
    }
}

fn key_store_error(error: sqlx::Error) -> AppError {
    AppError::KeyStore(error.to_string())
}

/// The secret matches, and the key is neither revoked nor expired at `now`.
fn verify(row: &KeyRow, secret: &[u8; SECRET_LEN], now: DateTime<Utc>) -> Result<()> {
    let hash = Sha256::digest(secret);
    let matches: bool = hash.as_slice().ct_eq(&row.hashed_secret).into();
    if !matches {
        return Err(AppError::InvalidToken);
    }
    if row.revoked_at.is_some() || row.expires_at.is_some_and(|at| at <= now) {
        return Err(AppError::InvalidToken);
    }
    Ok(())
}

/// `sk_<kid>_<secret>` → the kid and the secret's bytes, if well-formed.
fn parse_key(token: &str) -> Option<(&str, [u8; SECRET_LEN])> {
    let rest = token.strip_prefix(KEY_PREFIX)?;
    let (kid, secret_hex) = rest.split_once('_')?;
    let lower_hex = |s: &str| {
        s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    if kid.len() != KID_LEN
        || !lower_hex(kid)
        || secret_hex.len() != SECRET_LEN * 2
        || !lower_hex(secret_hex)
    {
        return None;
    }
    let mut secret = [0u8; SECRET_LEN];
    hex::decode_to_slice(secret_hex, &mut secret).ok()?;
    Some((kid, secret))
}

fn bearer_token(headers: &axum::http::HeaderMap) -> Option<&str> {
    let value = headers.get(AUTHORIZATION_HEADER)?.to_str().ok()?.trim();
    let (scheme, token) = value.split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}

fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KID: &str = "0123456789abcdef";
    const SECRET_HEX: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    fn key() -> String {
        format!("sk_{KID}_{SECRET_HEX}")
    }

    fn row(secret: &[u8; SECRET_LEN]) -> KeyRow {
        KeyRow {
            org_id: "acme-retail".into(),
            workspace_id: "web-shop".into(),
            hashed_secret: Sha256::digest(secret).to_vec(),
            expires_at: None,
            revoked_at: None,
        }
    }

    fn headers(value: &str) -> axum::http::HeaderMap {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(AUTHORIZATION_HEADER, value.parse().unwrap());
        headers
    }

    #[test]
    fn parses_a_well_formed_key() {
        let key = key();
        let (kid, secret) = parse_key(&key).unwrap();
        assert_eq!(kid, KID);
        assert_eq!(hex::encode(secret), SECRET_HEX);
    }

    #[test]
    fn rejects_malformed_keys() {
        let short_secret = &SECRET_HEX[2..];
        for bad in [
            String::new(),
            format!("pk_{KID}_{SECRET_HEX}"),
            format!("sk{KID}_{SECRET_HEX}"),
            format!("sk_{KID}{SECRET_HEX}"),
            format!("sk_{}_{SECRET_HEX}", &KID[1..]),
            format!("sk_{KID}_{short_secret}"),
            format!("sk_{KID}_{SECRET_HEX}00"),
            format!("sk_{}_{SECRET_HEX}", KID.to_uppercase()),
            format!("sk_{KID}_{}", SECRET_HEX.to_uppercase()),
            format!("sk_{KID}_{}g", &SECRET_HEX[1..]),
            format!("sk_{KID}_{SECRET_HEX}_x"),
        ] {
            assert!(parse_key(&bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn reads_a_bearer_token_case_insensitively() {
        assert_eq!(bearer_token(&headers("Bearer abc")), Some("abc"));
        assert_eq!(bearer_token(&headers("bEaReR  abc ")), Some("abc"));
        assert_eq!(bearer_token(&headers("Basic abc")), None);
        assert_eq!(bearer_token(&headers("Bearer   ")), None);
        assert_eq!(bearer_token(&axum::http::HeaderMap::new()), None);
    }

    #[test]
    fn accepts_the_matching_secret() {
        let (_, secret) = parse_key(&key()).unwrap();
        assert!(verify(&row(&secret), &secret, Utc::now()).is_ok());
    }

    #[test]
    fn refuses_another_secret() {
        let (_, secret) = parse_key(&key()).unwrap();
        let mut other = secret;
        other[0] ^= 1;
        assert!(matches!(
            verify(&row(&secret), &other, Utc::now()),
            Err(AppError::InvalidToken)
        ));
    }

    #[test]
    fn refuses_revoked_and_expired_keys() {
        let (_, secret) = parse_key(&key()).unwrap();
        let now = Utc::now();

        let revoked = KeyRow {
            revoked_at: Some(now),
            ..row(&secret)
        };
        assert!(matches!(
            verify(&revoked, &secret, now),
            Err(AppError::InvalidToken)
        ));

        let expired = KeyRow {
            expires_at: Some(now),
            ..row(&secret)
        };
        assert!(matches!(
            verify(&expired, &secret, now),
            Err(AppError::InvalidToken)
        ));

        let expiring = KeyRow {
            expires_at: Some(now + chrono::Duration::hours(1)),
            ..row(&secret)
        };
        assert!(verify(&expiring, &secret, now).is_ok());
    }
}
