//! Admin API: provisions the projects created from the dashboard.
//!
//! A project starts `PENDING`. `POST /admin/projects/{project_id}/provision`
//! moves it to `CREATING` and provisions it in the background, ending
//! `ACTIVE`, or `ERRORED` with the failed step in `provisioning_error`. An
//! `ERRORED` project can be provisioned again; finished steps are kept.

use std::str::FromStr;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use canonical_event::{OrgId, ProjectId};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    AppState,
    db::{self, ProjectStatus, ProvisioningStart},
    middleware::Admin,
    provisioning::{self, STALE_CREATING_AFTER},
};

#[derive(Serialize)]
pub struct ProvisioningStarted {
    pub id: String,
    pub status: ProjectStatus,
}

#[derive(Serialize)]
pub struct ProvisioningResponse {
    pub id: String,
    pub organization_id: Option<String>,
    pub status: ProjectStatus,
    /// Set while the project is `ERRORED`: the failed step, its error, and
    /// which steps completed or were skipped.
    pub provisioning_error: Option<Value>,
    /// When the last attempt started.
    pub provisioning_started_at: Option<String>,
    pub updated_at: String,
}

// ============ Provision Project ============

pub async fn provision_project(
    State(state): State<AppState>,
    _: Admin,
    Path(project_id): Path<String>,
) -> Result<(StatusCode, Json<ProvisioningStarted>), AdminError> {
    let project_id = ProjectId::from_str(&project_id).map_err(|_| AdminError::InvalidProjectId)?;

    let start = db::start_provisioning(&state.db_pool, &project_id, STALE_CREATING_AFTER)
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
        project_id.clone(),
        attempt,
    ));

    Ok((
        StatusCode::ACCEPTED,
        Json(ProvisioningStarted {
            id: project_id.to_string(),
            status: ProjectStatus::Creating,
        }),
    ))
}

async fn provision_in_background(
    state: AppState,
    org_id: OrgId,
    project_id: ProjectId,
    attempt: Uuid,
) {
    let (status, error) = match provisioning::provision_project(&state, &org_id, &project_id).await
    {
        Ok(()) => {
            tracing::info!("Provisioned project {}", project_id);
            (ProjectStatus::Active, None)
        }
        Err(failure) => {
            tracing::error!(
                "Provisioning project {} failed at {:?}: {}",
                project_id,
                failure.failed_step,
                failure.error
            );
            (ProjectStatus::Errored, serde_json::to_value(&failure).ok())
        }
    };

    // Should this fail, the project stays CREATING until it goes stale.
    match db::finish_provisioning(&state.db_pool, &project_id, attempt, status, error).await {
        Ok(true) => {}
        Ok(false) => tracing::warn!(
            "Provisioning attempt {} for project {} was superseded; outcome {:?} not recorded",
            attempt,
            project_id,
            status
        ),
        Err(e) => tracing::error!(
            "Failed to record provisioning outcome {:?} for project {}: {}",
            status,
            project_id,
            e
        ),
    }
}

// ============ Get Project Provisioning ============

pub async fn get_project_provisioning(
    State(state): State<AppState>,
    _: Admin,
    Path(project_id): Path<String>,
) -> Result<Json<ProvisioningResponse>, AdminError> {
    let project_id = ProjectId::from_str(&project_id).map_err(|_| AdminError::InvalidProjectId)?;

    let project = db::get_project_provisioning(&state.db_pool, &project_id)
        .await
        .map_err(|e| AdminError::Database(e.to_string()))?
        .ok_or(AdminError::NotFound)?;

    Ok(Json(ProvisioningResponse {
        id: project.id.to_string(),
        organization_id: project.organization_id.map(|id| id.to_string()),
        status: project.status,
        provisioning_error: project.provisioning_error,
        provisioning_started_at: project.provisioning_started_at.map(|at| at.to_rfc3339()),
        updated_at: project.updated_at.to_rfc3339(),
    }))
}

// ============ Error Types ============

#[derive(Debug, thiserror::Error)]
pub enum AdminError {
    #[error("Invalid project ID")]
    InvalidProjectId,
    #[error("Project not found")]
    NotFound,
    #[error("Project has no organization to provision it under")]
    NoOrganization,
    #[error("Project's organization was deleted")]
    OrganizationDeleted,
    #[error("Project is {0:?}")]
    Refused(ProjectStatus),
    #[error("Database error: {0}")]
    Database(String),
}

impl IntoResponse for AdminError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            AdminError::InvalidProjectId => (
                StatusCode::BAD_REQUEST,
                serde_json::json!({ "error": "Invalid project ID" }),
            ),
            AdminError::NotFound => (
                StatusCode::NOT_FOUND,
                serde_json::json!({ "error": "Project not found" }),
            ),
            AdminError::NoOrganization => (
                StatusCode::CONFLICT,
                serde_json::json!({ "error": "Project has no organization to provision it under" }),
            ),
            AdminError::OrganizationDeleted => (
                StatusCode::CONFLICT,
                serde_json::json!({ "error": "Project's organization was deleted" }),
            ),
            AdminError::Refused(project_status) => {
                let message = match project_status {
                    ProjectStatus::Creating => "Project is already being provisioned",
                    ProjectStatus::Active => "Project is already provisioned",
                    ProjectStatus::Deleting => "Project is being deleted",
                    ProjectStatus::Pending | ProjectStatus::Errored => {
                        "Project cannot be provisioned now"
                    }
                };
                (
                    StatusCode::CONFLICT,
                    serde_json::json!({ "error": message, "status": project_status }),
                )
            }
            AdminError::Database(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                serde_json::json!({ "error": e }),
            ),
        };
        (status, Json(body)).into_response()
    }
}
