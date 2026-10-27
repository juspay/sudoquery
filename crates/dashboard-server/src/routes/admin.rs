//! Admin API: provisions the workspaces created from the dashboard.
//!
//! A workspace starts `PENDING`. `POST /admin/workspaces/{workspace_id}/provision`
//! moves it to `CREATING` and provisions it in the background, ending
//! `ACTIVE`, or `ERRORED` with the failed step in `provisioning_error`. An
//! `ERRORED` workspace can be provisioned again; finished steps are kept.

use std::str::FromStr;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use canonical_event::{OrgId, WorkspaceId};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    AppState,
    db::{self, ProvisioningStart, WorkspaceStatus},
    middleware::Admin,
    provisioning::{self, STALE_CREATING_AFTER},
};

#[derive(Serialize)]
pub struct ProvisioningStarted {
    pub id: String,
    pub status: WorkspaceStatus,
}

#[derive(Serialize)]
pub struct ProvisioningResponse {
    pub id: String,
    pub organization_id: Option<String>,
    pub status: WorkspaceStatus,
    /// Set while the workspace is `ERRORED`: the failed step, its error, and
    /// which steps completed or were skipped.
    pub provisioning_error: Option<Value>,
    /// When the last attempt started.
    pub provisioning_started_at: Option<String>,
    pub updated_at: String,
}

// ============ Provision Workspace ============

pub async fn provision_workspace(
    State(state): State<AppState>,
    _: Admin,
    Path(workspace_id): Path<String>,
) -> Result<(StatusCode, Json<ProvisioningStarted>), AdminError> {
    let workspace_id =
        WorkspaceId::from_str(&workspace_id).map_err(|_| AdminError::InvalidWorkspaceId)?;

    let start = db::start_provisioning(&state.db_pool, &workspace_id, STALE_CREATING_AFTER)
        .await
        .map_err(|e| AdminError::Database(e.to_string()))?;
    let (org_id, attempt) = match start {
        ProvisioningStart::Started { org_id, attempt } => (org_id, attempt),
        ProvisioningStart::NotFound => return Err(AdminError::NotFound),
        ProvisioningStart::NoOrganization => return Err(AdminError::NoOrganization),
        ProvisioningStart::OrganizationDeleted => return Err(AdminError::OrganizationDeleted),
        ProvisioningStart::Refused(status) => return Err(AdminError::Refused(status)),
    };

    tokio::spawn(provision_in_background(
        state.clone(),
        org_id,
        workspace_id.clone(),
        attempt,
    ));

    Ok((
        StatusCode::ACCEPTED,
        Json(ProvisioningStarted {
            id: workspace_id.to_string(),
            status: WorkspaceStatus::Creating,
        }),
    ))
}

async fn provision_in_background(
    state: AppState,
    org_id: OrgId,
    workspace_id: WorkspaceId,
    attempt: Uuid,
) {
    let (status, error) =
        match provisioning::provision_workspace(&state, &org_id, &workspace_id).await {
            Ok(()) => {
                tracing::info!("Provisioned workspace {}", workspace_id);
                (WorkspaceStatus::Active, None)
            }
            Err(failure) => {
                tracing::error!(
                    "Provisioning workspace {} failed at {:?}: {}",
                    workspace_id,
                    failure.failed_step,
                    failure.error
                );
                (
                    WorkspaceStatus::Errored,
                    serde_json::to_value(&failure).ok(),
                )
            }
        };

    // Should this fail, the workspace stays CREATING until it goes stale.
    match db::finish_provisioning(&state.db_pool, &workspace_id, attempt, status, error).await {
        Ok(true) => {}
        Ok(false) => tracing::warn!(
            "Provisioning attempt {} for workspace {} was superseded; outcome {:?} not recorded",
            attempt,
            workspace_id,
            status
        ),
        Err(e) => tracing::error!(
            "Failed to record provisioning outcome {:?} for workspace {}: {}",
            status,
            workspace_id,
            e
        ),
    }
}

// ============ Get Workspace Provisioning ============

pub async fn get_workspace_provisioning(
    State(state): State<AppState>,
    _: Admin,
    Path(workspace_id): Path<String>,
) -> Result<Json<ProvisioningResponse>, AdminError> {
    let workspace_id =
        WorkspaceId::from_str(&workspace_id).map_err(|_| AdminError::InvalidWorkspaceId)?;

    let workspace = db::get_workspace_provisioning(&state.db_pool, &workspace_id)
        .await
        .map_err(|e| AdminError::Database(e.to_string()))?
        .ok_or(AdminError::NotFound)?;

    Ok(Json(ProvisioningResponse {
        id: workspace.id.to_string(),
        organization_id: workspace.organization_id.map(|id| id.to_string()),
        status: workspace.status,
        provisioning_error: workspace.provisioning_error,
        provisioning_started_at: workspace.provisioning_started_at.map(|at| at.to_rfc3339()),
        updated_at: workspace.updated_at.to_rfc3339(),
    }))
}

// ============ Error Types ============

#[derive(Debug, thiserror::Error)]
pub enum AdminError {
    #[error("Invalid workspace ID")]
    InvalidWorkspaceId,
    #[error("Workspace not found")]
    NotFound,
    #[error("Workspace has no organization to provision it under")]
    NoOrganization,
    #[error("Workspace's organization was deleted")]
    OrganizationDeleted,
    #[error("Workspace is {0:?}")]
    Refused(WorkspaceStatus),
    #[error("Database error: {0}")]
    Database(String),
}

impl IntoResponse for AdminError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            AdminError::InvalidWorkspaceId => (
                StatusCode::BAD_REQUEST,
                serde_json::json!({ "error": "Invalid workspace ID" }),
            ),
            AdminError::NotFound => (
                StatusCode::NOT_FOUND,
                serde_json::json!({ "error": "Workspace not found" }),
            ),
            AdminError::NoOrganization => (
                StatusCode::CONFLICT,
                serde_json::json!({ "error": "Workspace has no organization to provision it under" }),
            ),
            AdminError::OrganizationDeleted => (
                StatusCode::CONFLICT,
                serde_json::json!({ "error": "Workspace's organization was deleted" }),
            ),
            AdminError::Refused(workspace_status) => {
                let message = match workspace_status {
                    WorkspaceStatus::Creating => "Workspace is already being provisioned",
                    WorkspaceStatus::Active => "Workspace is already provisioned",
                    WorkspaceStatus::Deleting => "Workspace is being deleted",
                    WorkspaceStatus::Pending | WorkspaceStatus::Errored => {
                        "Workspace cannot be provisioned now"
                    }
                };
                (
                    StatusCode::CONFLICT,
                    serde_json::json!({ "error": message, "status": workspace_status }),
                )
            }
            AdminError::Database(e) => {
                tracing::error!("Database error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    serde_json::json!({ "error": "Internal server error" }),
                )
            }
        };
        (status, Json(body)).into_response()
    }
}
