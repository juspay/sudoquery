//! `/api/cdp/*`: the project's customer profiles and segments, served by the
//! CDP control plane through [`crate::cdp::Cdp`].
//!
//! Every route is for the `X-Project-Id` project. Reads need project
//! membership and act as the project's CDP workspace — read-only on the CDP
//! side too. Segment writes need project (or organization) admin and act as
//! the organization's tenant, with `workspace_id` always set to the project
//! here, never taken from the caller. The event and metric vocabulary is the
//! organization's (tenant-wide), so changing it needs organization admin
//! (`X-Organization-Id`); metric claims made here apply to every project.
//! Query parameters are forwarded only from a per-route allow-list, and path
//! ids are checked before they become part of a CDP path.

use std::collections::HashMap;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use canonical_event::{OrgId, ProjectId};
use reqwest::Method;
use serde_json::{Value, json};

use crate::{
    AppState,
    cdp::{Cdp, CdpError, CdpScope},
    db,
    middleware::{OrgAdmin, ProjectAccess, ProjectAdminOrOrgAdmin},
};

type CdpResult = Result<(StatusCode, Json<Value>), CdpRouteError>;

#[derive(Debug)]
pub enum CdpRouteError {
    Disabled,
    NoOrganization,
    BadId(&'static str),
    Upstream(CdpError),
}

impl From<CdpError> for CdpRouteError {
    fn from(error: CdpError) -> Self {
        CdpRouteError::Upstream(error)
    }
}

impl IntoResponse for CdpRouteError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            CdpRouteError::Disabled => (
                StatusCode::SERVICE_UNAVAILABLE,
                "the CDP is not configured".to_owned(),
            ),
            CdpRouteError::NoOrganization => (
                StatusCode::CONFLICT,
                "project has no organization".to_owned(),
            ),
            CdpRouteError::BadId(what) => (StatusCode::BAD_REQUEST, format!("invalid {what}")),
            CdpRouteError::Upstream(error) => {
                tracing::error!(%error, "CDP request failed");
                (StatusCode::BAD_GATEWAY, "the CDP is unavailable".to_owned())
            }
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

fn cdp(state: &AppState) -> Result<&Cdp, CdpRouteError> {
    state.cdp.as_ref().ok_or(CdpRouteError::Disabled)
}

fn org_of(project: &db::Project) -> Result<&OrgId, CdpRouteError> {
    project
        .organization_id
        .as_ref()
        .ok_or(CdpRouteError::NoOrganization)
}

/// Segment ids are caller-chosen; keep them to characters that can't change
/// the CDP path they are placed in.
fn segment_id(id: &str) -> Result<&str, CdpRouteError> {
    let ok = (1..=128).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    ok.then_some(id).ok_or(CdpRouteError::BadId("segment id"))
}

/// Event names as the SDKs send them (`order_completed`, `checkout.started`)
/// — never a lone `.` / `..`, which a URL would resolve as a path step.
fn event_name(name: &str) -> Result<&str, CdpRouteError> {
    let ok = (1..=128).contains(&name.len())
        && name.bytes().any(|b| b != b'.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'));
    ok.then_some(name).ok_or(CdpRouteError::BadId("event name"))
}

fn profile_id(id: &str) -> Result<&str, CdpRouteError> {
    uuid::Uuid::parse_str(id)
        .map(|_| id)
        .map_err(|_| CdpRouteError::BadId("profile id"))
}

/// The allowed keys of `query`, in a stable order.
fn allowed<'a>(query: &'a HashMap<String, String>, keys: &[&'a str]) -> Vec<(&'a str, &'a str)> {
    keys.iter()
        .filter_map(|key| query.get(*key).map(|value| (*key, value.as_str())))
        .collect()
}

fn reply((status, body): (StatusCode, Value)) -> CdpResult {
    Ok((status, Json(body)))
}

/// A read of the project's own workspace, acting as that workspace.
async fn workspace_read(
    state: &AppState,
    project: &db::Project,
    suffix: &str,
    query: &[(&str, &str)],
) -> CdpResult {
    let org = org_of(project)?;
    let path = format!(
        "/v1/tenants/{}/workspaces/{}{suffix}",
        org.as_ref(),
        project.id.as_ref()
    );
    let scope = CdpScope::Workspace(org, &project.id);
    reply(
        cdp(state)?
            .forward(Method::GET, &path, query, scope, None)
            .await?,
    )
}

// ============ Profiles ============

pub async fn summary(
    State(state): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
) -> CdpResult {
    workspace_read(&state, &project, "/profiles/summary", &[]).await
}

pub async fn list_profiles(
    State(state): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
    Query(query): Query<HashMap<String, String>>,
) -> CdpResult {
    let query = allowed(&query, &["limit", "cursor", "profile_type"]);
    workspace_read(&state, &project, "/profiles", &query).await
}

pub async fn lookup_profile(
    State(state): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
    Query(query): Query<HashMap<String, String>>,
) -> CdpResult {
    let query = allowed(&query, &["type", "value"]);
    workspace_read(&state, &project, "/profiles/lookup", &query).await
}

