use std::net::SocketAddr;
use std::str;

use axum::body::Bytes;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use event_collector::config::{get_config_from_local_file, get_default_config_from_local_file};
use event_collector::kafka_connector::test_connection;
use event_collector::result::AppError;
use event_collector::{CollectionStatus, collect_events, collect_events_batch};
use serde::Serialize;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

const TENANT_ID_HEADER: &str = "x-tenant-id";
const WORKSPACE_ID_HEADER: &str = "x-workspace-id";
const FORWARDED_FOR_HEADER: &str = "x-forwarded-for";
const REAL_IP_HEADER: &str = "x-real-ip";
const ROUTE_PREFIX: &str = "/cdp/collect";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = get_default_config_from_local_file().await?;
    let server_config = config.server_config;
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
        .route("/batch", post(push_batch))
        .route("/events", post(push_events));

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

    Ok(Json(HealthResponse { status: "ok" }))
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
        let tenant_id = required_header(headers, TENANT_ID_HEADER)?;
        let workspace_id = optional_header(headers, WORKSPACE_ID_HEADER)?;
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
        let (status, error) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::ServiceUnavailable(message) => (StatusCode::SERVICE_UNAVAILABLE, message),
            Self::App(AppError::Json(error)) => (StatusCode::BAD_REQUEST, error.to_string()),
            Self::App(AppError::Kafka(error)) => (StatusCode::BAD_GATEWAY, error.to_string()),
            Self::App(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
        };

        (status, Json(ErrorResponse { error })).into_response()
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
}
