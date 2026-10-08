use std::net::SocketAddr;
use std::str;

use axum::body::Bytes;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use canonical_event::is_valid_slug;
use event_collector::auth;
use event_collector::config::{
    ServerConfig, get_config_from_local_file, get_default_config_from_local_file,
};
use event_collector::kafka_connector::test_connection;
use event_collector::result::AppError;
use event_collector::{
    CollectionStatus, collect_events, collect_events_authenticated, collect_events_batch,
};
use serde::Serialize;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

const TENANT_ID_HEADER: &str = "x-tenant-id";
const WORKSPACE_ID_HEADER: &str = "x-workspace-id";
const FORWARDED_FOR_HEADER: &str = "x-forwarded-for";
const REAL_IP_HEADER: &str = "x-real-ip";
const ROUTE_PREFIX: &str = "/v1";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server_config = ServerConfig::from_env()?;
    let addr = server_config.addr;
    let listener = TcpListener::bind(&addr).await?;

    println!("listening on http://{}", addr);
    axum::serve(
        listener,
        app(server_config.accept_cors).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

fn app(accept_cors: bool) -> Router {
    let routes = Router::new()
        .route("/health", get(health))
        .route("/events/batch", post(push_batch))
        .route("/events", post(push_events))
        .route("/events/authenticated", post(push_events_authenticated));

    let router = Router::new().nest(ROUTE_PREFIX, routes);

    if accept_cors {
        router.layer(CorsLayer::permissive().allow_private_network(true))
    } else {
        router
    }
}

async fn health() -> Result<Json<HealthResponse>, ApiError> {
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

    let authenticated_endpoint = match auth::health_check().await {
        Ok(()) => "ok".to_string(),
        Err(error) => error.to_string(),
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
    let config = get_config_from_local_file(context.tenant_id, context.workspace_id).await?;
    let body = request_body_as_str(&body)?;
    let status = collect_events_batch(body, &config, context.ip_address.as_deref()).await?;

    Ok(Json(status))
}

async fn push_events(
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<CollectionStatus>, ApiError> {
    let context = RequestContext::from_headers(&headers, Some(peer_addr))?;
    let config = get_config_from_local_file(context.tenant_id, context.workspace_id).await?;
    let body = request_body_as_str(&body)?;
    let status = collect_events(body, &config, context.ip_address.as_deref()).await?;

    Ok(Json(status))
}

async fn push_events_authenticated(
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<CollectionStatus>, ApiError> {
    auth::validate_bearer_token(&headers).await?;
    let context = RequestContext::from_headers(&headers, Some(peer_addr))?;
    let config = get_config_from_local_file(context.tenant_id, context.workspace_id).await?;
    let body = request_body_as_str(&body)?;
    let status = collect_events_authenticated(body, &config, context.ip_address.as_deref()).await?;

    Ok(Json(status))
}

fn request_body_as_str(body: &Bytes) -> Result<&str, ApiError> {
    str::from_utf8(body).map_err(|_| ApiError::bad_request("request body must be valid UTF-8"))
}

struct RequestContext {
    tenant_id: String,
    workspace_id: Option<String>,
    ip_address: Option<String>,
}

impl RequestContext {
    fn from_headers(headers: &HeaderMap, peer_addr: Option<SocketAddr>) -> Result<Self, ApiError> {
        let tenant_id = required_slug_header(headers, TENANT_ID_HEADER)?;
        let workspace_id = optional_slug_header(headers, WORKSPACE_ID_HEADER)?;
        let ip_address =
            forwarded_ip(headers)?.or_else(|| peer_addr.map(|addr| addr.ip().to_string()));

        Ok(Self {
            tenant_id,
            workspace_id,
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

/// Reads a required header whose value must be a valid slug
/// ([`is_valid_slug`]). The tenant/workspace ids flow into Kafka message
/// keys and OpenSearch index names, so malformed values are rejected here,
/// before any processing.
fn required_slug_header(headers: &HeaderMap, name: &'static str) -> Result<String, ApiError> {
    let value = required_header(headers, name)?;
    validate_slug_header(name, &value)?;
    Ok(value)
}

fn optional_slug_header(
    headers: &HeaderMap,
    name: &'static str,
) -> Result<Option<String>, ApiError> {
    let Some(value) = optional_header(headers, name)? else {
        return Ok(None);
    };
    validate_slug_header(name, &value)?;
    Ok(Some(value))
}

fn validate_slug_header(name: &'static str, value: &str) -> Result<(), ApiError> {
    if is_valid_slug(value) {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!(
            "`{}` header must be a valid slug: 6-30 characters of lowercase letters, digits, and \
             hyphens, starting with a lowercase letter and ending with a lowercase letter or digit",
            name
        )))
    }
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
        headers.insert(TENANT_ID_HEADER, "tenant-1".parse().unwrap());
        headers.insert(WORKSPACE_ID_HEADER, "workspace-1".parse().unwrap());
        headers.insert(
            FORWARDED_FOR_HEADER,
            "203.0.113.1, 203.0.113.2".parse().unwrap(),
        );

        let peer_addr = "127.0.0.1:51000".parse().unwrap();
        let context = RequestContext::from_headers(&headers, Some(peer_addr)).unwrap();

        assert_eq!(context.tenant_id, "tenant-1");
        assert_eq!(context.workspace_id.as_deref(), Some("workspace-1"));
        assert_eq!(context.ip_address.as_deref(), Some("203.0.113.1"));
    }

    #[test]
    fn request_context_falls_back_to_peer_ip() {
        let mut headers = HeaderMap::new();
        headers.insert(TENANT_ID_HEADER, "tenant-1".parse().unwrap());
        let peer_addr = "127.0.0.1:51000".parse().unwrap();

        let context = RequestContext::from_headers(&headers, Some(peer_addr)).unwrap();

        assert_eq!(context.ip_address.as_deref(), Some("127.0.0.1"));
    }

    #[test]
    fn request_context_requires_tenant_id() {
        let headers = HeaderMap::new();

        assert!(matches!(
            RequestContext::from_headers(&headers, None),
            Err(ApiError::BadRequest(_))
        ));
    }

    #[test]
    fn request_context_accepts_valid_slug_header_values() {
        let mut headers = HeaderMap::new();
        headers.insert(TENANT_ID_HEADER, "acme-k3x9qa".parse().unwrap());
        headers.insert(WORKSPACE_ID_HEADER, "blue-ocean-7".parse().unwrap());

        let context = RequestContext::from_headers(&headers, None).unwrap();

        assert_eq!(context.tenant_id, "acme-k3x9qa");
        assert_eq!(context.workspace_id.as_deref(), Some("blue-ocean-7"));
    }

    #[test]
    fn request_context_rejects_invalid_tenant_id_slugs() {
        for invalid_tenant_id in [
            "acme",
            "acme-store-k3x9qa-with-a-very-long-suffix",
            "Acme-k3x9qa",
            "9acme-k3x9qa",
            "acme_k3x9qa",
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(TENANT_ID_HEADER, invalid_tenant_id.parse().unwrap());

            assert!(
                matches!(
                    RequestContext::from_headers(&headers, None),
                    Err(ApiError::BadRequest(_))
                ),
                "`{invalid_tenant_id}` should be rejected as a tenant id"
            );
        }
    }

    #[test]
    fn request_context_rejects_invalid_workspace_id_slug() {
        let mut headers = HeaderMap::new();
        headers.insert(TENANT_ID_HEADER, "acme-k3x9qa".parse().unwrap());
        headers.insert(WORKSPACE_ID_HEADER, "Acme-k3x9qa".parse().unwrap());

        assert!(matches!(
            RequestContext::from_headers(&headers, None),
            Err(ApiError::BadRequest(_))
        ));
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
