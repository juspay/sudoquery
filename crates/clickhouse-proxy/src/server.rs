//! The HTTP API of `clickhouse-proxy`, for dashboard-server and the CDP
//! dashboard. Everything is under `/v1/workspaces/{workspace_id}`:
//!
//! - `POST query`: run SQL as the workspace's ClickHouse user
//! - `GET events`, `GET events/today-count`, `GET events/{name}/properties`,
//!   `GET schema`: what the workspace's events look like
//! - `event-descriptions`, `property-descriptions`: what they mean
//! - `owners/{owner}/consoles`: each person's saved SQL consoles
//! - `PUT`/`DELETE database` and `user`: the workspace's ClickHouse objects
//!
//! Callers decide who may do what in which workspace before they call, so
//! this service is internal: with `CLICKHOUSE_PROXY_TOKEN` set, every request
//! but `/health` must carry it as a bearer token. A workspace reads only its
//! own database, as its own ClickHouse user.

use std::collections::BTreeMap;

use axum::{
    Router,
    extract::{Path, Query, Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Json, Response},
    routing::{get, post, put},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::clickhouse::{ClickHouse, ClickHouseError, GENERIC_MESSAGE};
use crate::provisioning::{self, check_topic, check_workspace_id};
use crate::store;

/// Longest error text returned from a failed provisioning statement.
const MAX_ERROR_TEXT: usize = 512;
/// Longest owner name: an email address fits.
const MAX_OWNER_LEN: usize = 320;

#[derive(Clone)]
pub struct ProxyState {
    pub clickhouse: ClickHouse,
    pub pool: PgPool,
    /// Brokers as ClickHouse reaches them, for workspace Kafka tables.
    pub kafka_brokers: String,
    /// Bearer token every request must carry; `None` accepts any caller.
    pub token: Option<String>,
}

pub fn router(state: ProxyState) -> Router {
    let workspace = Router::new()
        .route("/query", post(run_query))
        .route("/events", get(event_names))
        .route("/events/today-count", get(today_count))
        .route("/events/{event_name}/properties", get(event_properties))
        .route("/schema", get(schema))
        .route("/event-descriptions", get(list_event_descriptions))
        .route(
            "/event-descriptions/{event_name}",
            get(get_event_description).put(upsert_event_description),
        )
        .route("/property-descriptions", get(list_property_descriptions))
        .route(
            "/property-descriptions/{event_name}/{property_name}",
            get(get_property_description).put(upsert_property_description),
        )
        .route(
            "/owners/{owner}/consoles",
            get(list_consoles).post(create_console),
        )
        .route(
            "/owners/{owner}/consoles/{console_id}",
            get(get_console)
                .patch(update_console)
                .delete(delete_console),
        )
        .route("/database", put(create_database).delete(drop_database))
        .route("/user", put(create_user).delete(drop_user))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_token));

    Router::new()
        .route("/health", get(health))
        .nest("/v1/workspaces/{workspace_id}", workspace)
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
            return ApiError::Unauthorized.into_response();
        }
    }
    next.run(request).await
}

/// ClickHouse and Postgres reachable: 200, else 503; the body says which.
async fn health(State(state): State<ProxyState>) -> Response {
    let clickhouse = state.clickhouse.admin("SELECT 1").await.is_ok();
    let postgres = sqlx::query("SELECT 1").execute(&state.pool).await.is_ok();
    let label = |ok: bool| if ok { "connected" } else { "disconnected" };
    let status = if clickhouse && postgres {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(json!({ "clickhouse": label(clickhouse), "postgres": label(postgres) })),
    )
        .into_response()
}

fn workspace(id: &str) -> Result<&str, ApiError> {
    check_workspace_id(id).map_err(ApiError::BadRequest)?;
    Ok(id)
}

