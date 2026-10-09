//! In-process stand-ins for the dashboard-server's external services:
//!
//! * a dummy OIDC provider serving the realm JWKS that
//!   `middleware::auth::get_decoding_key` fetches, plus helpers that mint
//!   RS256 access tokens shaped like Keycloak's, and
//! * a dummy ClickHouse admin endpoint that records every query and can be
//!   told to fail.
//!
//! RSA keys are generated per run, so no private key is ever committed.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::Context;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use rsa::RsaPrivateKey;
use rsa::pkcs1::{EncodeRsaPrivateKey, LineEnding};
use rsa::traits::PublicKeyParts;
use serde::Serialize;
use serde_json::json;

/// The Keycloak realm the dashboard-server is pointed at.
pub(crate) const REALM: &str = "e2e";

/// An OIDC user; `sub` is unique per instance, so each test starts from a
/// user the server has never seen.
pub(crate) struct TestUser {
    pub(crate) sub: String,
    pub(crate) email: String,
    pub(crate) name: String,
}

impl TestUser {
    pub(crate) fn new(name: &str) -> Self {
        let sub = uuid::Uuid::new_v4().to_string();
        Self {
            email: format!("{}@e2e.test", &sub[..8]),
            name: name.to_owned(),
            sub,
        }
    }
}

struct SigningKey {
    kid: String,
    encoding: EncodingKey,
    n: String,
    e: String,
}

impl SigningKey {
    fn generate(kid: &str) -> anyhow::Result<Self> {
        let key = RsaPrivateKey::new(&mut rand::thread_rng(), 2048)
            .context("failed to generate RSA key")?;
        let pem = key.to_pkcs1_pem(LineEnding::LF)?;
        Ok(Self {
            kid: kid.to_owned(),
            encoding: EncodingKey::from_rsa_pem(pem.as_bytes())?,
            n: URL_SAFE_NO_PAD.encode(key.n().to_bytes_be()),
            e: URL_SAFE_NO_PAD.encode(key.e().to_bytes_be()),
        })
    }
}

#[derive(Serialize)]
struct Claims<'a> {
    sub: &'a str,
    exp: usize,
    iat: usize,
    iss: &'a str,
    email: &'a str,
    preferred_username: &'a str,
    name: &'a str,
}

#[derive(Default)]
struct ClickHouse {
    queries: Mutex<Vec<String>>,
    fail: AtomicBool,
}

#[derive(Clone)]
struct MockState {
    jwks: Arc<serde_json::Value>,
    clickhouse: Arc<ClickHouse>,
}

pub(crate) struct Mock {
    base_url: String,
    key: SigningKey,
    /// Same `kid` as `key`, but never published: tokens it signs must fail.
    rogue_key: SigningKey,
    clickhouse: Arc<ClickHouse>,
}

impl Mock {
    pub(crate) async fn start() -> anyhow::Result<Self> {
        let key = SigningKey::generate("e2e-key")?;
        let rogue_key = SigningKey::generate("e2e-key")?;
        let jwks = json!({
            "keys": [{ "kid": key.kid, "kty": "RSA", "alg": "RS256", "use": "sig", "n": key.n, "e": key.e }]
        });
        let clickhouse = Arc::new(ClickHouse::default());
        let state = MockState {
            jwks: Arc::new(jwks),
            clickhouse: clickhouse.clone(),
        };

        let app = Router::new()
            .route("/realms/{realm}/protocol/openid-connect/certs", get(certs))
            .route("/clickhouse", post(clickhouse_query))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let base_url = format!("http://{}", listener.local_addr()?);
        tokio::spawn(async move { axum::serve(listener, app).await });

        Ok(Self {
            base_url,
            key,
            rogue_key,
            clickhouse,
        })
    }

    /// Base URL to use as the server's `KEYCLOAK_URL`.
    pub(crate) fn keycloak_url(&self) -> &str {
        &self.base_url
    }

    /// URL to use as the server's `CLICKHOUSE_ADMIN_URL`.
    pub(crate) fn clickhouse_url(&self) -> String {
        format!("{}/clickhouse", self.base_url)
    }

    pub(crate) fn issuer(&self) -> String {
        format!("{}/realms/{REALM}", self.base_url)
    }

    /// A valid token for `user`.
    pub(crate) fn token(&self, user: &TestUser) -> String {
        self.sign(&self.key, user, &self.issuer())
    }

    /// A token with the right `kid` but signed by an unpublished key.
    pub(crate) fn rogue_token(&self, user: &TestUser) -> String {
        self.sign(&self.rogue_key, user, &self.issuer())
    }

    /// A correctly signed token from a different issuer.
    pub(crate) fn token_with_issuer(&self, user: &TestUser, issuer: &str) -> String {
        self.sign(&self.key, user, issuer)
    }

    fn sign(&self, key: &SigningKey, user: &TestUser, issuer: &str) -> String {
        let now = chrono::Utc::now().timestamp() as usize;
        let claims = Claims {
            sub: &user.sub,
            exp: now + 300,
            iat: now,
            iss: issuer,
            email: &user.email,
            preferred_username: &user.email,
            name: &user.name,
        };
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(key.kid.clone());
        jsonwebtoken::encode(&header, &claims, &key.encoding).expect("failed to sign token")
    }

    /// Every query the dummy ClickHouse received since the last reset.
    pub(crate) fn clickhouse_queries(&self) -> Vec<String> {
        self.clickhouse.queries.lock().unwrap().clone()
    }

    pub(crate) fn fail_clickhouse(&self, fail: bool) {
        self.clickhouse.fail.store(fail, Ordering::SeqCst);
    }

    /// Clears recorded queries and failure mode; called before every case.
    pub(crate) fn reset(&self) {
        self.clickhouse.queries.lock().unwrap().clear();
        self.fail_clickhouse(false);
    }
}

async fn certs(State(state): State<MockState>, Path(realm): Path<String>) -> impl IntoResponse {
    if realm == REALM {
        Json((*state.jwks).clone()).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

async fn clickhouse_query(State(state): State<MockState>, body: String) -> impl IntoResponse {
    state.clickhouse.queries.lock().unwrap().push(body);
    if state.clickhouse.fail.load(Ordering::SeqCst) {
        (StatusCode::INTERNAL_SERVER_ERROR, "mock ClickHouse failure")
    } else {
        (StatusCode::OK, "")
    }
}
