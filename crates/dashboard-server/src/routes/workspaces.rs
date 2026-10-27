use axum::{extract::State, http::StatusCode, response::Json};
use canonical_event::WorkspaceId;
use sea_orm::{ActiveModelTrait, Set};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::{
    AppState,
    db::{self, DeletionStart, WorkspaceStatus},
    entities,
    middleware::{AuthUser, OrgAdmin, OrgContext, WorkspaceAccess, WorkspaceAdminOrOrgAdmin},
    provisioning,
};

// ============ Create Workspace ============

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateWorkspaceRequest {
    pub name: String,
}

#[derive(Serialize)]
pub struct CreateWorkspaceResponse {
    pub id: String,
    pub organization_id: Option<String>,
    pub name: String,
    pub status: WorkspaceStatus,
    pub workspace_token: String,
    pub created_at: String,
}

pub async fn create_workspace(
    State(state): State<AppState>,
    OrgAdmin {
        organization,
        auth_user,
    }: OrgAdmin,
    Json(req): Json<CreateWorkspaceRequest>,
) -> Result<(StatusCode, Json<CreateWorkspaceResponse>), WorkspaceError> {
    // The workspace starts PENDING: its topic, ClickHouse database and user,
    // and OpenSearch data stream are provisioned through the admin API.
    let workspace = db::create_workspace(&state.db_pool, Some(&organization.id), &req.name)
        .await
        .map_err(|e| WorkspaceError::Database(e.to_string()))?;

    // Add the creator as workspace admin
    db::add_user_to_workspace(
        &state.db_pool,
        auth_user.user.id,
        &workspace.id,
        &db::WorkspaceRole::WorkspaceAdmin,
    )
    .await
    .map_err(|e| WorkspaceError::Database(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(CreateWorkspaceResponse {
            id: workspace.id.to_string(),
            organization_id: workspace.organization_id.map(|id| id.to_string()),
            name: workspace.name,
            status: workspace.status,
            workspace_token: workspace.workspace_token.to_string(),
            created_at: workspace.created_at.to_rfc3339(),
        }),
    ))
}

// ============ List Workspaces (by Organization) ============

#[derive(Serialize)]
pub struct WorkspaceResponse {
    pub id: String,
    pub organization_id: Option<String>,
    pub name: String,
    pub timezone: Option<String>,
    pub status: WorkspaceStatus,
    pub created_at: String,
}