/// The rows of a ClickHouse JSON answer.
fn rows(answer: &Value) -> &[Value] {
    answer["data"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
}

fn column(answer: &Value, name: &str) -> Vec<String> {
    rows(answer)
        .iter()
        .filter_map(|row| row[name].as_str().map(str::to_owned))
        .collect()
}

// ============ Queries ============

#[derive(Deserialize)]
pub struct QueryRequest {
    pub query: String,
}

/// ClickHouse's JSON answer as it came: `meta`, `data`, `rows`, `statistics`.
async fn run_query(
    State(state): State<ProxyState>,
    Path(workspace_id): Path<String>,
    Json(req): Json<QueryRequest>,
) -> Result<Json<Value>, ApiError> {
    let workspace_id = workspace(&workspace_id)?;
    if req.query.trim().is_empty() {
        return Err(ApiError::BadRequest("Empty query".into()));
    }
    Ok(Json(
        state
            .clickhouse
            .query(workspace_id, &req.query, &[])
            .await?,
    ))
}

/// The names of the events the workspace has received.
async fn event_names(
    State(state): State<ProxyState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<String>>, ApiError> {
    let workspace_id = workspace(&workspace_id)?;
    let answer = state
        .clickhouse
        .query(
            workspace_id,
            "SELECT event_name FROM events GROUP BY event_name ORDER BY event_name",
            &[],
        )
        .await?;
    Ok(Json(column(&answer, "event_name")))
}

#[derive(Deserialize)]
pub struct TodayCountQuery {
    /// The day's boundaries are in this timezone; UTC when absent.
    #[serde(default)]
    pub timezone: Option<String>,
}

/// Events that occurred today, in the given timezone.
async fn today_count(
    State(state): State<ProxyState>,
    Path(workspace_id): Path<String>,
    Query(params): Query<TodayCountQuery>,
) -> Result<Json<Value>, ApiError> {
    let workspace_id = workspace(&workspace_id)?;
    let timezone = params.timezone.unwrap_or_else(|| "UTC".to_owned());
    let answer = state
        .clickhouse
        .query(
            workspace_id,
            "SELECT count() AS count FROM events \
             WHERE toDate(event_timestamp, {tz:String}) = toDate(now({tz:String}))",
            &[("tz", &timezone)],
        )
        .await?;
    // ClickHouse sends 64-bit integers as strings in JSON.
    let count = rows(&answer)
        .first()
        .and_then(|row| {
            row["count"]
                .as_u64()
                .or_else(|| row["count"].as_str()?.parse().ok())
        })
        .unwrap_or(0);
    Ok(Json(json!({ "count": count })))
}

/// The property paths the event has carried, sorted.
async fn event_properties(
    State(state): State<ProxyState>,
    Path((workspace_id, event_name)): Path<(String, String)>,
) -> Result<Json<Vec<String>>, ApiError> {
    let workspace_id = workspace(&workspace_id)?;
    let answer = state
        .clickhouse
        .query(
            workspace_id,
            "SELECT DISTINCT key FROM (SELECT properties FROM events WHERE event_name = {event_name:String}) \
             ARRAY JOIN JSONAllPaths(properties) AS key ORDER BY key",
            &[("event_name", &event_name)],
        )
        .await?;
    Ok(Json(column(&answer, "key")))
}

#[derive(Serialize)]
pub struct SchemaProperty {
    pub property: String,
    /// The ClickHouse type the path was stored with.
    #[serde(rename = "type")]
    pub kind: String,
    pub description: Option<String>,
}

#[derive(Serialize)]
pub struct SchemaEvent {
    pub event_name: String,
    pub description: Option<String>,
    pub properties: Vec<SchemaProperty>,
}

#[derive(Serialize)]
pub struct Schema {
    pub events: Vec<SchemaEvent>,
}

/// Every event the workspace has received, with its properties and what the
/// workspace says they mean: ClickHouse's schema catalog joined with the
/// descriptions this service keeps.
async fn schema(
    State(state): State<ProxyState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Schema>, ApiError> {
    let workspace_id = workspace(&workspace_id)?;
    let (names, catalog, events, properties) = tokio::try_join!(
        async {
            state
                .clickhouse
                .query(
                    workspace_id,
                    "SELECT event_name FROM events GROUP BY event_name",
                    &[],
                )
                .await
                .map_err(ApiError::from)
        },
        async {
            state
                .clickhouse
                .query(
                    workspace_id,
                    "SELECT DISTINCT event_name, property, type FROM event_schema_catalog \
                     ORDER BY event_name, property, type",
                    &[],
                )
                .await
                .map_err(ApiError::from)
        },
        async {
            store::list_event_descriptions(&state.pool, workspace_id)
                .await
                .map_err(ApiError::from)
        },
        async {
            store::list_property_descriptions(&state.pool, workspace_id, None)
                .await
                .map_err(ApiError::from)
        },
    )?;

    let event_descriptions: BTreeMap<String, String> = events
        .into_iter()
        .map(|d| (d.event_name, d.description))
        .collect();
    let property_descriptions: BTreeMap<(String, String), String> = properties
        .into_iter()
        .map(|d| ((d.event_name, d.property_name), d.description))
        .collect();

    // Every event name, whether or not it has properties; a path stored with
    // several types is listed once, with the first.
    let mut by_event: BTreeMap<String, Vec<SchemaProperty>> = column(&names, "event_name")
        .into_iter()
        .map(|name| (name, Vec::new()))
        .collect();
    for row in rows(&catalog) {
        let (Some(event_name), Some(property), Some(kind)) = (
            row["event_name"].as_str(),
            row["property"].as_str(),
            row["type"].as_str(),
        ) else {
            continue;
        };
        let listed = by_event.entry(event_name.to_owned()).or_default();
        if listed.iter().any(|p| p.property == property) {
            continue;
        }
        listed.push(SchemaProperty {
            property: property.to_owned(),
            kind: kind.to_owned(),
            description: property_descriptions
                .get(&(event_name.to_owned(), property.to_owned()))
                .cloned(),
        });
    }

    Ok(Json(Schema {
        events: by_event
            .into_iter()
            .map(|(event_name, properties)| SchemaEvent {
                description: event_descriptions.get(&event_name).cloned(),
                event_name,
                properties,
            })
            .collect(),
    }))
}

// ============ Event descriptions ============

#[derive(Deserialize)]
pub struct UpsertEventDescription {
    pub description: String,
}

#[derive(Serialize)]
pub struct EventDescriptionResponse {
    pub id: String,
    pub workspace_id: String,
    pub event_name: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<store::EventDescription> for EventDescriptionResponse {
    fn from(d: store::EventDescription) -> Self {
        Self {
            id: d.id.to_string(),
            workspace_id: d.workspace_id,
            event_name: d.event_name,
            description: d.description,
            created_at: d.created_at.to_rfc3339(),
            updated_at: d.updated_at.to_rfc3339(),
        }
    }
}

async fn list_event_descriptions(
    State(state): State<ProxyState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let workspace_id = workspace(&workspace_id)?;
    let descriptions: Vec<EventDescriptionResponse> =
        store::list_event_descriptions(&state.pool, workspace_id)
            .await?
            .into_iter()
            .map(Into::into)
            .collect();
    Ok(Json(json!({ "event_descriptions": descriptions })))
}

async fn get_event_description(
    State(state): State<ProxyState>,
    Path((workspace_id, event_name)): Path<(String, String)>,
) -> Result<Json<EventDescriptionResponse>, ApiError> {
    let workspace_id = workspace(&workspace_id)?;
    store::get_event_description(&state.pool, workspace_id, &event_name)
        .await?
        .map(|d| Json(d.into()))
        .ok_or(ApiError::NotFound("Event description not found"))
}

async fn upsert_event_description(
    State(state): State<ProxyState>,
    Path((workspace_id, event_name)): Path<(String, String)>,
    Json(req): Json<UpsertEventDescription>,
) -> Result<Json<EventDescriptionResponse>, ApiError> {
    let workspace_id = workspace(&workspace_id)?;
    let description =
        store::upsert_event_description(&state.pool, workspace_id, &event_name, &req.description)
            .await?;
    Ok(Json(description.into()))
}

// ============ Property descriptions ============

#[derive(Deserialize)]
pub struct UpsertPropertyDescription {
    pub property_type: String,
    pub description: String,
}

#[derive(Deserialize)]
pub struct ListPropertyDescriptionsQuery {
    #[serde(default)]
    pub event_name: Option<String>,
}

#[derive(Serialize)]
pub struct PropertyDescriptionResponse {
    pub id: String,
    pub workspace_id: String,
    pub event_name: String,
    pub property_name: String,
    pub property_type: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<store::PropertyDescription> for PropertyDescriptionResponse {
    fn from(d: store::PropertyDescription) -> Self {
        Self {
            id: d.id.to_string(),
            workspace_id: d.workspace_id,
            event_name: d.event_name,
            property_name: d.property_name,
            property_type: d.property_type,
            description: d.description,
            created_at: d.created_at.to_rfc3339(),
            updated_at: d.updated_at.to_rfc3339(),
        }
    }
}

async fn list_property_descriptions(
    State(state): State<ProxyState>,
    Path(workspace_id): Path<String>,
    Query(query): Query<ListPropertyDescriptionsQuery>,
) -> Result<Json<Value>, ApiError> {
    let workspace_id = workspace(&workspace_id)?;
    let descriptions: Vec<PropertyDescriptionResponse> =
        store::list_property_descriptions(&state.pool, workspace_id, query.event_name.as_deref())
            .await?
            .into_iter()
            .map(Into::into)
            .collect();
    Ok(Json(json!({ "property_descriptions": descriptions })))
}

async fn get_property_description(
    State(state): State<ProxyState>,
    Path((workspace_id, event_name, property_name)): Path<(String, String, String)>,
) -> Result<Json<PropertyDescriptionResponse>, ApiError> {
    let workspace_id = workspace(&workspace_id)?;
    store::get_property_description(&state.pool, workspace_id, &event_name, &property_name)
        .await?
        .map(|d| Json(d.into()))
        .ok_or(ApiError::NotFound("Property description not found"))
}

async fn upsert_property_description(
    State(state): State<ProxyState>,
    Path((workspace_id, event_name, property_name)): Path<(String, String, String)>,
    Json(req): Json<UpsertPropertyDescription>,
) -> Result<Json<PropertyDescriptionResponse>, ApiError> {
    let workspace_id = workspace(&workspace_id)?;
    let description = store::upsert_property_description(
        &state.pool,
        workspace_id,
        &event_name,
        &property_name,
        &req.property_type,
        &req.description,
    )
    .await?;
    Ok(Json(description.into()))
}

// ============ Consoles ============

#[derive(Deserialize)]
pub struct ConsoleRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub console: Option<String>,
}

#[derive(Serialize)]
pub struct ConsoleResponse {
    pub id: String,
    /// The owner, under the name dashboard-server's API has always used.
    pub user_id: String,
    pub workspace_id: String,
    pub name: Option<String>,
    pub console: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<store::Console> for ConsoleResponse {
    fn from(c: store::Console) -> Self {
        Self {
            id: c.id.to_string(),
            user_id: c.owner,
            workspace_id: c.workspace_id,
            name: c.name,
            console: c.console,
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Serialize)]
pub struct ConsoleListItem {
    pub id: String,
    pub name: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

fn owner(owner: &str) -> Result<&str, ApiError> {
    if owner.is_empty() || owner.len() > MAX_OWNER_LEN || owner.chars().any(char::is_control) {
        return Err(ApiError::BadRequest(format!(
            "owner must be 1-{MAX_OWNER_LEN} characters"
        )));
    }
    Ok(owner)
}

fn console_id(id: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("Invalid console ID".into()))
}

async fn list_consoles(
    State(state): State<ProxyState>,
    Path((workspace_id, owner_name)): Path<(String, String)>,
) -> Result<Json<Vec<ConsoleListItem>>, ApiError> {
    let consoles =
        store::list_consoles(&state.pool, workspace(&workspace_id)?, owner(&owner_name)?)
            .await?
            .into_iter()
            .map(|c| ConsoleListItem {
                id: c.id.to_string(),
                name: c.name,
                created_at: c.created_at.to_rfc3339(),
                updated_at: c.updated_at.to_rfc3339(),
            })
            .collect();
    Ok(Json(consoles))
}

async fn create_console(
    State(state): State<ProxyState>,
    Path((workspace_id, owner_name)): Path<(String, String)>,
    Json(req): Json<ConsoleRequest>,
) -> Result<(StatusCode, Json<ConsoleResponse>), ApiError> {
    let console = store::create_console(
        &state.pool,
        workspace(&workspace_id)?,
        owner(&owner_name)?,
        req.name.as_deref(),
        req.console.as_deref(),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(console.into())))
}

async fn get_console(
    State(state): State<ProxyState>,
    Path((workspace_id, owner_name, id)): Path<(String, String, String)>,
) -> Result<Json<ConsoleResponse>, ApiError> {
    store::get_console(
        &state.pool,
        workspace(&workspace_id)?,
        owner(&owner_name)?,
        console_id(&id)?,
    )
    .await?
    .map(|c| Json(c.into()))
    .ok_or(ApiError::NotFound("Console not found"))
}

async fn update_console(
    State(state): State<ProxyState>,
    Path((workspace_id, owner_name, id)): Path<(String, String, String)>,
    Json(req): Json<ConsoleRequest>,
) -> Result<Json<ConsoleResponse>, ApiError> {
    store::update_console(
        &state.pool,
        workspace(&workspace_id)?,
        owner(&owner_name)?,
        console_id(&id)?,
        req.name.as_deref(),
        req.console.as_deref(),
    )
    .await?
    .map(|c| Json(c.into()))
    .ok_or(ApiError::NotFound("Console not found"))
}

async fn delete_console(
    State(state): State<ProxyState>,
    Path((workspace_id, owner_name, id)): Path<(String, String, String)>,
) -> Result<StatusCode, ApiError> {
    let deleted = store::delete_console(
        &state.pool,
        workspace(&workspace_id)?,
        owner(&owner_name)?,
        console_id(&id)?,
    )
    .await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound("Console not found"))
    }
}

// ============ Provisioning ============

#[derive(Deserialize)]
pub struct CreateDatabaseRequest {
    /// The workspace's Kafka topic, which its Kafka table consumes.
    pub topic: String,
}

/// Creates the workspace's database; safe to repeat.
async fn create_database(
    State(state): State<ProxyState>,
    Path(workspace_id): Path<String>,
    Json(req): Json<CreateDatabaseRequest>,
) -> Result<StatusCode, ApiError> {
    let workspace_id = workspace(&workspace_id)?;
    check_topic(&req.topic).map_err(ApiError::BadRequest)?;
    provisioning::create_database(
        &state.clickhouse,
        workspace_id,
        &req.topic,
        &state.kafka_brokers,
    )
    .await
    .map_err(|e| provisioning_error(&state, e))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Drops the workspace's database and every event in it; safe to repeat.
async fn drop_database(
    State(state): State<ProxyState>,
    Path(workspace_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let workspace_id = workspace(&workspace_id)?;
    provisioning::drop_database(&state.clickhouse, workspace_id)
        .await
        .map_err(|e| provisioning_error(&state, e))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Creates the workspace's read-only user; safe to repeat.
async fn create_user(
    State(state): State<ProxyState>,
    Path(workspace_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let workspace_id = workspace(&workspace_id)?;
    provisioning::create_user(&state.clickhouse, workspace_id)
        .await
        .map_err(|e| provisioning_error(&state, e))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn drop_user(
    State(state): State<ProxyState>,
    Path(workspace_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let workspace_id = workspace(&workspace_id)?;
    provisioning::drop_user(&state.clickhouse, workspace_id)
        .await
        .map_err(|e| provisioning_error(&state, e))?;
    Ok(StatusCode::NO_CONTENT)
}

/// What failed, for the caller's record (dashboard-server keeps it as the
/// workspace's `provisioning_error`): without the workspace password, and
/// bounded.
fn provisioning_error(state: &ProxyState, error: ClickHouseError) -> ApiError {
    let text = state.clickhouse.redact(&error.to_string());
    let cut = match text.char_indices().nth(MAX_ERROR_TEXT) {
        Some((end, _)) => &text[..end],
        None => &text,
    };
    ApiError::Provisioning(cut.to_owned())
}

// ============ Errors ============

#[derive(Debug)]
pub enum ApiError {
    Unauthorized,
    BadRequest(String),
    NotFound(&'static str),
    ClickHouse(ClickHouseError),
    Provisioning(String),
    Database(sqlx::Error),
}

impl From<ClickHouseError> for ApiError {
    fn from(error: ClickHouseError) -> Self {
        ApiError::ClickHouse(error)
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        ApiError::Database(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "missing or invalid token".to_string(),
            ),
            ApiError::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            ApiError::NotFound(message) => (StatusCode::NOT_FOUND, message.to_string()),
            // The client's query was wrong: ClickHouse's own words, cleaned.
            ApiError::ClickHouse(error) if error.is_query_error() => {
                (StatusCode::BAD_REQUEST, error.public_message())
            }
            ApiError::ClickHouse(error) => {
                tracing::error!("ClickHouse error: {}", error);
                (StatusCode::BAD_GATEWAY, GENERIC_MESSAGE.to_string())
            }
            ApiError::Provisioning(message) => {
                tracing::error!("ClickHouse provisioning failed: {}", message);
                (StatusCode::BAD_GATEWAY, message)
            }
            ApiError::Database(error) => {
                tracing::error!("Database error: {}", error);
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
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;
    use wiremock::matchers::{basic_auth, body_string_contains, method, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::clickhouse::ClickHouseConfig;

    const BASE: &str = "/v1/workspaces/acme-shop";

    /// The proxy over a mock ClickHouse. Nothing listens behind the pool:
    /// these tests never reach Postgres.
    async fn app(token: Option<&str>) -> (Router, MockServer) {
        let server = MockServer::start().await;
        let clickhouse = ClickHouse::new(ClickHouseConfig {
            url: server.uri(),
            admin_url: format!("{}/admin", server.uri()),
            admin_user: "admin".into(),
            admin_password: "admin-pass".into(),
            workspace_password: "ws-pass".into(),
        });
        let pool = PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(300))
            .connect_lazy("postgres://nobody:nothing@127.0.0.1:9/none")
            .unwrap();
        let state = ProxyState {
            clickhouse,
            pool,
            kafka_brokers: "redpanda:9092".into(),
            token: token.map(str::to_owned),
        };
        (router(state), server)
    }

    fn request(verb: &str, uri: &str, body: Option<Value>) -> Request<Body> {
        let builder = Request::builder().method(verb).uri(uri);
        match body {
            Some(body) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
            None => builder.body(Body::empty()).unwrap(),
        }
    }

    async fn json_body(response: Response) -> Value {
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
    }

    #[tokio::test]
    async fn the_api_refuses_a_missing_or_wrong_token() {
        let (app, _server) = app(Some("secret")).await;
        for authorization in [None, Some("Bearer nope")] {
            let mut req = request(
                "POST",
                &format!("{BASE}/query"),
                Some(json!({ "query": "SELECT 1" })),
            );
            if let Some(value) = authorization {
                req.headers_mut()
                    .insert(header::AUTHORIZATION, value.parse().unwrap());
            }
            let response = app.clone().oneshot(req).await.unwrap();
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "{authorization:?}"
            );
        }
    }

    #[tokio::test]
    async fn a_query_runs_as_the_workspaces_user_in_its_database() {
        let (app, server) = app(None).await;
        Mock::given(method("POST"))
            .and(query_param("database", "acme-shop"))
            .and(basic_auth("acme-shop", "ws-pass"))
            .and(body_string_contains("SELECT 1 FORMAT JSON"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({ "rows": 1, "data": [] })),
            )
            .expect(1)
            .mount(&server)
            .await;

        let response = app
            .oneshot(request(
                "POST",
                &format!("{BASE}/query"),
                Some(json!({ "query": "SELECT 1;" })),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_body(response).await, json!({ "rows": 1, "data": [] }));
    }

    #[tokio::test]
    async fn a_bad_query_is_a_400_in_clickhouses_words_and_an_outage_is_a_generic_502() {
        let (app, server) = app(None).await;
        Mock::given(method("POST"))
            .and(body_string_contains("SELEC"))
            .respond_with(ResponseTemplate::new(400).set_body_string(
                "Code: 62. DB::Exception: Syntax error. (SYNTAX_ERROR) (version 24.8.1)",
            ))
            .mount(&server)
            .await;
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                &format!("{BASE}/query"),
                Some(json!({ "query": "SELEC" })),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            json_body(response).await,
            json!({ "error": "Code: 62. Syntax error. (SYNTAX_ERROR)" })
        );

        server.reset().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500).set_body_string(
                "Code: 516. DB::Exception: Authentication failed (AUTHENTICATION_FAILED)",
            ))
            .mount(&server)
            .await;
        let response = app
            .oneshot(request(
                "POST",
                &format!("{BASE}/query"),
                Some(json!({ "query": "SELECT 1" })),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(
            json_body(response).await,
            json!({ "error": "Query failed" })
        );
    }

    #[tokio::test]
    async fn values_travel_as_query_parameters_never_inside_the_sql() {
        let (app, server) = app(None).await;
        let evil = "x' OR 1=1 --";
        Mock::given(method("POST"))
            .and(query_param("param_event_name", evil))
            .and(body_string_contains("{event_name:String}"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{ "key": "cart.total" }, { "key": "plan" }]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let response = app
            .oneshot(request(
                "GET",
                &format!("{BASE}/events/x'%20OR%201=1%20--/properties"),
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_body(response).await, json!(["cart.total", "plan"]));
    }

    #[tokio::test]
    async fn todays_count_uses_the_timezone_and_reads_64_bit_counts() {
        let (app, server) = app(None).await;
        Mock::given(method("POST"))
            .and(query_param("param_tz", "Asia/Kolkata"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({ "data": [{ "count": "42" }] })),
            )
            .mount(&server)
            .await;

        let response = app
            .oneshot(request(
                "GET",
                &format!("{BASE}/events/today-count?timezone=Asia/Kolkata"),
                None,
            ))
            .await
            .unwrap();
        assert_eq!(json_body(response).await, json!({ "count": 42 }));
    }

    #[tokio::test]
    async fn a_workspace_id_that_is_not_safe_or_is_reserved_never_reaches_clickhouse() {
        let (app, server) = app(None).await;
        for id in ["default", "system", "Shop", "a%60b"] {
            let response = app
                .clone()
                .oneshot(request(
                    "POST",
                    &format!("/v1/workspaces/{id}/query"),
                    Some(json!({ "query": "SELECT 1" })),
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{id}");
        }
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn provisioning_builds_the_database_from_the_topic_as_admin() {
        let (app, server) = app(None).await;
        Mock::given(method("POST"))
            .and(basic_auth("admin", "admin-pass"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let response = app
            .clone()
            .oneshot(request(
                "PUT",
                &format!("{BASE}/database"),
                Some(json!({ "topic": "events.acme.acme-shop" })),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let statements: Vec<String> = server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .map(|r| String::from_utf8_lossy(&r.body).into_owned())
            .collect();
        assert_eq!(statements[0], "CREATE DATABASE IF NOT EXISTS `acme-shop`");
        assert!(
            statements
                .iter()
                .any(|s| s.contains("'events.acme.acme-shop'"))
        );

        let response = app
            .oneshot(request(
                "PUT",
                &format!("{BASE}/database"),
                Some(json!({ "topic": "a'b" })),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn a_failed_provisioning_step_reports_clickhouses_error_without_the_password() {
        let (app, server) = app(None).await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500).set_body_string(
                "Code: 497. DB::Exception: Not enough privileges for CREATE USER 'acme-shop' BY 'ws-pass'",
            ))
            .mount(&server)
            .await;

        let response = app
            .oneshot(request("PUT", &format!("{BASE}/user"), None))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let error = json_body(response).await["error"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(error.contains("Not enough privileges"), "{error}");
        assert!(!error.contains("ws-pass"), "{error}");
    }

    #[tokio::test]
    async fn health_reports_each_dependency() {
        let (app, server) = app(Some("secret")).await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string("1\n"))
            .mount(&server)
            .await;

        let response = app.oneshot(request("GET", "/health", None)).await.unwrap();
        // Postgres is unreachable in this test.
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            json_body(response).await,
            json!({ "clickhouse": "connected", "postgres": "disconnected" })
        );
    }
}
