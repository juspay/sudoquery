//! The HTTP API of `opensearch-proxy`: event search over one workspace's
//! events, for dashboard-server and the CDP dashboard.
//!
//! The org and workspace come from the path. Callers decide who may read
//! which workspace before they call, so this service is internal: with
//! `OPENSEARCH_PROXY_TOKEN` set, every request must carry it as a bearer
//! token. Isolation between workspaces comes from the scope filter every
//! query is wrapped in, whatever the client's query says.

use axum::{
    Router,
    extract::{Path, Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use subtle::ConstantTimeEq;

use crate::{
    CursorPage, Event, Facets, Histogram, OpenSearch, OpenSearchError, QueryRequest, Scope,
    SessionSummary, SortOrder,
};

#[derive(Clone)]
pub struct ProxyState {
    pub opensearch: OpenSearch,
    /// Bearer token every request must carry; `None` accepts any caller.
    pub token: Option<String>,
}

/// `/health` plus the search API under
/// `/v1/orgs/{org_id}/workspaces/{workspace_id}/events`.
pub fn router(state: ProxyState) -> Router {
    let events = Router::new()
        .route("/search", post(search))
        .route("/count", post(count))
        .route("/doc/{doc_id}", get(get_doc))
        .route("/histogram", post(histogram))
        .route("/facets", post(facets))
        .route("/sessions", post(list_sessions))
        .route("/sessions/{session_id}", post(get_session))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_token));

    Router::new()
        .route("/health", get(|| async { "ok" }))
        .nest("/v1/orgs/{org_id}/workspaces/{workspace_id}/events", events)
        .with_state(state)
}

async fn require_token(State(state): State<ProxyState>, request: Request, next: Next) -> Response {
    if let Some(expected) = &state.token {
        let presented = request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .unwrap_or_default();
        if presented.as_bytes().ct_eq(expected.as_bytes()).unwrap_u8() != 1 {
            return SearchError::Unauthorized.into_response();
        }
    }
    next.run(request).await
}

/// The workspace a request reads, from its path.
#[derive(Deserialize)]
struct WorkspacePath {
    org_id: String,
    workspace_id: String,
}

impl From<WorkspacePath> for Scope {
    fn from(path: WorkspacePath) -> Self {
        Scope {
            org_id: path.org_id,
            workspace_id: path.workspace_id,
        }
    }
}

// ============ Request/Response Types ============

#[derive(Deserialize)]
pub struct CountRequest {
    #[serde(default)]
    pub query: Option<Value>,
}

#[derive(Serialize)]
pub struct CountResponse {
    pub count: u64,
}