pub async fn list_workspaces(
    State(state): State<AppState>,
    OrgContext { organization, .. }: OrgContext,
    AuthUser { user, .. }: AuthUser,
) -> Result<Json<Vec<WorkspaceResponse>>, WorkspaceError> {
    let workspaces =
        db::list_user_workspaces_in_organization(&state.db_pool, user.id, &organization.id)
            .await
            .map_err(|e| WorkspaceError::Database(e.to_string()))?;

    let response: Vec<WorkspaceResponse> = workspaces
        .into_iter()
        .map(|p| WorkspaceResponse {
            id: p.id.to_string(),
            organization_id: p.organization_id.map(|id| id.to_string()),
            name: p.name,
            timezone: p.timezone,
            status: p.status,
            created_at: p.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(response))
}

// ============ List My Workspaces ============

pub async fn list_my_workspaces(
    State(state): State<AppState>,
    auth_user: crate::middleware::AuthUser,
) -> Result<Json<Vec<WorkspaceResponse>>, WorkspaceError> {
    let workspace_ids = db::list_user_workspaces(&state.db_pool, auth_user.user.id)
        .await
        .map_err(|e| WorkspaceError::Database(e.to_string()))?;

    let mut workspaces = Vec::new();
    for workspace_id in workspace_ids {
        if let Some(workspace) = db::get_workspace_by_id(&state.db_pool, &workspace_id)
            .await
            .map_err(|e| WorkspaceError::Database(e.to_string()))?
        {
            workspaces.push(WorkspaceResponse {
                id: workspace.id.to_string(),
                organization_id: workspace.organization_id.map(|id| id.to_string()),
                name: workspace.name,
                timezone: workspace.timezone,
                status: workspace.status,
                created_at: workspace.created_at.to_rfc3339(),
            });
        }
    }

    Ok(Json(workspaces))
}

// ============ Get Workspace ============

pub async fn get_workspace(
    WorkspaceAccess { workspace, .. }: WorkspaceAccess,
) -> Result<Json<WorkspaceResponse>, WorkspaceError> {
    Ok(Json(WorkspaceResponse {
        id: workspace.id.to_string(),
        organization_id: workspace.organization_id.map(|id| id.to_string()),
        name: workspace.name,
        timezone: workspace.timezone,
        status: workspace.status,
        created_at: workspace.created_at.to_rfc3339(),
    }))
}

// ============ Update Workspace Name ============

#[derive(Deserialize)]
pub struct UpdateWorkspaceRequest {
    pub name: Option<String>,
    pub timezone: Option<String>,
}

pub async fn update_workspace(
    State(state): State<AppState>,
    WorkspaceAdminOrOrgAdmin { workspace, .. }: WorkspaceAdminOrOrgAdmin,
    Json(req): Json<UpdateWorkspaceRequest>,
) -> Result<Json<WorkspaceResponse>, WorkspaceError> {
    use sea_orm::EntityTrait;

    // Validate timezone if provided
    if let Some(ref tz) = req.timezone {
        if !AvailableTimezones.contains(&tz.as_str()) {
            return Err(WorkspaceError::InvalidTimezone);
        }
    }

    // Fetch the SeaORM entity
    let existing = entities::workspace::Entity::find_by_id(workspace.id)
        .one(&state.db_conn)
        .await
        .map_err(|e| WorkspaceError::Database(e.to_string()))?
        .ok_or(WorkspaceError::NotFound)?;

    let mut workspace_active: entities::workspace::ActiveModel = existing.into();

    if let Some(name) = req.name {
        workspace_active.name = Set(name);
    }
    if let Some(timezone) = req.timezone {
        workspace_active.timezone = Set(Some(timezone));
    }
    workspace_active.updated_at = Set(chrono::Utc::now().into());

    let updated = workspace_active
        .update(&state.db_conn)
        .await
        .map_err(|e| WorkspaceError::Database(e.to_string()))?;

    Ok(Json(WorkspaceResponse {
        id: updated.id.to_string(),
        organization_id: updated.organization_id.map(|id| id.to_string()),
        name: updated.name,
        timezone: updated.timezone,
        // The update leaves the status alone.
        status: workspace.status,
        created_at: updated.created_at.to_rfc3339(),
    }))
}

// ============ Get Available Timezones ============

pub async fn get_timezones() -> Json<Vec<&'static str>> {
    Json(AvailableTimezones.to_vec())
}

// ============ Get Workspace Tokens ============

#[derive(Serialize)]
pub struct WorkspaceTokenResponse {
    pub id: String,
    pub token: String,
    pub name: Option<String>,
    pub last_used_at: Option<String>,
    pub created_at: String,
}