pub async fn get_profile(
    State(state): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
    Path(id): Path<String>,
) -> CdpResult {
    let suffix = format!("/profiles/{}", profile_id(&id)?);
    workspace_read(&state, &project, &suffix, &[]).await
}

pub async fn profile_segments(
    State(state): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
    Path(id): Path<String>,
) -> CdpResult {
    let suffix = format!("/profiles/{}/segments", profile_id(&id)?);
    workspace_read(&state, &project, &suffix, &[]).await
}

// ============ Segments ============

pub async fn list_segments(
    State(state): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
    Query(query): Query<HashMap<String, String>>,
) -> CdpResult {
    let org = org_of(&project)?;
    let mut forwarded = allowed(&query, &["limit", "cursor"]);
    forwarded.push(("workspace_id", project.id.as_ref()));
    let path = format!("/v1/tenants/{}/segments", org.as_ref());
    let scope = CdpScope::Workspace(org, &project.id);
    reply(
        cdp(&state)?
            .forward(Method::GET, &path, &forwarded, scope, None)
            .await?,
    )
}

pub async fn get_segment(
    State(state): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
    Path(id): Path<String>,
) -> CdpResult {
    let org = org_of(&project)?;
    let path = format!(
        "/v1/tenants/{}/segments/{}/definition",
        org.as_ref(),
        segment_id(&id)?
    );
    let query = [("workspace_id", project.id.as_ref())];
    let scope = CdpScope::Workspace(org, &project.id);
    reply(
        cdp(&state)?
            .forward(Method::GET, &path, &query, scope, None)
            .await?,
    )
}

