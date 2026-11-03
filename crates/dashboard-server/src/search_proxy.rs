//! Client for opensearch-proxy, the service that runs event searches. The
//! routes in routes/search.rs, aggregations.rs and sessions.rs check the
//! caller's access to the workspace, then forward the request body as it
//! came and return the proxy's answer as it went.

use std::time::Duration;

use axum::{
    body::{Body, Bytes},
    http::{StatusCode, header},
    response::{IntoResponse, Json, Response},
};
use reqwest::{Method, Url};
use serde_json::json;

use crate::db;

#[derive(Clone)]
pub struct SearchProxy {
    http: reqwest::Client,
    base_url: Url,
    /// Bearer token opensearch-proxy requires (`OPENSEARCH_PROXY_TOKEN`).
    token: Option<String>,
}

impl SearchProxy {
    pub fn new(base_url: &str, token: Option<String>, timeout: Duration) -> Result<Self, String> {
        let base_url = Url::parse(base_url).map_err(|error| format!("{base_url}: {error}"))?;
        if base_url.cannot_be_a_base() {
            return Err(format!("{base_url} cannot be a base URL"));
        }
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            http,
            base_url,
            token,
        })
    }

    /// `{base}/v1/orgs/{org}/workspaces/{workspace}/events/{path…}`, each
    /// segment escaped: session and document ids come from the client.
    fn url(&self, org_id: &str, workspace_id: &str, path: &[&str]) -> Url {
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .expect("checked in new")
            .pop_if_empty()
            .extend(["v1", "orgs", org_id, "workspaces", workspace_id, "events"])
            .extend(path);
        url
    }

    /// Sends the request for the workspace's events and relays the answer.
    pub async fn forward(
        &self,
        method: Method,
        workspace: &db::Workspace,
        path: &[&str],
        body: Option<Bytes>,
    ) -> Response {
        // Events are stored per organization, so a workspace outside any
        // organization has none.
        let Some(org_id) = workspace.organization_id.as_ref() else {
            return error(
                StatusCode::BAD_REQUEST,
                "Workspace does not belong to an organization",
            );
        };
        let url = self.url(org_id.as_ref(), workspace.id.as_ref(), path);

        let mut request = self.http.request(method, url);
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request
                .header(header::CONTENT_TYPE, "application/json")
                .body(body);
        }

        let reply = match request.send().await {
            Ok(reply) => reply,
            Err(error) => {
                tracing::error!("opensearch-proxy request failed: {}", error.without_url());
                return error_unavailable();
            }
        };
        let status = StatusCode::from_u16(reply.status().as_u16())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        match reply.bytes().await {
            Ok(bytes) => (
                status,
                [(header::CONTENT_TYPE, "application/json")],
                Body::from(bytes),
            )
                .into_response(),
            Err(error) => {
                tracing::error!(
                    "opensearch-proxy answer unreadable: {}",
                    error.without_url()
                );
                error_unavailable()
            }
        }
    }
}

fn error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn error_unavailable() -> Response {
    error(StatusCode::BAD_GATEWAY, "Event search is unavailable")
}

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;
    use chrono::Utc;
    use wiremock::matchers::{body_json, header as has_header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn workspace(organization: Option<&str>) -> db::Workspace {
        db::Workspace {
            id: canonical_event::WorkspaceId::try_from("acme_corp-shop".to_string()).unwrap(),
            organization_id: organization
                .map(|org| canonical_event::OrgId::try_from(org.to_string()).unwrap()),
            name: "shop".into(),
            timezone: None,
            status: db::WorkspaceStatus::Active,
            deleted_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn proxy(base: &str, token: Option<&str>) -> SearchProxy {
        SearchProxy::new(base, token.map(str::to_owned), Duration::from_secs(2)).unwrap()
    }

    async fn text(response: Response) -> String {
        String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn forwards_to_the_workspaces_events_with_the_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(
                "/prefix/v1/orgs/acme_corp/workspaces/acme_corp-shop/events/sessions/a%2Fb%20c",
            ))
            .and(has_header("authorization", "Bearer secret"))
            .and(body_json(json!({ "page_size": 5 })))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"items":[]}"#))
            .expect(1)
            .mount(&server)
            .await;

        let response = proxy(&format!("{}/prefix/", server.uri()), Some("secret"))
            .forward(
                Method::POST,
                &workspace(Some("acme_corp")),
                &["sessions", "a/b c"],
                Some(Bytes::from_static(br#"{"page_size":5}"#)),
            )
            .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(text(response).await, r#"{"items":[]}"#);
    }

    #[tokio::test]
    async fn relays_the_proxys_error_as_it_is() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(400).set_body_string(r#"{"error":"bad query"}"#))
            .mount(&server)
            .await;

        let response = proxy(&server.uri(), None)
            .forward(
                Method::POST,
                &workspace(Some("acme_corp")),
                &["search"],
                None,
            )
            .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(text(response).await, r#"{"error":"bad query"}"#);
    }

    #[tokio::test]
    async fn an_unreachable_proxy_is_a_bad_gateway() {
        // Nothing listens on port 9 (discard) in the test environment.
        let response = proxy("http://127.0.0.1:9", None)
            .forward(
                Method::POST,
                &workspace(Some("acme_corp")),
                &["search"],
                None,
            )
            .await;

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }

    #[tokio::test]
    async fn a_workspace_without_an_organization_has_no_events() {
        let response = proxy("http://127.0.0.1:9", None)
            .forward(Method::POST, &workspace(None), &["search"], None)
            .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