pub async fn get_workspace_tokens(
    State(state): State<AppState>,
    WorkspaceAccess { workspace, .. }: WorkspaceAccess,
) -> Result<Json<Vec<WorkspaceTokenResponse>>, WorkspaceError> {
    let tokens = db::list_tokens_by_workspace(&state.db_pool, &workspace.id)
        .await
        .map_err(|e| WorkspaceError::Database(e.to_string()))?;

    let response: Vec<WorkspaceTokenResponse> = tokens
        .into_iter()
        .map(|t| WorkspaceTokenResponse {
            id: t.id.to_string(),
            token: t.token.to_string(),
            name: t.name,
            last_used_at: t.last_used_at.map(|dt| dt.to_rfc3339()),
            created_at: t.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(response))
}

// ============ Delete Workspace ============

pub async fn delete_workspace(
    State(state): State<AppState>,
    OrgAdmin { organization, .. }: OrgAdmin,
    headers: axum::http::HeaderMap,
) -> Result<StatusCode, WorkspaceError> {
    let workspace_id_str = headers
        .get("X-Workspace-Id")
        .and_then(|v| v.to_str().ok())
        .ok_or(WorkspaceError::InvalidWorkspaceId)?;

    let workspace_id =
        WorkspaceId::from_str(workspace_id_str).map_err(|_| WorkspaceError::InvalidWorkspaceId)?;

    // DELETING keeps provisioning away while the infrastructure is dropped.
    let start = db::start_deleting(
        &state.db_pool,
        &workspace_id,
        &organization.id,
        provisioning::STALE_CREATING_AFTER,
    )
    .await
    .map_err(|e| WorkspaceError::Database(e.to_string()))?;
    match start {
        DeletionStart::Started => {}
        DeletionStart::NotFound => return Err(WorkspaceError::NotFound),
        DeletionStart::BeingProvisioned => return Err(WorkspaceError::BeingProvisioned),
    }

    // Drop the workspace's infrastructure before the row, so a failed drop
    // leaves the workspace DELETING, to delete again.
    provisioning::deprovision_workspace(&state, &organization.id, &workspace_id)
        .await
        .map_err(|e| {
            WorkspaceError::Deprovision(format!("Failed to delete workspace data: {e}"))
        })?;

    db::soft_delete_workspace(&state.db_pool, &workspace_id)
        .await
        .map_err(|e| WorkspaceError::Database(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

// ============ Error Types ============

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("Invalid workspace ID")]
    InvalidWorkspaceId,
    #[error("Invalid timezone")]
    InvalidTimezone,
    #[error("Workspace not found")]
    NotFound,
    #[error("Workspace is being provisioned")]
    BeingProvisioned,
    #[error("Database error: {0}")]
    Database(String),
    #[error("{0}")]
    Deprovision(String),
    #[error(transparent)]
    Auth(#[from] crate::middleware::AuthError),
}

impl axum::response::IntoResponse for WorkspaceError {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;

        let (status, message): (StatusCode, String) = match self {
            WorkspaceError::InvalidWorkspaceId => {
                (StatusCode::BAD_REQUEST, "Invalid workspace ID".to_string())
            }
            WorkspaceError::InvalidTimezone => {
                (StatusCode::BAD_REQUEST, "Invalid timezone".to_string())
            }
            WorkspaceError::NotFound => (StatusCode::NOT_FOUND, "Workspace not found".to_string()),
            WorkspaceError::BeingProvisioned => (
                StatusCode::CONFLICT,
                "Workspace is being provisioned; try again in a few minutes".to_string(),
            ),
            WorkspaceError::Database(e) | WorkspaceError::Deprovision(e) => {
                tracing::error!("Workspace error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            WorkspaceError::Auth(e) => return e.into_response(),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

// available timezones
const AvailableTimezones: [&str; 597] = [
    "Africa/Abidjan",
    "Africa/Accra",
    "Africa/Addis_Ababa",
    "Africa/Algiers",
    "Africa/Asmara",
    "Africa/Asmera",
    "Africa/Bamako",
    "Africa/Bangui",
    "Africa/Banjul",
    "Africa/Bissau",
    "Africa/Blantyre",
    "Africa/Brazzaville",
    "Africa/Bujumbura",
    "Africa/Cairo",
    "Africa/Casablanca",
    "Africa/Ceuta",
    "Africa/Conakry",
    "Africa/Dakar",
    "Africa/Dar_es_Salaam",
    "Africa/Djibouti",
    "Africa/Douala",
    "Africa/El_Aaiun",
    "Africa/Freetown",
    "Africa/Gaborone",
    "Africa/Harare",
    "Africa/Johannesburg",
    "Africa/Juba",
    "Africa/Kampala",
    "Africa/Khartoum",
    "Africa/Kigali",
    "Africa/Kinshasa",
    "Africa/Lagos",
    "Africa/Libreville",
    "Africa/Lome",
    "Africa/Luanda",
    "Africa/Lubumbashi",
    "Africa/Lusaka",
    "Africa/Malabo",
    "Africa/Maputo",
    "Africa/Maseru",
    "Africa/Mbabane",
    "Africa/Mogadishu",
    "Africa/Monrovia",
    "Africa/Nairobi",
    "Africa/Ndjamena",
    "Africa/Niamey",
    "Africa/Nouakchott",
    "Africa/Ouagadougou",
    "Africa/Porto-Novo",
    "Africa/Sao_Tome",
    "Africa/Timbuktu",
    "Africa/Tripoli",
    "Africa/Tunis",
    "Africa/Windhoek",
    "America/Adak",
    "America/Anchorage",
    "America/Anguilla",
    "America/Antigua",
    "America/Araguaina",
    "America/Argentina/Buenos_Aires",
    "America/Argentina/Catamarca",
    "America/Argentina/ComodRivadavia",
    "America/Argentina/Cordoba",
    "America/Argentina/Jujuy",
    "America/Argentina/La_Rioja",
    "America/Argentina/Mendoza",
    "America/Argentina/Rio_Gallegos",
    "America/Argentina/Salta",
    "America/Argentina/San_Juan",
    "America/Argentina/San_Luis",
    "America/Argentina/Tucuman",
    "America/Argentina/Ushuaia",
    "America/Aruba",
    "America/Asuncion",
    "America/Atikokan",
    "America/Atka",
    "America/Bahia",
    "America/Bahia_Banderas",
    "America/Barbados",
    "America/Belem",
    "America/Belize",
    "America/Blanc-Sablon",
    "America/Boa_Vista",
    "America/Bogota",
    "America/Boise",
    "America/Buenos_Aires",
    "America/Cambridge_Bay",
    "America/Campo_Grande",
    "America/Cancun",
    "America/Caracas",
    "America/Catamarca",
    "America/Cayenne",
    "America/Cayman",
    "America/Chicago",
    "America/Chihuahua",
    "America/Ciudad_Juarez",
    "America/Coral_Harbour",
    "America/Cordoba",
    "America/Costa_Rica",
    "America/Creston",
    "America/Cuiaba",
    "America/Curacao",
    "America/Danmarkshavn",
    "America/Dawson",
    "America/Dawson_Creek",
    "America/Denver",
    "America/Detroit",
    "America/Dominica",
    "America/Edmonton",
    "America/Eirunepe",
    "America/El_Salvador",
    "America/Ensenada",
    "America/Fort_Nelson",
    "America/Fort_Wayne",
    "America/Fortaleza",
    "America/Glace_Bay",
    "America/Godthab",
    "America/Goose_Bay",
    "America/Grand_Turk",
    "America/Grenada",
    "America/Guadeloupe",
    "America/Guatemala",
    "America/Guayaquil",
    "America/Guyana",
    "America/Halifax",
    "America/Havana",
    "America/Hermosillo",
    "America/Indiana/Indianapolis",
    "America/Indiana/Knox",
    "America/Indiana/Marengo",
    "America/Indiana/Petersburg",
    "America/Indiana/Tell_City",
    "America/Indiana/Vevay",
    "America/Indiana/Vincennes",
    "America/Indiana/Winamac",
    "America/Indianapolis",
    "America/Inuvik",
    "America/Iqaluit",
    "America/Jamaica",
    "America/Jujuy",
    "America/Juneau",
    "America/Kentucky/Louisville",
    "America/Kentucky/Monticello",
    "America/Knox_IN",
    "America/Kralendijk",
    "America/La_Paz",
    "America/Lima",
    "America/Los_Angeles",
    "America/Louisville",
    "America/Lower_Princes",
    "America/Maceio",
    "America/Managua",
    "America/Manaus",
    "America/Marigot",
    "America/Martinique",
    "America/Matamoros",
    "America/Mazatlan",
    "America/Mendoza",
    "America/Menominee",
    "America/Merida",
    "America/Metlakatla",
    "America/Mexico_City",
    "America/Miquelon",
    "America/Moncton",
    "America/Monterrey",
    "America/Montevideo",
    "America/Montreal",
    "America/Montserrat",
    "America/Nassau",
    "America/New_York",
    "America/Nipigon",
    "America/Nome",
    "America/Noronha",
    "America/North_Dakota/Beulah",
    "America/North_Dakota/Center",
    "America/North_Dakota/New_Salem",
    "America/Nuuk",
    "America/Ojinaga",
    "America/Panama",
    "America/Pangnirtung",
    "America/Paramaribo",
    "America/Phoenix",
    "America/Port-au-Prince",
    "America/Port_of_Spain",
    "America/Porto_Acre",
    "America/Porto_Velho",
    "America/Puerto_Rico",
    "America/Punta_Arenas",
    "America/Rainy_River",
    "America/Rankin_Inlet",
    "America/Recife",
    "America/Regina",
    "America/Resolute",
    "America/Rio_Branco",
    "America/Rosario",
    "America/Santa_Isabel",
    "America/Santarem",
    "America/Santiago",
    "America/Santo_Domingo",
    "America/Sao_Paulo",
    "America/Scoresbysund",
    "America/Shiprock",
    "America/Sitka",
    "America/St_Barthelemy",
    "America/St_Johns",
    "America/St_Kitts",
    "America/St_Lucia",
    "America/St_Thomas",
    "America/St_Vincent",
    "America/Swift_Current",
    "America/Tegucigalpa",
    "America/Thule",
    "America/Thunder_Bay",
    "America/Tijuana",
    "America/Toronto",
    "America/Tortola",
    "America/Vancouver",
    "America/Virgin",
    "America/Whitehorse",
    "America/Winnipeg",
    "America/Yakutat",
    "America/Yellowknife",
    "Antarctica/Casey",
    "Antarctica/Davis",
    "Antarctica/DumontDUrville",
    "Antarctica/Macquarie",
    "Antarctica/Mawson",
    "Antarctica/McMurdo",
    "Antarctica/Palmer",
    "Antarctica/Rothera",
    "Antarctica/South_Pole",
    "Antarctica/Syowa",
    "Antarctica/Troll",
    "Antarctica/Vostok",
    "Arctic/Longyearbyen",
    "Asia/Aden",
    "Asia/Almaty",
    "Asia/Amman",
    "Asia/Anadyr",
    "Asia/Aqtau",
    "Asia/Aqtobe",
    "Asia/Ashgabat",
    "Asia/Ashkhabad",
    "Asia/Atyrau",
    "Asia/Baghdad",
    "Asia/Bahrain",
    "Asia/Baku",
    "Asia/Bangkok",
    "Asia/Barnaul",
    "Asia/Beirut",
    "Asia/Bishkek",
    "Asia/Brunei",
    "Asia/Calcutta",
    "Asia/Chita",
    "Asia/Choibalsan",
    "Asia/Chongqing",
    "Asia/Chungking",
    "Asia/Colombo",
    "Asia/Dacca",
    "Asia/Damascus",
    "Asia/Dhaka",
    "Asia/Dili",
    "Asia/Dubai",
    "Asia/Dushanbe",
    "Asia/Famagusta",
    "Asia/Gaza",
    "Asia/Harbin",
    "Asia/Hebron",
    "Asia/Ho_Chi_Minh",
    "Asia/Hong_Kong",
    "Asia/Hovd",
    "Asia/Irkutsk",
    "Asia/Istanbul",
    "Asia/Jakarta",
    "Asia/Jayapura",
    "Asia/Jerusalem",
    "Asia/Kabul",
    "Asia/Kamchatka",
    "Asia/Karachi",
    "Asia/Kashgar",
    "Asia/Kathmandu",
    "Asia/Katmandu",
    "Asia/Khandyga",
    "Asia/Kolkata",
    "Asia/Krasnoyarsk",
    "Asia/Kuala_Lumpur",
    "Asia/Kuching",
    "Asia/Kuwait",
    "Asia/Macao",
    "Asia/Macau",
    "Asia/Magadan",
    "Asia/Makassar",
    "Asia/Manila",
    "Asia/Muscat",
    "Asia/Nicosia",
    "Asia/Novokuznetsk",
    "Asia/Novosibirsk",
    "Asia/Omsk",
    "Asia/Oral",
    "Asia/Phnom_Penh",
    "Asia/Pontianak",
    "Asia/Pyongyang",
    "Asia/Qatar",
    "Asia/Qostanay",
    "Asia/Qyzylorda",
    "Asia/Rangoon",
    "Asia/Riyadh",
    "Asia/Saigon",
    "Asia/Sakhalin",
    "Asia/Samarkand",
    "Asia/Seoul",
    "Asia/Shanghai",
    "Asia/Singapore",
    "Asia/Srednekolymsk",
    "Asia/Taipei",
    "Asia/Tashkent",
    "Asia/Tbilisi",
    "Asia/Tehran",
    "Asia/Tel_Aviv",
    "Asia/Thimbu",
    "Asia/Thimphu",
    "Asia/Tokyo",
    "Asia/Tomsk",
    "Asia/Ujung_Pandang",
    "Asia/Ulaanbaatar",
    "Asia/Ulan_Bator",
    "Asia/Urumqi",
    "Asia/Ust-Nera",
    "Asia/Vientiane",
    "Asia/Vladivostok",
    "Asia/Yakutsk",
    "Asia/Yangon",
    "Asia/Yekaterinburg",
    "Asia/Yerevan",
    "Atlantic/Azores",
    "Atlantic/Bermuda",
    "Atlantic/Canary",
    "Atlantic/Cape_Verde",
    "Atlantic/Faeroe",
    "Atlantic/Faroe",
    "Atlantic/Jan_Mayen",
    "Atlantic/Madeira",
    "Atlantic/Reykjavik",
    "Atlantic/South_Georgia",
    "Atlantic/St_Helena",
    "Atlantic/Stanley",
    "Australia/ACT",
    "Australia/Adelaide",
    "Australia/Brisbane",
    "Australia/Broken_Hill",
    "Australia/Canberra",
    "Australia/Currie",
    "Australia/Darwin",
    "Australia/Eucla",
    "Australia/Hobart",
    "Australia/LHI",
    "Australia/Lindeman",
    "Australia/Lord_Howe",
    "Australia/Melbourne",
    "Australia/NSW",
    "Australia/North",
    "Australia/Perth",
    "Australia/Queensland",
    "Australia/South",
    "Australia/Sydney",
    "Australia/Tasmania",
    "Australia/Victoria",
    "Australia/West",
    "Australia/Yancowinna",
    "Brazil/Acre",
    "Brazil/DeNoronha",
    "Brazil/East",
    "Brazil/West",
    "CET",
    "CST6CDT",
    "Canada/Atlantic",
    "Canada/Central",
    "Canada/Eastern",
    "Canada/Mountain",
    "Canada/Newfoundland",
    "Canada/Pacific",
    "Canada/Saskatchewan",
    "Canada/Yukon",
    "Chile/Continental",
    "Chile/EasterIsland",
    "Cuba",
    "EET",
    "EST",
    "EST5EDT",
    "Egypt",
    "Eire",
    "Etc/GMT",
    "Etc/GMT+0",
    "Etc/GMT+1",
    "Etc/GMT+10",
    "Etc/GMT+11",
    "Etc/GMT+12",
    "Etc/GMT+2",
    "Etc/GMT+3",
    "Etc/GMT+4",
    "Etc/GMT+5",
    "Etc/GMT+6",
    "Etc/GMT+7",
    "Etc/GMT+8",
    "Etc/GMT+9",
    "Etc/GMT-0",
    "Etc/GMT-1",
    "Etc/GMT-10",
    "Etc/GMT-11",
    "Etc/GMT-12",
    "Etc/GMT-13",
    "Etc/GMT-14",
    "Etc/GMT-2",
    "Etc/GMT-3",
    "Etc/GMT-4",
    "Etc/GMT-5",
    "Etc/GMT-6",
    "Etc/GMT-7",
    "Etc/GMT-8",
    "Etc/GMT-9",
    "Etc/GMT0",
    "Etc/Greenwich",
    "Etc/UCT",
    "Etc/UTC",
    "Etc/Universal",
    "Etc/Zulu",
    "Europe/Amsterdam",
    "Europe/Andorra",
    "Europe/Astrakhan",
    "Europe/Athens",
    "Europe/Belfast",
    "Europe/Belgrade",
    "Europe/Berlin",
    "Europe/Bratislava",
    "Europe/Brussels",
    "Europe/Bucharest",
    "Europe/Budapest",
    "Europe/Busingen",
    "Europe/Chisinau",
    "Europe/Copenhagen",
    "Europe/Dublin",
    "Europe/Gibraltar",
    "Europe/Guernsey",
    "Europe/Helsinki",
    "Europe/Isle_of_Man",
    "Europe/Istanbul",
    "Europe/Jersey",
    "Europe/Kaliningrad",
    "Europe/Kiev",
    "Europe/Kirov",
    "Europe/Kyiv",
    "Europe/Lisbon",
    "Europe/Ljubljana",
    "Europe/London",
    "Europe/Luxembourg",
    "Europe/Madrid",
    "Europe/Malta",
    "Europe/Mariehamn",
    "Europe/Minsk",
    "Europe/Monaco",
    "Europe/Moscow",
    "Europe/Nicosia",
    "Europe/Oslo",
    "Europe/Paris",
    "Europe/Podgorica",
    "Europe/Prague",
    "Europe/Riga",
    "Europe/Rome",
    "Europe/Samara",
    "Europe/San_Marino",
    "Europe/Sarajevo",
    "Europe/Saratov",
    "Europe/Simferopol",
    "Europe/Skopje",
    "Europe/Sofia",
    "Europe/Stockholm",
    "Europe/Tallinn",
    "Europe/Tirane",
    "Europe/Tiraspol",
    "Europe/Ulyanovsk",
    "Europe/Uzhgorod",
    "Europe/Vaduz",
    "Europe/Vatican",
    "Europe/Vienna",
    "Europe/Vilnius",
    "Europe/Volgograd",
    "Europe/Warsaw",
    "Europe/Zagreb",
    "Europe/Zaporozhye",
    "Europe/Zurich",
    "Factory",
    "GB",
    "GB-Eire",
    "GMT",
    "GMT+0",
    "GMT-0",
    "GMT0",
    "Greenwich",
    "HST",
    "Hongkong",
    "Iceland",
    "Indian/Antananarivo",
    "Indian/Chagos",
    "Indian/Christmas",
    "Indian/Cocos",
    "Indian/Comoro",
    "Indian/Kerguelen",
    "Indian/Mahe",
    "Indian/Maldives",
    "Indian/Mauritius",
    "Indian/Mayotte",
    "Indian/Reunion",
    "Iran",
    "Israel",
    "Jamaica",
    "Japan",
    "Kwajalein",
    "Libya",
    "MET",
    "MST",
    "MST7MDT",
    "Mexico/BajaNorte",
    "Mexico/BajaSur",
    "Mexico/General",
    "NZ",
    "NZ-CHAT",
    "Navajo",
    "PRC",
    "PST8PDT",
    "Pacific/Apia",
    "Pacific/Auckland",
    "Pacific/Bougainville",
    "Pacific/Chatham",
    "Pacific/Chuuk",
    "Pacific/Easter",
    "Pacific/Efate",
    "Pacific/Enderbury",
    "Pacific/Fakaofo",
    "Pacific/Fiji",
    "Pacific/Funafuti",
    "Pacific/Galapagos",
    "Pacific/Gambier",
    "Pacific/Guadalcanal",
    "Pacific/Guam",
    "Pacific/Honolulu",
    "Pacific/Johnston",
    "Pacific/Kanton",
    "Pacific/Kiritimati",
    "Pacific/Kosrae",
    "Pacific/Kwajalein",
    "Pacific/Majuro",
    "Pacific/Marquesas",
    "Pacific/Midway",
    "Pacific/Nauru",
    "Pacific/Niue",
    "Pacific/Norfolk",
    "Pacific/Noumea",
    "Pacific/Pago_Pago",
    "Pacific/Palau",
    "Pacific/Pitcairn",
    "Pacific/Pohnpei",
    "Pacific/Ponape",
    "Pacific/Port_Moresby",
    "Pacific/Rarotonga",
    "Pacific/Saipan",
    "Pacific/Samoa",
    "Pacific/Tahiti",
    "Pacific/Tarawa",
    "Pacific/Tongatapu",
    "Pacific/Truk",
    "Pacific/Wake",
    "Pacific/Wallis",
    "Pacific/Yap",
    "Poland",
    "Portugal",
    "ROC",
    "ROK",
    "Singapore",
    "Turkey",
    "UCT",
    "US/Alaska",
    "US/Aleutian",
    "US/Arizona",
    "US/Central",
    "US/East-Indiana",
    "US/Eastern",
    "US/Hawaii",
    "US/Indiana-Starke",
    "US/Michigan",
    "US/Mountain",
    "US/Pacific",
    "US/Samoa",
    "UTC",
    "Universal",
    "W-SU",
    "WET",
    "Zulu",
];
