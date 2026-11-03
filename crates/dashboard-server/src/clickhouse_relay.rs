//! Relays a request to clickhouse-proxy and its answer back as it came, for
//! the routes whose data the proxy owns (saved consoles, event and property
//! descriptions). The routes check the caller's access first.

use axum::{
    body::{Body, Bytes},
    http::{StatusCode, header},
    response::{IntoResponse, Json, Response},
};
use canonical_event::WorkspaceId;
use reqwest::Method;
use serde_json::json;

use crate::AppState;

pub async fn relay(
    state: &AppState,
    method: Method,
    workspace: &WorkspaceId,
    path: &[&str],
    query: &[(&str, &str)],
    body: Option<Bytes>,
) -> Response {
    match state
        .clickhouse
        .relay(
            method,
            workspace.as_ref(),
            path,
            query,
            body.map(|b| b.to_vec()),
        )
        .await
    {
        Ok((status, bytes)) => {
            let status = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
            if bytes.is_empty() {
                return status.into_response();
            }
            (
                status,
                [(header::CONTENT_TYPE, "application/json")],
                Body::from(bytes),
            )
                .into_response()
        }
        Err(error) => {
            tracing::error!("{}", error);
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": "The ClickHouse service is unavailable" })),
            )
                .into_response()
        }
    }
}
