//! Client for the CDP control plane: customer profiles, metrics and
//! segments built from a project's events. An organization is a CDP tenant
//! and a project is one of its workspaces; their slugs are used as the CDP
//! ids unchanged.
//!
//! dashboard-server is the CDP's only gateway. It holds the CDP's platform
//! token (`CDP_PLATFORM_TOKEN`), checks the caller's project membership
//! itself, and then names the tenant (and, for read-only calls, the
//! workspace) the request acts as in `X-CDP-Tenant` / `X-CDP-Workspace`. The
//! browser never sees a CDP credential.
//!
//! Configured with `CDP_CONTROL_PLANE_URL` (e.g.
//! `http://localhost:8082/cdp/control-plane`) and `CDP_PLATFORM_TOKEN`;
//! without both, the CDP is disabled: provisioning skips it and the `/cdp`
//! routes answer 503.

use std::time::Duration;

use canonical_event::{OrgId, ProjectId};
use reqwest::{Client, Method, StatusCode};
use serde_json::{Value, json};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, thiserror::Error)]
pub enum CdpError {
    #[error("CDP request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("CDP answered {status} to {action}: {body}")]
    Status {
        action: &'static str,
        status: StatusCode,
        body: String,
    },
}

/// Who a forwarded request acts as.
#[derive(Clone, Copy)]
pub enum CdpScope<'a> {
    /// The whole tenant (the organization): reads and config writes.
    Tenant(&'a OrgId),
    /// One workspace (the project), read-only — the CDP refuses writes.
    Workspace(&'a OrgId, &'a ProjectId),
}

#[derive(Clone)]
pub struct Cdp {
    http: Client,
    base_url: String,
    token: String,
}

impl Cdp {
    /// `None` when the CDP isn't configured.
    pub fn from_env() -> Option<Self> {
        let var = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        };
        let base_url = var("CDP_CONTROL_PLANE_URL")?;
        let token = var("CDP_PLATFORM_TOKEN")?;
        let http = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("reqwest client builds");
        Some(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_owned(),
            token,
        })
    }

    fn request(
        &self,
        method: Method,
        path: &str,
        scope: Option<CdpScope<'_>>,
    ) -> reqwest::RequestBuilder {
        let builder = self
            .http
            .request(method, format!("{}{path}", self.base_url))
            .bearer_auth(&self.token);
        match scope {
            None => builder,
            Some(CdpScope::Tenant(org)) => builder.header("X-CDP-Tenant", org.as_ref()),
            Some(CdpScope::Workspace(org, project)) => builder
                .header("X-CDP-Tenant", org.as_ref())
                .header("X-CDP-Workspace", project.as_ref()),
        }
    }

    /// Connectivity probe: the control plane answered with anything but a
    /// server error.
    pub async fn ping(&self) -> Result<(), CdpError> {
        let response = self.request(Method::GET, "/v1/tenants", None).send().await?;
        if response.status().is_server_error() {
            return Err(status_error("ping", response).await);
        }
        Ok(())
    }

    /// Creates the organization's tenant (if absent) and the project's
    /// workspace. Safe to run again: both already existing is success.
    pub async fn provision(
        &self,
        org: &OrgId,
        org_name: &str,
        project: &ProjectId,
        project_name: &str,
    ) -> Result<(), CdpError> {
        let tenant = self
            .request(Method::PUT, &format!("/v1/tenants/{}", org.as_ref()), None)
            .json(&json!({ "display_name": org_name }))
            .send()
            .await?;
        let status = tenant.status();
        if !status.is_success() {
            return Err(status_error("provision tenant", tenant).await);
        }
        let body: Value = tenant.json().await?;
        if let Some(missing) = body["missing_keys"]
            .as_array()
            .filter(|keys| !keys.is_empty())
        {
            // Events are rejected until ops adds them; the workspace can
            // still be created, so provisioning carries on.
            tracing::warn!(org = %org.as_ref(), ?missing, "CDP tenant has no keys of these kinds yet");
        }

        let workspace = self
            .request(
                Method::POST,
                &format!("/v1/tenants/{}/workspaces", org.as_ref()),
                Some(CdpScope::Tenant(org)),
            )
            .json(&json!({ "workspace_id": project.as_ref(), "display_name": project_name }))
            .send()
            .await?;
        match workspace.status() {
            status if status.is_success() => Ok(()),
            StatusCode::CONFLICT => Ok(()),
            _ => Err(status_error("create workspace", workspace).await),
        }
    }

    /// Permanently deletes the project's workspace and all its CDP data. The
    /// tenant stays: the organization may have other projects. A workspace
    /// (or tenant) that doesn't exist is success.
    pub async fn deprovision(&self, org: &OrgId, project: &ProjectId) -> Result<(), CdpError> {
        let response = self
            .request(
                Method::DELETE,
                &format!(
                    "/v1/tenants/{}/workspaces/{}?confirm={}",
                    org.as_ref(),
                    project.as_ref(),
                    project.as_ref()
                ),
                Some(CdpScope::Tenant(org)),
            )
            .send()
            .await?;
        match response.status() {
            status if status.is_success() => Ok(()),
            StatusCode::NOT_FOUND => Ok(()),
            _ => Err(status_error("delete workspace", response).await),
        }
    }

    /// Forwards one call and returns the CDP's status and JSON body as they
    /// are (a non-JSON body becomes `{"error": <text>}`). `path` starts with
    /// `/v1/`; `query` is URL-encoded here.
    pub async fn forward(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        scope: CdpScope<'_>,
        body: Option<&Value>,
    ) -> Result<(StatusCode, Value), CdpError> {
        let mut request = self.request(method, path, Some(scope)).query(query);
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request.send().await?;
        let status = response.status();
        let text = response.text().await?;
        let value = serde_json::from_str(&text).unwrap_or_else(|_| json!({ "error": text }));
        Ok((status, value))
    }
}

async fn status_error(action: &'static str, response: reqwest::Response) -> CdpError {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    CdpError::Status {
        action,
        status,
        body,
    }
}