pub async fn segment_members(
    State(state): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> CdpResult {
    let suffix = format!("/segments/{}/members", segment_id(&id)?);
    let query = allowed(&query, &["limit", "cursor", "profile_type"]);
    workspace_read(&state, &project, &suffix, &query).await
}

/// `body` with `workspace_id` set to the project — whatever the caller sent.
fn for_project(mut body: Value, project: &ProjectId) -> Value {
    if let Value::Object(fields) = &mut body {
        fields.insert("workspace_id".to_owned(), json!(project.as_ref()));
    }
    body
}

/// Dry-run of a segment rule. Writes nothing, so any member may preview;
/// the CDP serves it at tenant scope only, hence the tenant scope here.
pub async fn preview_segment(
    State(state): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
    Json(body): Json<Value>,
) -> CdpResult {
    let org = org_of(&project)?;
    let path = format!("/v1/tenants/{}/segments/preview", org.as_ref());
    let body = for_project(body, &project.id);
    reply(
        cdp(&state)?
            .forward(Method::POST, &path, &[], CdpScope::Tenant(org), Some(&body))
            .await?,
    )
}

pub async fn create_segment(
    State(state): State<AppState>,
    ProjectAdminOrOrgAdmin { project, .. }: ProjectAdminOrOrgAdmin,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> CdpResult {
    let org = org_of(&project)?;
    let path = format!("/v1/tenants/{}/segments/{}", org.as_ref(), segment_id(&id)?);
    let body = for_project(body, &project.id);
    reply(
        cdp(&state)?
            .forward(Method::PUT, &path, &[], CdpScope::Tenant(org), Some(&body))
            .await?,
    )
}

pub async fn set_segment_status(
    State(state): State<AppState>,
    ProjectAdminOrOrgAdmin { project, .. }: ProjectAdminOrOrgAdmin,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> CdpResult {
    let org = org_of(&project)?;
    let path = format!(
        "/v1/tenants/{}/segments/{}/status",
        org.as_ref(),
        segment_id(&id)?
    );
    let body = for_project(body, &project.id);
    reply(
        cdp(&state)?
            .forward(Method::PUT, &path, &[], CdpScope::Tenant(org), Some(&body))
            .await?,
    )
}

pub async fn delete_segment(
    State(state): State<AppState>,
    ProjectAdminOrOrgAdmin { project, .. }: ProjectAdminOrOrgAdmin,
    Path(id): Path<String>,
) -> CdpResult {
    let org = org_of(&project)?;
    let path = format!("/v1/tenants/{}/segments/{}", org.as_ref(), segment_id(&id)?);
    let query = [("workspace_id", project.id.as_ref())];
    reply(
        cdp(&state)?
            .forward(Method::DELETE, &path, &query, CdpScope::Tenant(org), None)
            .await?,
    )
}

// ============ Vocabulary ============

/// The metrics segments can use in this project: tenant-wide (`*`) claims
/// plus the project's own.
pub async fn metric_definitions(
    State(state): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
) -> CdpResult {
    let org = org_of(&project)?;
    let path = format!("/v1/tenants/{}/metric-definitions", org.as_ref());
    let scope = CdpScope::Workspace(org, &project.id);
    let (status, mut body) = cdp(&state)?
        .forward(Method::GET, &path, &[], scope, None)
        .await?;
    if let Some(rows) = body
        .get_mut("metric_definitions")
        .and_then(Value::as_array_mut)
    {
        rows.retain(|row| {
            row["workspace_id"]
                .as_str()
                .is_none_or(|ws| ws == "*" || ws == project.id.as_ref())
        });
    }
    reply((status, body))
}

pub async fn event_types(
    State(state): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
) -> CdpResult {
    let org = org_of(&project)?;
    let path = format!("/v1/tenants/{}/event-types", org.as_ref());
    let scope = CdpScope::Workspace(org, &project.id);
    reply(
        cdp(&state)?
            .forward(Method::GET, &path, &[], scope, None)
            .await?,
    )
}

// ============ Vocabulary writes (organization admin) ============

/// Declares (or replaces) one of the organization's event types; only
/// declared events feed profiles and metrics.
pub async fn put_event_type(
    State(state): State<AppState>,
    OrgAdmin { organization, .. }: OrgAdmin,
    Path(name): Path<String>,
    Json(body): Json<Value>,
) -> CdpResult {
    let org = &organization.id;
    let path = format!(
        "/v1/tenants/{}/event-types/{}",
        org.as_ref(),
        event_name(&name)?
    );
    reply(
        cdp(&state)?
            .forward(Method::PUT, &path, &[], CdpScope::Tenant(org), Some(&body))
            .await?,
    )
}

pub async fn delete_event_type(
    State(state): State<AppState>,
    OrgAdmin { organization, .. }: OrgAdmin,
    Path(name): Path<String>,
) -> CdpResult {
    let org = &organization.id;
    let path = format!(
        "/v1/tenants/{}/event-types/{}",
        org.as_ref(),
        event_name(&name)?
    );
    reply(
        cdp(&state)?
            .forward(Method::DELETE, &path, &[], CdpScope::Tenant(org), None)
            .await?,
    )
}

/// Adds an organization-wide metric claim (`workspace_id` is always `*`).
pub async fn put_metric_definition(
    State(state): State<AppState>,
    OrgAdmin { organization, .. }: OrgAdmin,
    Json(body): Json<Value>,
) -> CdpResult {
    let org = &organization.id;
    let mut body = body;
    if let Value::Object(fields) = &mut body {
        fields.insert("workspace_id".to_owned(), json!("*"));
    }
    let path = format!("/v1/tenants/{}/metric-definitions", org.as_ref());
    reply(
        cdp(&state)?
            .forward(Method::PUT, &path, &[], CdpScope::Tenant(org), Some(&body))
            .await?,
    )
}

pub async fn delete_metric_definition(
    State(state): State<AppState>,
    OrgAdmin { organization, .. }: OrgAdmin,
    Query(query): Query<HashMap<String, String>>,
) -> CdpResult {
    let org = &organization.id;
    let mut forwarded = allowed(&query, &["metric_name", "event_name"]);
    forwarded.push(("workspace_id", "*"));
    let path = format!("/v1/tenants/{}/metric-definitions", org.as_ref());
    reply(
        cdp(&state)?
            .forward(
                Method::DELETE,
                &path,
                &forwarded,
                CdpScope::Tenant(org),
                None,
            )
            .await?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_ids_cannot_escape_their_path() {
        assert!(segment_id("vip-buyers_2").is_ok());
        for bad in [
            "",
            "../tokens",
            "a/b",
            "a?b",
            "a%2Fb",
            "a.b",
            &"x".repeat(129),
        ] {
            assert!(segment_id(bad).is_err(), "{bad:?} accepted");
        }
    }

    #[test]
    fn event_names_cannot_escape_their_path() {
        for good in ["order_completed", "checkout.started", "ns:click", "a-b"] {
            assert!(event_name(good).is_ok(), "{good:?} rejected");
        }
        for bad in ["", ".", "..", "a/b", "../x", "a?b", "a b", "a%2F"] {
            assert!(event_name(bad).is_err(), "{bad:?} accepted");
        }
    }

    #[test]
    fn profile_ids_must_be_uuids() {
        assert!(profile_id("2d610ef6-54b4-8325-96c0-90919b1f9377").is_ok());
        assert!(profile_id("../segments").is_err());
    }

    #[test]
    fn only_allow_listed_query_keys_are_forwarded() {
        let query: HashMap<String, String> = [
            ("limit", "10"),
            ("workspace_id", "someone-elses-project"),
            ("cursor", "c1"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
        assert_eq!(
            allowed(&query, &["limit", "cursor"]),
            [("limit", "10"), ("cursor", "c1")]
        );
    }

    #[test]
    fn the_body_workspace_is_always_the_project() {
        let project: ProjectId = "my-project-1".parse().unwrap();
        let body = for_project(
            json!({ "workspace_id": "other-project", "display_name": "VIP" }),
            &project,
        );
        assert_eq!(body["workspace_id"], "my-project-1");
        assert_eq!(body["display_name"], "VIP");
    }
}
