use axum::{extract::State, http::StatusCode, response::Json};
use canonical_event::ProjectId;
use sea_orm::{ActiveModelTrait, Set};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::{
    AppState, clickhouse, db, entities,
    middleware::{AuthUser, OrgAdmin, OrgContext, ProjectAccess, ProjectAdminOrOrgAdmin},
};

// ============ Create Project ============

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateProjectRequest {
    pub name: String,
}

#[derive(Serialize)]
pub struct CreateProjectResponse {
    pub id: String,
    pub organization_id: Option<String>,
    pub name: String,
    pub project_token: String,
    pub created_at: String,
}

pub async fn create_project(
    State(state): State<AppState>,
    OrgAdmin {
        organization,
        auth_user,
    }: OrgAdmin,
    Json(req): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<CreateProjectResponse>), ProjectError> {
    let project = db::create_project(&state.db_pool, Some(&organization.id), &req.name)
        .await
        .map_err(|e| ProjectError::Database(e.to_string()))?;

    // Create ClickHouse user - fail the API if this fails
    if let Err(e) = clickhouse::create_project_user(
        &state.clickhouse_admin_url,
        &state.clickhouse_admin_user,
        &state.clickhouse_admin_password,
        &state.clickhouse_project_password,
        &project.id,
    )
    .await
    {
        // Clean up the project from DB since ClickHouse creation failed
        if let Err(cleanup_err) = db::soft_delete_project(&state.db_pool, &project.id).await {
            tracing::error!(
                "Failed to cleanup project {} after ClickHouse error: {}",
                project.id,
                cleanup_err
            );
        }
        return Err(ProjectError::ClickHouse(format!(
            "Failed to create ClickHouse user: {}",
            e
        )));
    }

    // Add the creator as project admin
    db::add_user_to_project(
        &state.db_pool,
        auth_user.user.id,
        &project.id,
        &db::ProjectRole::ProjectAdmin,
    )
    .await
    .map_err(|e| ProjectError::Database(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(CreateProjectResponse {
            id: project.id.to_string(),
            organization_id: project.organization_id.map(|id| id.to_string()),
            name: project.name,
            project_token: project.project_token.to_string(),
            created_at: project.created_at.to_rfc3339(),
        }),
    ))
}

// ============ List Projects (by Organization) ============

#[derive(Serialize)]
pub struct ProjectResponse {
    pub id: String,
    pub organization_id: Option<String>,
    pub name: String,
    pub timezone: Option<String>,
    pub created_at: String,
}

pub async fn list_projects(
    State(state): State<AppState>,
    OrgContext { organization, .. }: OrgContext,
    AuthUser { user, .. }: AuthUser,
) -> Result<Json<Vec<ProjectResponse>>, ProjectError> {
    let projects =
        db::list_user_projects_in_organization(&state.db_pool, user.id, &organization.id)
            .await
            .map_err(|e| ProjectError::Database(e.to_string()))?;

    let response: Vec<ProjectResponse> = projects
        .into_iter()
        .map(|p| ProjectResponse {
            id: p.id.to_string(),
            organization_id: p.organization_id.map(|id| id.to_string()),
            name: p.name,
            timezone: p.timezone,
            created_at: p.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(response))
}

// ============ List My Projects ============

pub async fn list_my_projects(
    State(state): State<AppState>,
    auth_user: crate::middleware::AuthUser,
) -> Result<Json<Vec<ProjectResponse>>, ProjectError> {
    let project_ids = db::list_user_projects(&state.db_pool, auth_user.user.id)
        .await
        .map_err(|e| ProjectError::Database(e.to_string()))?;

    let mut projects = Vec::new();
    for project_id in project_ids {
        if let Some(project) = db::get_project_by_id(&state.db_pool, &project_id)
            .await
            .map_err(|e| ProjectError::Database(e.to_string()))?
        {
            projects.push(ProjectResponse {
                id: project.id.to_string(),
                organization_id: project.organization_id.map(|id| id.to_string()),
                name: project.name,
                timezone: project.timezone,
                created_at: project.created_at.to_rfc3339(),
            });
        }
    }

    Ok(Json(projects))
}

// ============ Get Project ============

pub async fn get_project(
    ProjectAccess { project, .. }: ProjectAccess,
) -> Result<Json<ProjectResponse>, ProjectError> {
    Ok(Json(ProjectResponse {
        id: project.id.to_string(),
        organization_id: project.organization_id.map(|id| id.to_string()),
        name: project.name,
        timezone: project.timezone,
        created_at: project.created_at.to_rfc3339(),
    }))
}

// ============ Update Project Name ============

#[derive(Deserialize)]
pub struct UpdateProjectRequest {
    pub name: Option<String>,
    pub timezone: Option<String>,
}

pub async fn update_project(
    State(state): State<AppState>,
    ProjectAdminOrOrgAdmin { project, .. }: ProjectAdminOrOrgAdmin,
    Json(req): Json<UpdateProjectRequest>,
) -> Result<Json<ProjectResponse>, ProjectError> {
    use sea_orm::EntityTrait;

    // Validate timezone if provided
    if let Some(ref tz) = req.timezone {
        if !AvailableTimezones.contains(&tz.as_str()) {
            return Err(ProjectError::InvalidTimezone);
        }
    }

    // Fetch the SeaORM entity
    let proj = entities::project::Entity::find_by_id(project.id)
        .one(&state.db_conn)
        .await
        .map_err(|e| ProjectError::Database(e.to_string()))?
        .ok_or(ProjectError::NotFound)?;

    let mut project_active: entities::project::ActiveModel = proj.into();

    if let Some(name) = req.name {
        project_active.name = Set(name);
    }
    if let Some(timezone) = req.timezone {
        project_active.timezone = Set(Some(timezone));
    }
    project_active.updated_at = Set(chrono::Utc::now().into());

    let updated = project_active
        .update(&state.db_conn)
        .await
        .map_err(|e| ProjectError::Database(e.to_string()))?;

    Ok(Json(ProjectResponse {
        id: updated.id.to_string(),
        organization_id: updated.organization_id.map(|id| id.to_string()),
        name: updated.name,
        timezone: updated.timezone,
        created_at: updated.created_at.to_rfc3339(),
    }))
}

// ============ Get Available Timezones ============

pub async fn get_timezones() -> Json<Vec<&'static str>> {
    Json(AvailableTimezones.to_vec())
}