#[derive(Deserialize)]
pub struct HistogramRequest {
    #[serde(default)]
    pub query: Option<Value>,
    /// Start of the time range, inclusive.
    pub from: DateTime<Utc>,
    /// End of the time range, exclusive.
    pub to: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct FacetsRequest {
    #[serde(default)]
    pub query: Option<Value>,
    pub field: String,
    #[serde(default)]
    pub size: Option<usize>,
    /// Only count values starting with this text.
    #[serde(default)]
    pub prefix: Option<String>,
}

// ============ Events ============

async fn search(
    State(state): State<ProxyState>,
    Path(path): Path<WorkspacePath>,
    Json(req): Json<QueryRequest>,
) -> Result<Json<CursorPage<Event>>, SearchError> {
    let page = state
        .opensearch
        .search_events(&path.into(), &req, None, SortOrder::Desc)
        .await?;
    Ok(Json(page))
}

async fn count(
    State(state): State<ProxyState>,
    Path(path): Path<WorkspacePath>,
    Json(req): Json<CountRequest>,
) -> Result<Json<CountResponse>, SearchError> {
    let count = state
        .opensearch
        .count(&path.into(), req.query.as_ref())
        .await?;
    Ok(Json(CountResponse { count }))
}

async fn get_doc(
    State(state): State<ProxyState>,
    Path((org_id, workspace_id, doc_id)): Path<(String, String, String)>,
) -> Result<Json<Event>, SearchError> {
    let scope = Scope {
        org_id,
        workspace_id,
    };
    let event = state
        .opensearch
        .get_event(&scope, &doc_id)
        .await?
        .ok_or(SearchError::NotFound)?;
    Ok(Json(event))
}

// ============ Aggregations ============

async fn histogram(
    State(state): State<ProxyState>,
    Path(path): Path<WorkspacePath>,
    Json(req): Json<HistogramRequest>,
) -> Result<Json<Histogram>, SearchError> {
    let histogram = state
        .opensearch
        .histogram(&path.into(), req.query.as_ref(), req.from, req.to)
        .await?;
    Ok(Json(histogram))
}

/// The query must hold a bounded `range` on `occured_at`, as for the session
/// list: the aggregation reads every event it matches.
async fn facets(
    State(state): State<ProxyState>,
    Path(path): Path<WorkspacePath>,
    Json(req): Json<FacetsRequest>,
) -> Result<Json<Facets>, SearchError> {
    let facets = state
        .opensearch
        .facets(
            &path.into(),
            req.query.as_ref(),
            &req.field,
            req.size,
            req.prefix.as_deref(),
        )
        .await?;
    Ok(Json(facets))
}

// ============ Sessions ============

/// The query must hold a bounded `range` on `occured_at`: every page
/// aggregates all the events it matches.
async fn list_sessions(
    State(state): State<ProxyState>,
    Path(path): Path<WorkspacePath>,
    Json(req): Json<QueryRequest>,
) -> Result<Json<CursorPage<SessionSummary>>, SearchError> {
    let page = state.opensearch.list_sessions(&path.into(), &req).await?;
    Ok(Json(page))
}

/// The events of one session, oldest first.
async fn get_session(
    State(state): State<ProxyState>,
    Path((org_id, workspace_id, session_id)): Path<(String, String, String)>,
    Json(req): Json<QueryRequest>,
) -> Result<Json<CursorPage<Event>>, SearchError> {
    let scope = Scope {
        org_id,
        workspace_id,
    };
    let page = state
        .opensearch
        .search_events(&scope, &req, Some(&session_id), SortOrder::Asc)
        .await?;
    Ok(Json(page))
}

// ============ Errors ============

#[derive(Debug)]
pub enum SearchError {
    Unauthorized,
    NotFound,
    OpenSearch(OpenSearchError),
}

impl From<OpenSearchError> for SearchError {
    fn from(error: OpenSearchError) -> Self {
        SearchError::OpenSearch(error)
    }
}

impl IntoResponse for SearchError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            SearchError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "missing or invalid token".to_string(),
            ),
            SearchError::NotFound => (StatusCode::NOT_FOUND, "Document not found".to_string()),
            SearchError::OpenSearch(OpenSearchError::Request(error)) => {
                (StatusCode::BAD_REQUEST, error.to_string())
            }
            SearchError::OpenSearch(error) => {
                tracing::error!("OpenSearch error: {}", error);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use tower::ServiceExt;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    const BASE: &str = "/v1/orgs/acme-corp/workspaces/acme-shop/events";

    async fn app(token: Option<&str>) -> (Router, MockServer) {
        let server = MockServer::start().await;
        let opensearch = OpenSearch::new(
            &server.uri(),
            None,
            "events-{org_id}-{workspace_id}".into(),
            Duration::from_secs(2),
        )
        .unwrap();
        let state = ProxyState {
            opensearch,
            token: token.map(str::to_owned),
        };
        (router(state), server)
    }

    fn post_json(uri: &str, body: Value) -> Request<Body> {
        Request::post(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    async fn body(response: Response) -> Value {
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
    }

    #[tokio::test]
    async fn health_needs_no_token() {
        let (app, _server) = app(Some("secret")).await;
        let response = app
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn the_api_refuses_a_missing_or_wrong_token() {
        let (app, _server) = app(Some("secret")).await;
        for authorization in [None, Some("Bearer nope"), Some("secret")] {
            let mut request = post_json(&format!("{BASE}/search"), json!({}));
            if let Some(value) = authorization {
                request
                    .headers_mut()
                    .insert(header::AUTHORIZATION, value.parse().unwrap());
            }
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "{authorization:?}"
            );
        }
    }

    #[tokio::test]
    async fn a_search_reads_the_workspace_in_the_path() {
        let (app, server) = app(Some("secret")).await;
        let index = "/events-acme-corp-acme-shop";
        Mock::given(method("POST"))
            .and(path(format!("{index}/_validate/query")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "valid": true })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path(format!("{index}/_search")))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({ "hits": { "hits": [] } })),
            )
            .mount(&server)
            .await;

        let mut request = post_json(&format!("{BASE}/search"), json!({ "page_size": 10 }));
        request
            .headers_mut()
            .insert(header::AUTHORIZATION, "Bearer secret".parse().unwrap());
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            body(response).await,
            json!({ "items": [], "next_cursor": null })
        );
    }

    #[tokio::test]
    async fn a_query_refused_locally_is_a_400_with_the_reason() {
        let (app, _server) = app(None).await;
        let response = app
            .oneshot(post_json(
                &format!("{BASE}/sessions"),
                json!({ "query": { "match_all": {} } }),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(
            body(response).await["error"]
                .as_str()
                .unwrap()
                .contains("occured_at")
        );
    }

    #[tokio::test]
    async fn an_opensearch_failure_is_not_shown_to_the_caller() {
        let (app, server) = app(None).await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(503).set_body_string("cluster events-acme is red"))
            .mount(&server)
            .await;

        let response = app
            .oneshot(post_json(&format!("{BASE}/count"), json!({})))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            body(response).await,
            json!({ "error": "Internal server error" })
        );
    }
}
