use std::net::SocketAddr;
use std::str;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use canonical_event::{WorkspaceId, is_valid_slug};
use event_collector::auth::{Authenticator, KeyScope};
use event_collector::config::{
    ServerConfig, get_config_from_local_file, get_default_config_from_local_file,
};
use event_collector::kafka_connector::test_connection;
use event_collector::result::AppError;
use event_collector::{
    CollectionStatus, Scope, collect_events, collect_events_authenticated, collect_events_batch,
};
use serde::Serialize;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

const ORG_ID_HEADER: &str = "x-org-id";
const WORKSPACE_ID_HEADER: &str = "x-workspace-id";
const FORWARDED_FOR_HEADER: &str = "x-forwarded-for";
const REAL_IP_HEADER: &str = "x-real-ip";
const ROUTE_PREFIX: &str = "/v1";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server_config = ServerConfig::from_env()?;
    let addr = server_config.addr;
    let state = Arc::new(AppState {
        auth: Authenticator::from_env()?,
    });
    let listener = TcpListener::bind(&addr).await?;

    println!("listening on http://{}", addr);
    axum::serve(
        listener,
        app(server_config.accept_cors, state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

struct AppState {
    /// The authenticated endpoint's key store; `None` when unconfigured,
    /// and the endpoint then answers 503.
    auth: Option<Authenticator>,
}

fn app(accept_cors: bool, state: Arc<AppState>) -> Router {
    let routes = Router::new()
        .route("/health", get(health))
        .route("/events/batch", post(push_batch))
        .route("/events", post(push_events))
        .route("/events/authenticated", post(push_events_authenticated))
        .with_state(state);

    let router = Router::new().nest(ROUTE_PREFIX, routes);

    if accept_cors {
        router.layer(CorsLayer::permissive().allow_private_network(true))
    } else {
        router
    }
}

async fn health(State(state): State<Arc<AppState>>) -> Result<Json<HealthResponse>, ApiError> {
    let config = get_default_config_from_local_file().await?;
    let kafka_connector = config.kafka_connector;

    tokio::task::spawn_blocking(move || test_connection(&kafka_connector))
        .await
        .map_err(|error| {
            ApiError::service_unavailable(format!("kafka health check failed to run: {}", error))
        })?
        .map_err(|error| {
            ApiError::service_unavailable(format!("kafka connection check failed: {}", error))
        })?;

    let authenticated_endpoint = match &state.auth {
        None => AppError::AuthNotConfigured.to_string(),
        Some(auth) => match auth.health_check().await {
            Ok(()) => "ok".to_string(),
            Err(error) => error.to_string(),
        },
    };

    Ok(Json(HealthResponse {
        status: "ok",
        authenticated_endpoint,
    }))
}

async fn push_batch(
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<CollectionStatus>, ApiError> {
    let context = RequestContext::from_headers(&headers, Some(peer_addr))?;
    let config =
        get_config_from_local_file(context.org_id.clone(), context.workspace_id.clone()).await?;
    let body = request_body_as_str(&body)?;
    let status = collect_events_batch(
        body,
        &config,
        &context.scope(),
        context.ip_address.as_deref(),
    )
    .await?;

    Ok(Json(status))
}

async fn push_events(
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<CollectionStatus>, ApiError> {
    let context = RequestContext::from_headers(&headers, Some(peer_addr))?;
    let config =
        get_config_from_local_file(context.org_id.clone(), context.workspace_id.clone()).await?;
    let body = request_body_as_str(&body)?;
    let status = collect_events(
        body,
        &config,
        &context.scope(),
        context.ip_address.as_deref(),
    )
    .await?;

    Ok(Json(status))
}

/// Like `push_events`, but the request is scoped by its collector key
/// (`Authorization: Bearer sk_…`), and its events are marked authenticated.
async fn push_events_authenticated(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<CollectionStatus>, ApiError> {
    let auth = state.auth.as_ref().ok_or(AppError::AuthNotConfigured)?;
    let key = auth.authenticate(&headers).await?;
    let context = RequestContext::for_key(&headers, Some(peer_addr), key)?;
    let config =
        get_config_from_local_file(context.org_id.clone(), context.workspace_id.clone()).await?;
    let body = request_body_as_str(&body)?;
    let status = collect_events_authenticated(
        body,
        &config,
        &context.scope(),
        context.ip_address.as_deref(),
    )
    .await?;

    Ok(Json(status))
}

fn request_body_as_str(body: &Bytes) -> Result<&str, ApiError> {
    str::from_utf8(body).map_err(|_| ApiError::bad_request("request body must be valid UTF-8"))
}

struct RequestContext {
    org_id: String,
    workspace_id: String,
    ip_address: Option<String>,
}

impl RequestContext {
    fn scope(&self) -> Scope<'_> {
        Scope {
            org_id: &self.org_id,
            workspace_id: &self.workspace_id,
        }
    }

    fn from_headers(headers: &HeaderMap, peer_addr: Option<SocketAddr>) -> Result<Self, ApiError> {
        let org_id = required_header(headers, ORG_ID_HEADER)?;
        let workspace_id = required_header(headers, WORKSPACE_ID_HEADER)?;
        validate_scope_headers(&org_id, &workspace_id)?;
        let ip_address =
            forwarded_ip(headers)?.or_else(|| peer_addr.map(|addr| addr.ip().to_string()));

        Ok(Self {
            org_id,
            workspace_id,
            ip_address,
        })
    }

    /// The scope of an authenticated request: its key's org and workspace.
    /// `x-org-id` / `x-workspace-id` are optional then, but must name the
    /// key's scope when sent.
    fn for_key(
        headers: &HeaderMap,
        peer_addr: Option<SocketAddr>,
        key: KeyScope,
    ) -> Result<Self, ApiError> {
        for (name, expected) in [
            (ORG_ID_HEADER, &key.org_id),
            (WORKSPACE_ID_HEADER, &key.workspace_id),
        ] {
            if let Some(sent) = optional_header(headers, name)?
                && sent != *expected
            {
                return Err(AppError::ScopeMismatch(format!(
                    "the `{name}` header is {sent:?}, but the key is for {expected:?}"
                ))
                .into());
            }
        }
        let ip_address =
            forwarded_ip(headers)?.or_else(|| peer_addr.map(|addr| addr.ip().to_string()));

        Ok(Self {
            org_id: key.org_id,
            workspace_id: key.workspace_id,
            ip_address,
        })
    }
}

fn required_header(headers: &HeaderMap, name: &'static str) -> Result<String, ApiError> {
    optional_header(headers, name)?
        .ok_or_else(|| ApiError::bad_request(format!("missing required `{}` header", name)))
}

fn optional_header(headers: &HeaderMap, name: &'static str) -> Result<Option<String>, ApiError> {
    let Some(value) = headers.get(name) else {
        return Ok(None);
    };

    let value = value
        .to_str()
        .map_err(|_| ApiError::bad_request(format!("`{}` header must be valid UTF-8", name)))?
        .trim();

    Ok((!value.is_empty()).then(|| value.to_string()))
}

/// Checks the `x-org-id` and `x-workspace-id` headers: an org slug, and a
/// workspace id (`{org_id}-{workspace slug}`) of that org. They select the
/// CAC config and the topic for the request, so malformed values are
/// rejected here, before any processing.
fn validate_scope_headers(org_id: &str, workspace_id: &str) -> Result<(), ApiError> {
    if !is_valid_slug(org_id) {
        return Err(ApiError::bad_request(format!(
            "`{ORG_ID_HEADER}` header must be a valid slug: 3-30 characters of lowercase \
             letters, digits, and underscores, starting with a lowercase letter and ending with \
             a lowercase letter or digit"
        )));
    }
    let workspace_id: WorkspaceId = workspace_id.parse().map_err(|_| {
        ApiError::bad_request(format!(
            "`{WORKSPACE_ID_HEADER}` header must be a workspace id: the org's slug and the \
             workspace's slug joined by `-`, e.g. `{org_id}-web_shop`"
        ))
    })?;
    if workspace_id.org_slug() != org_id {
        return Err(ApiError::bad_request(format!(
            "workspace `{workspace_id}` is not in org `{org_id}`"
        )));
    }
    Ok(())
}

fn forwarded_ip(headers: &HeaderMap) -> Result<Option<String>, ApiError> {
    if let Some(forwarded_for) = optional_header(headers, FORWARDED_FOR_HEADER)? {
        return Ok(forwarded_for
            .split(',')
            .next()
            .map(str::trim)
            .filter(|ip| !ip.is_empty())
            .map(ToOwned::to_owned));
    }

    optional_header(headers, REAL_IP_HEADER)
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    authenticated_endpoint: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug)]
enum ApiError {
    BadRequest(String),
    ServiceUnavailable(String),
    App(AppError),
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }

    fn service_unavailable(message: impl Into<String>) -> Self {
        Self::ServiceUnavailable(message.into())
    }
}

impl From<AppError> for ApiError {
    fn from(error: AppError) -> Self {
        Self::App(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error, www_authenticate) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message, false),
            Self::ServiceUnavailable(message) => (StatusCode::SERVICE_UNAVAILABLE, message, false),
            Self::App(AppError::Json(error)) => (StatusCode::BAD_REQUEST, error.to_string(), false),
            Self::App(AppError::ScopeMismatch(message)) => {
                (StatusCode::BAD_REQUEST, message, false)
            }
            Self::App(AppError::Kafka(error)) => {
                (StatusCode::BAD_GATEWAY, error.to_string(), false)
            }
            Self::App(AppError::InvalidToken) => (
                StatusCode::UNAUTHORIZED,
                "invalid bearer token".to_string(),
                true,
            ),
            Self::App(AppError::MissingCredentials) => (
                StatusCode::UNAUTHORIZED,
                "missing bearer token".to_string(),
                true,
            ),
            Self::App(error @ (AppError::KeyStore(_) | AppError::AuthNotConfigured)) => {
                (StatusCode::SERVICE_UNAVAILABLE, error.to_string(), false)
            }
            Self::App(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string(), false),
        };

        let mut response = (status, Json(ErrorResponse { error })).into_response();
        if www_authenticate {
            response
                .headers_mut()
                .insert("WWW-Authenticate", HeaderValue::from_static("Bearer"));
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_context_reads_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(ORG_ID_HEADER, "acme_retail".parse().unwrap());
        headers.insert(WORKSPACE_ID_HEADER, "acme_retail-web_shop".parse().unwrap());
        headers.insert(
            FORWARDED_FOR_HEADER,
            "203.0.113.1, 203.0.113.2".parse().unwrap(),
        );

        let peer_addr = "127.0.0.1:51000".parse().unwrap();
        let context = RequestContext::from_headers(&headers, Some(peer_addr)).unwrap();

        assert_eq!(context.org_id, "acme_retail");
        assert_eq!(context.workspace_id, "acme_retail-web_shop");
        assert_eq!(context.ip_address.as_deref(), Some("203.0.113.1"));
    }

    #[test]
    fn request_context_falls_back_to_peer_ip() {
        let mut headers = HeaderMap::new();
        headers.insert(ORG_ID_HEADER, "acme_retail".parse().unwrap());
        headers.insert(WORKSPACE_ID_HEADER, "acme_retail-web_shop".parse().unwrap());
        let peer_addr = "127.0.0.1:51000".parse().unwrap();

        let context = RequestContext::from_headers(&headers, Some(peer_addr)).unwrap();

        assert_eq!(context.ip_address.as_deref(), Some("127.0.0.1"));
    }

    #[test]
    fn request_context_requires_org_id() {
        let mut headers = HeaderMap::new();
        headers.insert(WORKSPACE_ID_HEADER, "acme_retail-web_shop".parse().unwrap());

        assert!(matches!(
            RequestContext::from_headers(&headers, None),
            Err(ApiError::BadRequest(_))
        ));
    }

    #[test]
    fn request_context_requires_workspace_id() {
        let mut headers = HeaderMap::new();
        headers.insert(ORG_ID_HEADER, "acme_retail".parse().unwrap());

        assert!(matches!(
            RequestContext::from_headers(&headers, None),
            Err(ApiError::BadRequest(_))
        ));
    }

    #[test]
    fn request_context_rejects_invalid_ids() {
        for (org_id, workspace_id) in [
            // Invalid org slugs.
            ("ac", "ac-web_shop"),
            ("acme-retail", "acme-retail-web_shop"),
            ("Acme_retail", "Acme_retail-web_shop"),
            ("9acme", "9acme-web_shop"),
            ("acme_retail_with_a_very_long_name", "acme_retail_with_a_very_long_name-web_shop"),
            // Invalid workspace ids.
            ("acme_retail", "web_shop"),
            ("acme_retail", "acme_retail-"),
            ("acme_retail", "acme_retail-web-shop"),
            ("acme_retail", "acme_retail/web_shop"),
            // Another org's workspace.
            ("acme_retail", "north_star-web_shop"),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(ORG_ID_HEADER, org_id.parse().unwrap());
            headers.insert(WORKSPACE_ID_HEADER, workspace_id.parse().unwrap());

            assert!(
                matches!(
                    RequestContext::from_headers(&headers, None),
                    Err(ApiError::BadRequest(_))
                ),
                "`{org_id}` / `{workspace_id}` should be rejected"
            );
        }
    }

    fn key_scope() -> KeyScope {
        KeyScope {
            org_id: "acme_retail".into(),
            workspace_id: "acme_retail-web_shop".into(),
        }
    }

    #[test]
    fn authenticated_requests_take_the_keys_scope() {
        let context = RequestContext::for_key(&HeaderMap::new(), None, key_scope()).unwrap();

        assert_eq!(context.org_id, "acme_retail");
        assert_eq!(context.workspace_id, "acme_retail-web_shop");
    }

    #[test]
    fn authenticated_requests_accept_headers_naming_the_keys_scope() {
        let mut headers = HeaderMap::new();
        headers.insert(ORG_ID_HEADER, "acme_retail".parse().unwrap());
        headers.insert(WORKSPACE_ID_HEADER, "acme_retail-web_shop".parse().unwrap());

        assert!(RequestContext::for_key(&headers, None, key_scope()).is_ok());
    }

    #[test]
    fn authenticated_requests_refuse_headers_for_another_scope() {
        for (header, value) in [
            (ORG_ID_HEADER, "other_org"),
            (WORKSPACE_ID_HEADER, "acme_retail-other"),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(header, value.parse().unwrap());

            assert!(
                matches!(
                    RequestContext::for_key(&headers, None, key_scope()),
                    Err(ApiError::App(AppError::ScopeMismatch(_)))
                ),
                "{header}: {value}"
            );
        }
    }

    #[test]
    fn an_unavailable_key_store_is_service_unavailable() {
        for error in [
            AppError::KeyStore("down".into()),
            AppError::AuthNotConfigured,
        ] {
            let response = ApiError::from(error).into_response();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        }
    }

    #[test]
    fn scope_mismatch_is_a_bad_request() {
        let response = ApiError::from(AppError::ScopeMismatch(
            "event 0 is for another workspace".into(),
        ))
        .into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn health_response_includes_authenticated_endpoint_status() {
        let payload = serde_json::to_value(HealthResponse {
            status: "ok",
            authenticated_endpoint: "ok".to_string(),
        })
        .unwrap();

        assert_eq!(
            payload.get("status").and_then(serde_json::Value::as_str),
            Some("ok")
        );
        assert_eq!(
            payload
                .get("authenticated_endpoint")
                .and_then(serde_json::Value::as_str),
            Some("ok")
        );
    }
}