// ============ Get Project Tokens ============

#[derive(Serialize)]
pub struct ProjectTokenResponse {
    pub id: String,
    pub token: String,
    pub name: Option<String>,
    pub last_used_at: Option<String>,
    pub created_at: String,
}

pub async fn get_project_tokens(
    State(state): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
) -> Result<Json<Vec<ProjectTokenResponse>>, ProjectError> {
    let tokens = db::list_tokens_by_project(&state.db_pool, &project.id)
        .await
        .map_err(|e| ProjectError::Database(e.to_string()))?;

    let response: Vec<ProjectTokenResponse> = tokens
        .into_iter()
        .map(|t| ProjectTokenResponse {
            id: t.id.to_string(),
            token: t.token.to_string(),
            name: t.name,
            last_used_at: t.last_used_at.map(|dt| dt.to_rfc3339()),
            created_at: t.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(response))
}

// ============ Delete Project ============

pub async fn delete_project(
    State(state): State<AppState>,
    OrgAdmin { organization, .. }: OrgAdmin,
    headers: axum::http::HeaderMap,
) -> Result<StatusCode, ProjectError> {
    let project_id_str = headers
        .get("X-Project-Id")
        .and_then(|v| v.to_str().ok())
        .ok_or(ProjectError::InvalidProjectId)?;

    let project_id =
        ProjectId::from_str(project_id_str).map_err(|_| ProjectError::InvalidProjectId)?;

    // Verify project belongs to organization
    let project = db::get_project_by_id(&state.db_pool, &project_id)
        .await
        .map_err(|e| ProjectError::Database(e.to_string()))?
        .ok_or(ProjectError::NotFound)?;

    if project.organization_id != Some(organization.id) {
        return Err(ProjectError::NotFound);
    }

    db::soft_delete_project(&state.db_pool, &project_id)
        .await
        .map_err(|e| ProjectError::Database(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

// ============ Error Types ============

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("Invalid project ID")]
    InvalidProjectId,
    #[error("Invalid timezone")]
    InvalidTimezone,
    #[error("Project not found")]
    NotFound,
    #[error("Database error: {0}")]
    Database(String),
    #[error("ClickHouse error: {0}")]
    ClickHouse(String),
    #[error(transparent)]
    Auth(#[from] crate::middleware::AuthError),
}

impl axum::response::IntoResponse for ProjectError {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;

        let (status, message): (StatusCode, String) = match self {
            ProjectError::InvalidProjectId => {
                (StatusCode::BAD_REQUEST, "Invalid project ID".to_string())
            }
            ProjectError::InvalidTimezone => {
                (StatusCode::BAD_REQUEST, "Invalid timezone".to_string())
            }
            ProjectError::NotFound => (StatusCode::NOT_FOUND, "Project not found".to_string()),
            ProjectError::Database(e) => (StatusCode::INTERNAL_SERVER_ERROR, e),
            ProjectError::ClickHouse(e) => (StatusCode::INTERNAL_SERVER_ERROR, e),
            ProjectError::Auth(e) => return e.into_response(),
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
