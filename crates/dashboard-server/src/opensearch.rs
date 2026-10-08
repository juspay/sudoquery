//! Read side of OpenSearch: queries the events `sink-opensearch` indexed.
//!
//! Every query goes through the same steps: check the client's query, wrap it
//! in the project's scope, have OpenSearch validate it, then run it.

use std::time::Duration;

use canonical_event::CanonicalEvent;
use chrono::{DateTime, Utc};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::db;

pub mod cursor;
pub mod query;

pub use cursor::{CursorPage, QueryRequest};
pub use query::SortOrder;

/// Longest piece of OpenSearch output kept in an error.
const MAX_ERROR_TEXT: usize = 512;

/// Sort key of an event: `[occured_at, id]`.
const EVENT_KEY_LEN: usize = 2;

/// What a project may read: the events of its organization (`org_id`) that
/// were sent for the project itself (`project_id`).
#[derive(Clone, Debug, PartialEq)]
pub struct Scope {
    pub org_id: String,
    pub project_id: String,
}

impl Scope {
    /// `None` when the project has no organization, and therefore no `org_id`.
    pub fn for_project(project: &db::Project) -> Option<Self> {
        Some(Self {
            org_id: project.organization_id.as_ref()?.to_string(),
            project_id: project.id.to_string(),
        })
    }
}

/// A request the client got wrong.
#[derive(Debug, PartialEq, thiserror::Error)]
pub enum RequestError {
    #[error("`query` must be a JSON object")]
    QueryNotAnObject,

    #[error("`query` must not contain `{0}`")]
    ForbiddenKey(String),

    #[error("`query` filters on an `org_id` this project cannot read")]
    WrongOrg,

    #[error("`query` must include a `range` on `occured_at` with a lower and an upper bound")]
    MissingTimeRange,

    #[error("invalid `cursor`")]
    InvalidCursor,

    #[error("`page_size` must be between 1 and {}", cursor::MAX_PAGE_SIZE)]
    InvalidPageSize,

    #[error("`from` must be before `to`, and at most 60 years before it")]
    InvalidTimeRange,

    #[error("`field` must be one of: {}", query::FACET_FIELDS.join(", "))]
    UnknownFacetField,

    #[error("`size` must be between 1 and {}", query::MAX_FACET_SIZE)]
    InvalidFacetSize,

    /// OpenSearch could not parse or run the query.
    #[error("invalid query: {0}")]
    InvalidQuery(String),
}

#[derive(Debug, thiserror::Error)]
pub enum OpenSearchError {
    #[error(transparent)]
    Request(#[from] RequestError),

    #[error("OpenSearch request failed: {0}")]
    Http(String),

    #[error("OpenSearch returned HTTP {status}: {body}")]
    Upstream { status: u16, body: String },

    #[error("unexpected OpenSearch response: {0}")]
    Decode(String),
}

/// An event as the API returns it: a `CanonicalEvent` without `org_id`,
/// which stays internal.
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct Event(Map<String, Value>);

impl Event {
    fn new(event: &CanonicalEvent) -> Result<Self, OpenSearchError> {
        match serde_json::to_value(event) {
            Ok(Value::Object(mut fields)) => {
                fields.remove("org_id");
                Ok(Self(fields))
            }
            Ok(_) => Err(decode_error("an event must serialize to an object")),
            Err(error) => Err(decode_error(error)),
        }
    }
}

/// A session, summarised over the events the query matched.
#[derive(Debug, PartialEq, Serialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub first_event_at: DateTime<Utc>,
    pub last_event_at: DateTime<Utc>,
    pub event_count: u64,
}

/// Event counts over a time range, split into equal buckets.
#[derive(Debug, PartialEq, Serialize)]
pub struct Histogram {
    /// Width of a bucket, as an OpenSearch interval such as `5m`.
    pub interval: &'static str,
    pub buckets: Vec<HistogramBucket>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct HistogramBucket {
    /// Start of the bucket.
    pub time: DateTime<Utc>,
    pub count: u64,
}

/// The most frequent values of one field among the events a query matched.
#[derive(Debug, PartialEq, Serialize)]
pub struct Facets {
    pub field: String,
    /// Events the query matched.
    pub total: u64,
    /// Events whose value is not among `buckets`.
    pub other: u64,
    pub buckets: Vec<FacetBucket>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct FacetBucket {
    pub value: String,
    pub count: u64,
}

/// The index a request reads, with the scope it is read under.
struct Target<'a> {
    index: String,
    scope: &'a Scope,
}

impl Target<'_> {
    /// Text from OpenSearch, safe to show to the client.
    fn redact(&self, text: &str) -> String {
        // Redact before cutting, or a cut could leave half an org id.
        excerpt(&query::redact(text, &self.index, self.scope)).to_owned()
    }
}

#[derive(Clone)]
pub struct OpenSearch {
    http: reqwest::Client,
    base_url: String,
    credentials: Option<(String, String)>,
    index_template: String,
}

impl OpenSearch {
    /// `index_template` names the index to read; `{org_id}` in it is
    /// replaced per request, as `sink-opensearch` does when writing.
    pub fn new(
        url: &str,
        credentials: Option<(String, String)>,
        index_template: String,
        request_timeout: Duration,
    ) -> Result<Self, reqwest::Error> {
        let http = reqwest::Client::builder()
            .timeout(request_timeout)
            .build()?;

        Ok(Self {
            http,
            base_url: url.trim_end_matches('/').to_owned(),
            credentials,
            index_template,
        })
    }

    /// One page of events, in the request's time order or `default_order`.
    /// `session_id` limits the page to one session.
    pub async fn search_events(
        &self,
        scope: &Scope,
        request: &QueryRequest,
        session_id: Option<&str>,
        default_order: SortOrder,
    ) -> Result<CursorPage<Event>, OpenSearchError> {
        let order = request.order.unwrap_or(default_order);
        let page_size = cursor::page_size(request.page_size)?;
        let after = request
            .cursor
            .as_deref()
            .map(|cursor| cursor::decode(cursor, EVENT_KEY_LEN))
            .transpose()?;

        let target = self.target(scope);
        let narrowing = session_id.map(query::session_filter).into_iter().collect();
        let Some(query) = self
            .prepare(&target, request.query.as_ref(), narrowing)
            .await?
        else {
            return Ok(CursorPage::empty());
        };
        let body = query::events_body(query, order, page_size, after);
        let Some(reply) = self
            .call(Method::POST, &target, "_search", Some(&body))
            .await?
        else {
            return Ok(CursorPage::empty());
        };

        let rows = array(&reply["hits"]["hits"], "hits")?
            .iter()
            .map(event_row)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(CursorPage::from_rows(rows, page_size))
    }

    pub async fn count(
        &self,
        scope: &Scope,
        query: Option<&Value>,
    ) -> Result<u64, OpenSearchError> {
        let target = self.target(scope);
        let Some(query) = self.prepare(&target, query, Vec::new()).await? else {
            return Ok(0);
        };
        let body = json!({ "query": query });
        let Some(reply) = self
            .call(Method::POST, &target, "_count", Some(&body))
            .await?
        else {
            return Ok(0);
        };

        reply["count"]
            .as_u64()
            .ok_or_else(|| decode_error("no `count` in the count response"))
    }

    /// The event with this id, if the project may read it.
    pub async fn get_event(
        &self,
        scope: &Scope,
        id: &str,
    ) -> Result<Option<Event>, OpenSearchError> {
        let target = self.target(scope);
        let api = format!("_doc/{}", urlencoding::encode(id));
        let Some(reply) = self.call(Method::GET, &target, &api, None).await? else {
            return Ok(None);
        };
        if reply["found"].as_bool() != Some(true) {
            return Ok(None);
        }

        let event = CanonicalEvent::deserialize(&reply["_source"]).map_err(decode_error)?;
        // `_doc` takes no query, so the scope is checked on the document.
        let in_scope = event.org_id == scope.org_id
            && event.project_id.as_deref() == Some(scope.project_id.as_str());
        if !in_scope {
            return Ok(None);
        }
        Event::new(&event).map(Some)
    }

    /// One page of sessions, ordered by session id. The query must be bounded
    /// in time; each summary covers the events inside that range.
    pub async fn list_sessions(
        &self,
        scope: &Scope,
        request: &QueryRequest,
    ) -> Result<CursorPage<SessionSummary>, OpenSearchError> {
        query::require_time_range(request.query.as_ref())?;
        let page_size = cursor::page_size(request.page_size)?;
        let after = request.cursor.as_deref().map(session_cursor).transpose()?;

        let target = self.target(scope);
        let Some(query) = self
            .prepare(&target, request.query.as_ref(), Vec::new())
            .await?
        else {
            return Ok(CursorPage::empty());
        };
        let body = query::sessions_body(query, page_size, after.as_deref());
        let Some(reply) = self
            .call(Method::POST, &target, "_search", Some(&body))
            .await?
        else {
            return Ok(CursorPage::empty());
        };

        let rows = array(
            &reply["aggregations"]["sessions"]["buckets"],
            "session buckets",
        )?
        .iter()
        .map(session_row)
        .collect::<Result<Vec<_>, _>>()?;
        Ok(CursorPage::from_rows(rows, page_size))
    }

    /// Event counts over `[from, to)`, in buckets sized to the range.
    pub async fn histogram(
        &self,
        scope: &Scope,
        query: Option<&Value>,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Histogram, OpenSearchError> {
        let interval = query::histogram_interval(from, to)?;
        let empty = || Histogram {
            interval,
            buckets: Vec::new(),
        };

        let target = self.target(scope);
        let narrowing = vec![query::time_filter(from, to)];
        let Some(query) = self.prepare(&target, query, narrowing).await? else {
            return Ok(empty());
        };
        let body = query::histogram_body(query, from, to, interval);
        let Some(reply) = self
            .call(Method::POST, &target, "_search", Some(&body))
            .await?
        else {
            return Ok(empty());
        };

        let buckets = array(
            &reply["aggregations"]["events"]["buckets"],
            "histogram buckets",
        )?
        .iter()
        .map(histogram_bucket)
        .collect::<Result<_, _>>()?;
        Ok(Histogram { interval, buckets })
    }

    /// The most frequent values of `field`, optionally only those starting
    /// with `prefix`. Like the session list, the query must be bounded in
    /// time.
    pub async fn facets(
        &self,
        scope: &Scope,
        query: Option<&Value>,
        field: &str,
        size: Option<usize>,
        prefix: Option<&str>,
    ) -> Result<Facets, OpenSearchError> {
        let size = query::facet_size(field, size)?;
        query::require_time_range(query)?;
        let facets = |total, other, buckets| Facets {
            field: field.to_owned(),
            total,
            other,
            buckets,
        };

        let target = self.target(scope);
        let narrowing = prefix
            .filter(|prefix| !prefix.is_empty())
            .map(|prefix| query::prefix_filter(field, prefix))
            .into_iter()
            .collect();
        let Some(query) = self.prepare(&target, query, narrowing).await? else {
            return Ok(facets(0, 0, Vec::new()));
        };
        let body = query::facets_body(query, field, size);
        let Some(reply) = self
            .call(Method::POST, &target, "_search", Some(&body))
            .await?
        else {
            return Ok(facets(0, 0, Vec::new()));
        };

        let values = &reply["aggregations"]["values"];
        let buckets = array(&values["buckets"], "facet buckets")?
            .iter()
            .map(facet_bucket)
            .collect::<Result<_, _>>()?;
        let total = count_of(&reply["hits"]["total"]["value"], "hit total")?;
        let other = count_of(&values["sum_other_doc_count"], "other count")?;
        Ok(facets(total, other, buckets))
    }

    fn target<'a>(&self, scope: &'a Scope) -> Target<'a> {
        Target {
            index: self.index_template.replace("{org_id}", &scope.org_id),
            scope,
        }
    }

    /// Checks the client's query, scopes it and has OpenSearch validate the
    /// result. `None` means the index does not exist yet.
    async fn prepare(
        &self,
        target: &Target<'_>,
        client: Option<&Value>,
        narrowing: Vec<Value>,
    ) -> Result<Option<Value>, OpenSearchError> {
        if let Some(client) = client {
            query::check_query(client, target.scope)?;
        }
        let scoped = query::scoped_query(client, target.scope, narrowing);

        let body = json!({ "query": &scoped });
        let Some(reply) = self
            .call(
                Method::POST,
                target,
                "_validate/query?explain=true",
                Some(&body),
            )
            .await?
        else {
            return Ok(None);
        };
        if reply["valid"].as_bool() != Some(true) {
            let reason = target.redact(validation_error(&reply));
            return Err(RequestError::InvalidQuery(reason).into());
        }
        Ok(Some(scoped))
    }

    /// Calls `{index}/{api}`. `None` means OpenSearch answered 404: the
    /// index, or the document asked for, does not exist.
    async fn call(
        &self,
        method: Method,
        target: &Target<'_>,
        api: &str,
        body: Option<&Value>,
    ) -> Result<Option<Value>, OpenSearchError> {
        let url = format!("{}/{}/{api}", self.base_url, target.index);
        let mut request = self.http.request(method, url);
        if let Some((username, password)) = &self.credentials {
            request = request.basic_auth(username, Some(password));
        }
        if let Some(body) = body {
            request = request.json(body);
        }

        let response = request
            .send()
            .await
            .map_err(|error| OpenSearchError::Http(error.without_url().to_string()))?;
        let status = response.status().as_u16();
        let text = response
            .text()
            .await
            .map_err(|error| OpenSearchError::Http(error.without_url().to_string()))?;

        match status {
            200..=299 => serde_json::from_str(&text).map(Some).map_err(decode_error),
            404 => Ok(None),
            400 => {
                let reason = target.redact(&rejection_reason(&text));
                Err(RequestError::InvalidQuery(reason).into())
            }
            _ => Err(OpenSearchError::Upstream {
                status,
                body: excerpt(&text).to_owned(),
            }),
        }
    }
}

fn event_row(hit: &Value) -> Result<(Event, Vec<Value>), OpenSearchError> {
    let event = CanonicalEvent::deserialize(&hit["_source"]).map_err(decode_error)?;
    let key = array(&hit["sort"], "sort values")?.clone();
    Ok((Event::new(&event)?, key))
}

fn session_row(bucket: &Value) -> Result<(SessionSummary, Vec<Value>), OpenSearchError> {
    let session_id = bucket["key"]["session_id"]
        .as_str()
        .ok_or_else(|| decode_error("a session bucket has no `session_id` key"))?;
    let summary = SessionSummary {
        session_id: session_id.to_owned(),
        first_event_at: instant(&bucket["first_event_at"])?,
        last_event_at: instant(&bucket["last_event_at"])?,
        event_count: count_of(&bucket["doc_count"], "session event count")?,
    };
    Ok((summary, vec![Value::from(session_id)]))
}

fn histogram_bucket(bucket: &Value) -> Result<HistogramBucket, OpenSearchError> {
    let time = bucket["key"]
        .as_i64()
        .and_then(DateTime::from_timestamp_millis)
        .ok_or_else(|| decode_error("a histogram bucket has no time"))?;
    Ok(HistogramBucket {
        time,
        count: count_of(&bucket["doc_count"], "histogram count")?,
    })
}

fn facet_bucket(bucket: &Value) -> Result<FacetBucket, OpenSearchError> {
    // Booleans come back as 1 / 0 with the readable form next to it.
    let value = match (&bucket["key_as_string"], &bucket["key"]) {
        (Value::String(text), _) | (_, Value::String(text)) => text.clone(),
        (_, Value::Null) => return Err(decode_error("a facet bucket has no value")),
        (_, other) => other.to_string(),
    };
    Ok(FacetBucket {
        value,
        count: count_of(&bucket["doc_count"], "facet count")?,
    })
}

fn count_of(value: &Value, what: &str) -> Result<u64, OpenSearchError> {
    value
        .as_u64()
        .ok_or_else(|| decode_error(format!("no {what} in the search response")))
}

/// A `min` / `max` over a date field: milliseconds since the epoch.
fn instant(metric: &Value) -> Result<DateTime<Utc>, OpenSearchError> {
    metric["value"]
        .as_f64()
        .and_then(|millis| DateTime::from_timestamp_millis(millis as i64))
        .ok_or_else(|| decode_error("a session bucket has no event time"))
}

/// The session id a session-list cursor resumes after.
fn session_cursor(cursor: &str) -> Result<String, RequestError> {
    match cursor::decode(cursor, 1)?.pop() {
        Some(Value::String(session_id)) => Ok(session_id),
        _ => Err(RequestError::InvalidCursor),
    }
}

fn array<'a>(value: &'a Value, what: &str) -> Result<&'a Vec<Value>, OpenSearchError> {
    value
        .as_array()
        .ok_or_else(|| decode_error(format!("no {what} in the search response")))
}

/// Why `_validate/query` refused a query: the per-index explanation, or the
/// top-level error when the query did not parse at all.
fn validation_error(reply: &Value) -> &str {
    reply["explanations"]
        .as_array()
        .into_iter()
        .flatten()
        .find_map(|explanation| explanation["error"].as_str())
        .or_else(|| reply["error"].as_str())
        .unwrap_or("OpenSearch could not parse the query")
}

/// The reason in an OpenSearch error body.
fn rejection_reason(body: &str) -> String {
    let body: Value = serde_json::from_str(body).unwrap_or_default();
    let error = &body["error"];
    error["root_cause"][0]["reason"]
        .as_str()
        .or_else(|| error["reason"].as_str())
        .or_else(|| error.as_str())
        .unwrap_or("OpenSearch rejected the request")
        .to_owned()
}

fn decode_error(error: impl ToString) -> OpenSearchError {
    OpenSearchError::Decode(error.to_string())
}

fn excerpt(text: &str) -> &str {
    match text.char_indices().nth(MAX_ERROR_TEXT) {
        Some((end, _)) => &text[..end],
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{basic_auth, body_json, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    const ORG: &str = "0198c0de-0000-7000-8000-000000000001";
    const PROJ: &str = "0198c0de-0000-7000-8000-000000000002";
    const INDEX: &str = "events-0198c0de-0000-7000-8000-000000000001";

    fn scope() -> Scope {
        Scope {
            org_id: ORG.into(),
            project_id: PROJ.into(),
        }
    }

    fn client(server: &MockServer) -> OpenSearch {
        OpenSearch::new(
            &server.uri(),
            None,
            "events-{org_id}".into(),
            Duration::from_secs(2),
        )
        .unwrap()
    }

    fn request(query: Option<Value>) -> QueryRequest {
        QueryRequest {
            query,
            ..QueryRequest::default()
        }
    }

    fn api(name: &str) -> String {
        format!("/{INDEX}/{name}")
    }

    /// A stored event, the way `sink-opensearch` writes it.
    fn source(n: u32, proj: &str) -> Value {
        json!({
            "envelop_version": "1.0",
            "id": format!("00000000-0000-4000-8000-{n:012}"),
            "name": "checkout_viewed",
            "occured_at": format!("2026-09-29T09:50:{n:02}Z"),
            "org_id": ORG,
            "project_id": proj,
            "session_id": "session-1",
            "anon_id": "anon-1",
            "properties": { "plan": "pro" },
        })
    }

    fn hit(n: u32) -> Value {
        let source = source(n, PROJ);
        json!({ "_id": source["id"], "_source": source, "sort": [n, source["id"]] })
    }

    fn hits(numbers: &[u32]) -> ResponseTemplate {
        let hits: Vec<Value> = numbers.iter().copied().map(hit).collect();
        ResponseTemplate::new(200).set_body_json(json!({ "hits": { "hits": hits } }))
    }

    async fn mount(server: &MockServer, verb: &str, api_name: &str, response: ResponseTemplate) {
        Mock::given(method(verb))
            .and(path(api(api_name)))
            .respond_with(response)
            .mount(server)
            .await;
    }

    async fn accept_any_query(server: &MockServer) {
        let valid = ResponseTemplate::new(200).set_body_json(json!({ "valid": true }));
        mount(server, "POST", "_validate/query", valid).await;
    }

    async fn requested_paths(server: &MockServer) -> Vec<String> {
        let requests = server.received_requests().await.unwrap();
        requests
            .iter()
            .map(|request| request.url.path().to_owned())
            .collect()
    }

    async fn search(server: &MockServer, request: &QueryRequest) -> CursorPage<Event> {
        client(server)
            .search_events(&scope(), request, None, SortOrder::Desc)
            .await
            .unwrap()
    }

    fn items<T: Serialize>(page: &CursorPage<T>) -> Value {
        serde_json::to_value(&page.items).unwrap()
    }

    #[tokio::test]
    async fn a_search_validates_the_scoped_query_then_runs_it() {
        let server = MockServer::start().await;
        let client_query = json!({ "term": { "name": "checkout_viewed" } });
        let scoped = query::scoped_query(Some(&client_query), &scope(), Vec::new());
        Mock::given(method("POST"))
            .and(path(api("_validate/query")))
            .and(query_param("explain", "true"))
            .and(body_json(json!({ "query": scoped })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "valid": true })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path(api("_search")))
            .and(body_json(query::events_body(
                scoped.clone(),
                SortOrder::Desc,
                cursor::DEFAULT_PAGE_SIZE,
                None,
            )))
            .respond_with(hits(&[2, 1]))
            .expect(1)
            .mount(&server)
            .await;

        let page = search(&server, &request(Some(client_query))).await;

        assert_eq!(page.items.len(), 2);
        assert_eq!(page.next_cursor, None);
        assert_eq!(
            requested_paths(&server).await,
            [api("_validate/query"), api("_search")]
        );
    }

    #[tokio::test]
    async fn events_are_returned_without_their_org_id() {
        let server = MockServer::start().await;
        accept_any_query(&server).await;
        mount(&server, "POST", "_search", hits(&[1])).await;

        let page = search(&server, &request(None)).await;

        let mut expected = source(1, PROJ);
        expected.as_object_mut().unwrap().remove("org_id");
        assert_eq!(items(&page), json!([expected]));
    }

    #[tokio::test]
    async fn an_extra_hit_becomes_the_next_cursor() {
        let server = MockServer::start().await;
        accept_any_query(&server).await;
        mount(&server, "POST", "_search", hits(&[3, 2, 1])).await;
        let first = QueryRequest {
            page_size: Some(2),
            ..QueryRequest::default()
        };

        let page = search(&server, &first).await;

        assert_eq!(page.items.len(), 2);
        let last_returned = hit(2)["sort"].as_array().unwrap().clone();
        assert_eq!(page.next_cursor, Some(cursor::encode(&last_returned)));

        // The cursor comes back to OpenSearch as `search_after`.
        let second = QueryRequest {
            cursor: page.next_cursor,
            ..first
        };
        search(&server, &second).await;
        let requests = server.received_requests().await.unwrap();
        let body: Value = requests.last().unwrap().body_json().unwrap();
        assert_eq!(body["search_after"], Value::Array(last_returned));
        assert_eq!(body["size"], 3);
    }

    #[tokio::test]
    async fn a_session_search_filters_on_the_session_oldest_first() {
        let server = MockServer::start().await;
        accept_any_query(&server).await;
        mount(&server, "POST", "_search", hits(&[1, 2])).await;

        client(&server)
            .search_events(&scope(), &request(None), Some("session-1"), SortOrder::Asc)
            .await
            .unwrap();

        let requests = server.received_requests().await.unwrap();
        let body: Value = requests.last().unwrap().body_json().unwrap();
        assert_eq!(
            body["query"]["bool"]["filter"][2],
            json!({ "term": { "session_id": "session-1" } })
        );
        assert_eq!(body["sort"][0]["occured_at"]["order"], "asc");
    }

    #[tokio::test]
    async fn a_query_opensearch_calls_invalid_is_not_run() {
        let server = MockServer::start().await;
        let refusal = json!({
            "valid": false,
            "explanations": [{
                "index": INDEX,
                "valid": false,
                "error": format!("[{INDEX}/xjCVa9zQ] QueryShardException[failed to parse date field [nope]]"),
            }],
        });
        mount(
            &server,
            "POST",
            "_validate/query",
            ResponseTemplate::new(200).set_body_json(refusal),
        )
        .await;

        let error = client(&server)
            .search_events(&scope(), &request(None), None, SortOrder::Desc)
            .await
            .unwrap_err();

        let OpenSearchError::Request(RequestError::InvalidQuery(reason)) = error else {
            panic!("expected an invalid query, got {error:?}");
        };
        assert_eq!(
            reason,
            "[[index]/xjCVa9zQ] QueryShardException[failed to parse date field [nope]]"
        );
        assert_eq!(requested_paths(&server).await, [api("_validate/query")]);
    }

    #[tokio::test]
    async fn an_unparseable_query_reports_the_top_level_error() {
        let server = MockServer::start().await;
        let refusal = json!({ "valid": false, "error": "ParsingException[unknown query [nope]]" });
        mount(
            &server,
            "POST",
            "_validate/query",
            ResponseTemplate::new(200).set_body_json(refusal),
        )
        .await;

        let error = client(&server).count(&scope(), None).await.unwrap_err();

        assert!(matches!(
            error,
            OpenSearchError::Request(RequestError::InvalidQuery(reason))
                if reason == "ParsingException[unknown query [nope]]"
        ));
    }

    #[tokio::test]
    async fn a_query_refused_locally_never_reaches_opensearch() {
        let server = MockServer::start().await;
        let other_org = request(Some(json!({ "term": { "org_id": "someone-else" } })));

        let error = client(&server)
            .search_events(&scope(), &other_org, None, SortOrder::Desc)
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            OpenSearchError::Request(RequestError::WrongOrg)
        ));
        assert!(requested_paths(&server).await.is_empty());
    }

    #[tokio::test]
    async fn a_bad_cursor_or_page_size_never_reaches_opensearch() {
        let server = MockServer::start().await;
        let bad_cursor = QueryRequest {
            cursor: Some("not a cursor".into()),
            ..QueryRequest::default()
        };
        let bad_page_size = QueryRequest {
            page_size: Some(0),
            ..QueryRequest::default()
        };

        for (request, expected) in [
            (bad_cursor, RequestError::InvalidCursor),
            (bad_page_size, RequestError::InvalidPageSize),
        ] {
            let error = client(&server)
                .search_events(&scope(), &request, None, SortOrder::Desc)
                .await
                .unwrap_err();
            assert!(matches!(error, OpenSearchError::Request(actual) if actual == expected));
        }
        assert!(requested_paths(&server).await.is_empty());
    }

    #[tokio::test]
    async fn a_missing_index_reads_as_no_events() {
        // Nothing is mounted, so every call gets a 404.
        let server = MockServer::start().await;
        let bounded = request(Some(json!({
            "range": { "occured_at": { "gte": "now-1d", "lte": "now" } }
        })));

        let events = search(&server, &request(None)).await;
        let count = client(&server).count(&scope(), None).await.unwrap();
        let event = client(&server)
            .get_event(&scope(), "missing")
            .await
            .unwrap();
        let sessions = client(&server)
            .list_sessions(&scope(), &bounded)
            .await
            .unwrap();

        assert!(events.items.is_empty() && events.next_cursor.is_none());
        assert_eq!(count, 0);
        assert!(event.is_none());
        assert!(sessions.items.is_empty() && sessions.next_cursor.is_none());
    }

    #[tokio::test]
    async fn count_sends_the_scoped_query() {
        let server = MockServer::start().await;
        accept_any_query(&server).await;
        let client_query = json!({ "term": { "name": "checkout_viewed" } });
        let scoped = query::scoped_query(Some(&client_query), &scope(), Vec::new());
        Mock::given(method("POST"))
            .and(path(api("_count")))
            .and(body_json(json!({ "query": scoped })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "count": 42 })))
            .expect(1)
            .mount(&server)
            .await;

        let count = client(&server)
            .count(&scope(), Some(&client_query))
            .await
            .unwrap();

        assert_eq!(count, 42);
    }

    #[tokio::test]
    async fn get_event_returns_a_document_in_scope() {
        let server = MockServer::start().await;
        let found = json!({ "found": true, "_source": source(1, PROJ) });
        mount(
            &server,
            "GET",
            "_doc/00000000-0000-4000-8000-000000000001",
            ResponseTemplate::new(200).set_body_json(found),
        )
        .await;

        let event = client(&server)
            .get_event(&scope(), "00000000-0000-4000-8000-000000000001")
            .await
            .unwrap()
            .expect("the event is in scope");

        let event = serde_json::to_value(event).unwrap();
        assert_eq!(event["id"], "00000000-0000-4000-8000-000000000001");
        assert!(event.get("org_id").is_none());
    }

    #[tokio::test]
    async fn get_event_hides_documents_outside_the_scope() {
        let mut other_org = source(1, PROJ);
        other_org["org_id"] = json!("someone-else");
        let mut no_proj = source(1, PROJ);
        no_proj.as_object_mut().unwrap().remove("project_id");

        for stored in [source(1, "another-proj"), other_org, no_proj] {
            let server = MockServer::start().await;
            let found = json!({ "found": true, "_source": stored });
            mount(
                &server,
                "GET",
                "_doc/1",
                ResponseTemplate::new(200).set_body_json(found),
            )
            .await;

            let event = client(&server).get_event(&scope(), "1").await.unwrap();

            assert!(event.is_none());
        }
    }

    #[tokio::test]
    async fn a_document_id_cannot_escape_its_path_segment() {
        let server = MockServer::start().await;

        client(&server)
            .get_event(&scope(), "a/../_search?q=*")
            .await
            .unwrap();

        let requests = server.received_requests().await.unwrap();
        assert_eq!(
            requests[0].url.path(),
            api("_doc/a%2F..%2F_search%3Fq%3D%2A")
        );
        assert_eq!(requests[0].url.query(), None);
    }

    #[tokio::test]
    async fn sessions_without_a_time_range_never_reach_opensearch() {
        let server = MockServer::start().await;

        for query in [None, Some(json!({ "term": { "name": "checkout_viewed" } }))] {
            let error = client(&server)
                .list_sessions(&scope(), &request(query))
                .await
                .unwrap_err();
            assert!(matches!(
                error,
                OpenSearchError::Request(RequestError::MissingTimeRange)
            ));
        }
        assert!(requested_paths(&server).await.is_empty());
    }

    #[tokio::test]
    async fn sessions_are_summarised_and_paged_by_session_id() {
        let server = MockServer::start().await;
        accept_any_query(&server).await;
        let bucket = |session: &str, count: u64, first: i64, last: i64| {
            json!({
                "key": { "session_id": session },
                "doc_count": count,
                "first_event_at": { "value": first as f64 },
                "last_event_at": { "value": last as f64 },
            })
        };
        let reply = json!({ "aggregations": { "sessions": { "buckets": [
            bucket("session-a", 3, 1_790_675_429_000, 1_790_675_431_500),
            bucket("session-b", 1, 1_790_675_440_000, 1_790_675_440_000),
        ]}}});
        mount(
            &server,
            "POST",
            "_search",
            ResponseTemplate::new(200).set_body_json(reply),
        )
        .await;
        let first = QueryRequest {
            query: Some(json!({ "range": { "occured_at": { "gte": "now-1d", "lte": "now" } } })),
            cursor: None,
            page_size: Some(1),
            order: None,
        };

        let page = client(&server)
            .list_sessions(&scope(), &first)
            .await
            .unwrap();

        assert_eq!(
            items(&page),
            json!([{
                "session_id": "session-a",
                "first_event_at": "2026-09-29T09:50:29Z",
                "last_event_at": "2026-09-29T09:50:31.500Z",
                "event_count": 3,
            }])
        );
        assert_eq!(
            page.next_cursor,
            Some(cursor::encode(&[json!("session-a")]))
        );

        // The cursor comes back to OpenSearch as the composite `after` key.
        let second = QueryRequest {
            cursor: page.next_cursor,
            ..first
        };
        client(&server)
            .list_sessions(&scope(), &second)
            .await
            .unwrap();
        let requests = server.received_requests().await.unwrap();
        let body: Value = requests.last().unwrap().body_json().unwrap();
        assert_eq!(
            body["aggs"]["sessions"]["composite"]["after"],
            json!({ "session_id": "session-a" })
        );
    }

    #[tokio::test]
    async fn an_event_cursor_is_not_a_session_cursor() {
        let server = MockServer::start().await;
        let request = QueryRequest {
            query: Some(json!({ "range": { "occured_at": { "gte": "now-1d", "lte": "now" } } })),
            cursor: Some(cursor::encode(&[json!(1), json!("id")])),
            page_size: None,
            order: None,
        };

        let error = client(&server)
            .list_sessions(&scope(), &request)
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            OpenSearchError::Request(RequestError::InvalidCursor)
        ));
    }

    #[tokio::test]
    async fn a_request_opensearch_rejects_is_the_clients_error() {
        let server = MockServer::start().await;
        accept_any_query(&server).await;
        let rejection = json!({
            "error": {
                "root_cause": [{
                    "type": "illegal_argument_exception",
                    "reason": format!("no mapping found for [id] in [{INDEX}]"),
                }],
                "type": "search_phase_execution_exception",
                "reason": "all shards failed",
            },
            "status": 400,
        });
        mount(
            &server,
            "POST",
            "_search",
            ResponseTemplate::new(400).set_body_json(rejection),
        )
        .await;

        let error = client(&server)
            .search_events(&scope(), &request(None), None, SortOrder::Desc)
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            OpenSearchError::Request(RequestError::InvalidQuery(reason))
                if reason == "no mapping found for [id] in [[index]]"
        ));
    }

    #[tokio::test]
    async fn an_opensearch_failure_is_not_the_clients_error() {
        let server = MockServer::start().await;
        mount(
            &server,
            "POST",
            "_validate/query",
            ResponseTemplate::new(503).set_body_string("unavailable"),
        )
        .await;

        let error = client(&server).count(&scope(), None).await.unwrap_err();

        assert!(matches!(
            error,
            OpenSearchError::Upstream { status: 503, ref body } if body == "unavailable"
        ));
    }

    #[tokio::test]
    async fn a_stored_document_that_is_not_an_event_is_a_decode_error() {
        let server = MockServer::start().await;
        accept_any_query(&server).await;
        let reply =
            json!({ "hits": { "hits": [{ "_source": { "name": "no id" }, "sort": [1, "a"] }] } });
        mount(
            &server,
            "POST",
            "_search",
            ResponseTemplate::new(200).set_body_json(reply),
        )
        .await;

        let error = client(&server)
            .search_events(&scope(), &request(None), None, SortOrder::Desc)
            .await
            .unwrap_err();

        assert!(matches!(error, OpenSearchError::Decode(_)));
    }

    #[tokio::test]
    async fn credentials_and_a_path_prefix_are_sent() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(format!("/search{}", api("_validate/query"))))
            .and(basic_auth("dashboard", "hunter2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "valid": true })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path(format!("/search{}", api("_count"))))
            .and(basic_auth("dashboard", "hunter2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "count": 1 })))
            .expect(1)
            .mount(&server)
            .await;
        let client = OpenSearch::new(
            &format!("{}/search/", server.uri()),
            Some(("dashboard".into(), "hunter2".into())),
            "events-{org_id}".into(),
            Duration::from_secs(2),
        )
        .unwrap();

        assert_eq!(client.count(&scope(), None).await.unwrap(), 1);
    }

    #[test]
    fn a_scope_needs_an_organization() {
        let organization = canonical_event::OrgId::try_from("acme-corp".to_string()).unwrap();
        let mut project = db::Project {
            id: canonical_event::ProjectId::try_from("acme-corp-shop".to_string()).unwrap(),
            organization_id: Some(organization.clone()),
            name: "shop".into(),
            timezone: None,
            deleted_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(
            Scope::for_project(&project),
            Some(Scope {
                org_id: organization.to_string(),
                project_id: project.id.to_string(),
            })
        );

        project.organization_id = None;
        assert_eq!(Scope::for_project(&project), None);
    }

    #[test]
    fn excerpt_cuts_long_text_on_a_char_boundary() {
        let text = "é".repeat(MAX_ERROR_TEXT + 10);

        assert_eq!(excerpt(&text).chars().count(), MAX_ERROR_TEXT);
        assert_eq!(excerpt("short"), "short");
    }

    fn at(time: &str) -> DateTime<Utc> {
        time.parse().unwrap()
    }

    fn bounded_query() -> Value {
        json!({ "range": { "occured_at": { "gte": "now-1d", "lte": "now" } } })
    }

    async fn last_search_body(server: &MockServer) -> Value {
        let requests = server.received_requests().await.unwrap();
        requests.last().unwrap().body_json().unwrap()
    }

    #[tokio::test]
    async fn the_request_can_choose_the_time_order() {
        let server = MockServer::start().await;
        accept_any_query(&server).await;
        mount(&server, "POST", "_search", hits(&[1, 2])).await;
        let oldest_first = QueryRequest {
            order: Some(SortOrder::Asc),
            ..QueryRequest::default()
        };

        search(&server, &oldest_first).await;

        let body = last_search_body(&server).await;
        assert_eq!(body["sort"][0]["occured_at"]["order"], "asc");
    }

    #[tokio::test]
    async fn a_histogram_counts_events_per_bucket_inside_the_range() {
        let server = MockServer::start().await;
        let (from, to) = (at("2026-09-01T10:00:00Z"), at("2026-09-01T11:00:00Z"));
        let client_query = json!({ "term": { "name": "checkout_viewed" } });
        let scoped = query::scoped_query(
            Some(&client_query),
            &scope(),
            vec![query::time_filter(from, to)],
        );
        Mock::given(method("POST"))
            .and(path(api("_validate/query")))
            .and(body_json(json!({ "query": scoped })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "valid": true })))
            .expect(1)
            .mount(&server)
            .await;
        let reply = json!({ "aggregations": { "events": { "buckets": [
            { "key_as_string": "2026-09-01T10:00:00.000Z", "key": 1_788_256_800_000_i64, "doc_count": 4 },
            { "key_as_string": "2026-09-01T10:01:00.000Z", "key": 1_788_256_860_000_i64, "doc_count": 0 },
        ]}}});
        Mock::given(method("POST"))
            .and(path(api("_search")))
            .and(body_json(query::histogram_body(
                scoped.clone(),
                from,
                to,
                "1m",
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(reply))
            .expect(1)
            .mount(&server)
            .await;

        let histogram = client(&server)
            .histogram(&scope(), Some(&client_query), from, to)
            .await
            .unwrap();

        assert_eq!(
            serde_json::to_value(histogram).unwrap(),
            json!({
                "interval": "1m",
                "buckets": [
                    { "time": "2026-09-01T10:00:00Z", "count": 4 },
                    { "time": "2026-09-01T10:01:00Z", "count": 0 },
                ],
            })
        );
    }

    #[tokio::test]
    async fn a_histogram_over_a_backwards_range_never_reaches_opensearch() {
        let server = MockServer::start().await;
        let (from, to) = (at("2026-09-01T11:00:00Z"), at("2026-09-01T10:00:00Z"));

        let error = client(&server)
            .histogram(&scope(), None, from, to)
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            OpenSearchError::Request(RequestError::InvalidTimeRange)
        ));
        assert!(requested_paths(&server).await.is_empty());
    }

    #[tokio::test]
    async fn facets_return_the_top_values_with_totals() {
        let server = MockServer::start().await;
        accept_any_query(&server).await;
        let reply = json!({
            "hits": { "total": { "value": 10, "relation": "eq" } },
            "aggregations": { "values": {
                "sum_other_doc_count": 2,
                "buckets": [
                    { "key": 1, "key_as_string": "true", "doc_count": 5 },
                    { "key": "page_view", "doc_count": 3 },
                ],
            }},
        });
        mount(
            &server,
            "POST",
            "_search",
            ResponseTemplate::new(200).set_body_json(reply),
        )
        .await;

        let facets = client(&server)
            .facets(&scope(), Some(&bounded_query()), "name", Some(2), None)
            .await
            .unwrap();

        assert_eq!(
            serde_json::to_value(facets).unwrap(),
            json!({
                "field": "name",
                "total": 10,
                "other": 2,
                "buckets": [
                    { "value": "true", "count": 5 },
                    { "value": "page_view", "count": 3 },
                ],
            })
        );
        let body = last_search_body(&server).await;
        assert_eq!(
            body["aggs"]["values"],
            json!({ "terms": { "field": "name", "size": 2 } })
        );
    }

    #[tokio::test]
    async fn a_facet_prefix_narrows_the_counted_values() {
        let server = MockServer::start().await;
        accept_any_query(&server).await;
        let reply = json!({
            "hits": { "total": { "value": 0 } },
            "aggregations": { "values": { "sum_other_doc_count": 0, "buckets": [] } },
        });
        mount(
            &server,
            "POST",
            "_search",
            ResponseTemplate::new(200).set_body_json(reply),
        )
        .await;

        client(&server)
            .facets(&scope(), Some(&bounded_query()), "name", None, Some("pa"))
            .await
            .unwrap();

        let body = last_search_body(&server).await;
        assert_eq!(
            body["query"]["bool"]["filter"][2],
            json!({ "prefix": { "name": "pa" } })
        );
    }

    #[tokio::test]
    async fn facets_refused_locally_never_reach_opensearch() {
        let server = MockServer::start().await;
        let bounded = bounded_query();
        let cases = [
            (
                "org_id",
                Some(&bounded),
                None,
                RequestError::UnknownFacetField,
            ),
            (
                "properties.plan",
                Some(&bounded),
                None,
                RequestError::UnknownFacetField,
            ),
            (
                "name",
                Some(&bounded),
                Some(51),
                RequestError::InvalidFacetSize,
            ),
            ("name", None, None, RequestError::MissingTimeRange),
        ];

        for (field, query, size, expected) in cases {
            let error = client(&server)
                .facets(&scope(), query, field, size, None)
                .await
                .unwrap_err();
            assert!(
                matches!(&error, OpenSearchError::Request(actual) if *actual == expected),
                "{field}: {error:?}"
            );
        }
        assert!(requested_paths(&server).await.is_empty());
    }

    #[tokio::test]
    async fn aggregations_over_a_missing_index_are_empty() {
        // Nothing is mounted, so every call gets a 404.
        let server = MockServer::start().await;
        let (from, to) = (at("2026-09-01T10:00:00Z"), at("2026-09-01T11:00:00Z"));

        let histogram = client(&server)
            .histogram(&scope(), None, from, to)
            .await
            .unwrap();
        let facets = client(&server)
            .facets(&scope(), Some(&bounded_query()), "name", None, None)
            .await
            .unwrap();

        assert_eq!(
            histogram,
            Histogram {
                interval: "1m",
                buckets: Vec::new(),
            }
        );
        assert_eq!(
            facets,
            Facets {
                field: "name".into(),
                total: 0,
                other: 0,
                buckets: Vec::new(),
            }
        );
    }

    /// A real OpenSearch holding one index shared by two projects of an
    /// org, plus an event of another org.
    struct Live {
        http: reqwest::Client,
        url: String,
        index: String,
        client: OpenSearch,
        org: String,
    }

    impl Live {
        /// Creates the index with the mappings `sink-opensearch` ships.
        async fn start() -> Self {
            let url = std::env::var("OPENSEARCH_URL")
                .unwrap_or_else(|_| "http://localhost:9200".to_string());
            let org = uuid::Uuid::new_v4().to_string();
            let live = Self {
                http: reqwest::Client::new(),
                client: OpenSearch::new(
                    &url,
                    None,
                    "events-{org_id}".into(),
                    Duration::from_secs(10),
                )
                .unwrap(),
                index: format!("events-{org}"),
                url,
                org,
            };

            let template = std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../sink-opensearch/index-template.json"
            ))
            .unwrap();
            let template: Value = serde_json::from_str(&template).unwrap();
            live.send(Method::PUT, "", Some(&template["template"]))
                .await;
            live
        }

        async fn send(&self, method: Method, api: &str, body: Option<&Value>) {
            let mut request = self
                .http
                .request(method, format!("{}/{}/{api}", self.url, self.index));
            if let Some(body) = body {
                request = request.json(body);
            }
            let response = request.send().await.unwrap();
            assert!(
                response.status().is_success(),
                "{}",
                response.text().await.unwrap()
            );
        }

        fn scope(&self, proj: &str) -> Scope {
            Scope {
                org_id: self.org.clone(),
                project_id: proj.into(),
            }
        }

        /// Indexes an event that occurred `second` seconds into the test
        /// minute, and returns its id.
        async fn index(
            &self,
            org: &str,
            proj: &str,
            session: Option<&str>,
            second: u32,
            name: &str,
        ) -> String {
            use chrono::TimeZone;

            let event = CanonicalEvent::builder()
                .name(name.into())
                .occured_at(Utc.with_ymd_and_hms(2026, 9, 1, 10, 0, second).unwrap())
                .org_id(org.into())
                .project_id(Some(proj.into()))
                .session_id(session.map(Into::into))
                .anon_id("anon-1".into())
                .properties(Some(json!({ "plan": "pro" })))
                .build();
            let id = event.id().to_string();
            let source = serde_json::to_value(&event).unwrap();
            self.send(Method::PUT, &format!("_doc/{id}"), Some(&source))
                .await;
            id
        }

        /// Follows cursors to the end; returns every item and the page count.
        async fn events(
            &self,
            scope: &Scope,
            query: Option<Value>,
            session: Option<&str>,
            order: SortOrder,
            page_size: usize,
        ) -> (Vec<Value>, usize) {
            let mut request = QueryRequest {
                query,
                cursor: None,
                page_size: Some(page_size),
                order: None,
            };
            let (mut all, mut pages) = (Vec::new(), 0);
            loop {
                let page = self
                    .client
                    .search_events(scope, &request, session, order)
                    .await
                    .unwrap();
                pages += 1;
                assert!(page.items.len() <= page_size);
                all.extend(items(&page).as_array().unwrap().iter().cloned());
                match page.next_cursor {
                    Some(cursor) => request.cursor = Some(cursor),
                    None => return (all, pages),
                }
            }
        }

        async fn sessions(&self, scope: &Scope, range: Value, page_size: usize) -> (Value, usize) {
            let mut request = QueryRequest {
                query: Some(json!({ "range": { "occured_at": range } })),
                cursor: None,
                page_size: Some(page_size),
                order: None,
            };
            let (mut all, mut pages) = (Vec::new(), 0);
            loop {
                let page = self.client.list_sessions(scope, &request).await.unwrap();
                pages += 1;
                all.extend(items(&page).as_array().unwrap().iter().cloned());
                match page.next_cursor {
                    Some(cursor) => request.cursor = Some(cursor),
                    None => return (Value::Array(all), pages),
                }
            }
        }
    }

    fn column(events: &[Value], field: &str) -> Vec<String> {
        events
            .iter()
            .map(|event| event[field].as_str().unwrap_or_default().to_owned())
            .collect()
    }

    /// Runs against a real OpenSearch at `OPENSEARCH_URL` (default
    /// `http://localhost:9200`), such as the one in `tests/docker-compose.yml`:
    /// `cargo test -p hyper-analytics-dashboard-server -- --ignored`
    #[tokio::test]
    #[ignore = "needs a running OpenSearch"]
    async fn live_opensearch_pages_and_isolates_projects() {
        const A: &str = "proj-a";
        const B: &str = "proj-b";
        let live = Live::start().await;
        let org = live.org.clone();

        // Project A: two sessions and one event outside any session. Two
        // events share a timestamp, which only the `id` tie-breaker orders.
        let mut a_ids = Vec::new();
        for (session, second, name) in [
            (Some("s1"), 0, "page_view"),
            (Some("s1"), 1, "page_view"),
            (Some("s1"), 2, "page_view"),
            (Some("s2"), 3, "click"),
            (Some("s2"), 4, "click"),
            (Some("s2"), 4, "click"),
            (None, 5, "click"),
        ] {
            a_ids.push(live.index(&org, A, session, second, name).await);
        }
        // Project B reuses session id `s1`; another org shares the index.
        let b_id = live.index(&org, B, Some("s3"), 6, "click").await;
        live.index(&org, B, Some("s1"), 7, "page_view").await;
        let foreign_id = live.index("other-org", A, Some("s1"), 8, "page_view").await;
        live.send(Method::POST, "_refresh", None).await;
        let (a, b) = (live.scope(A), live.scope(B));

        // Search: every event of A exactly once, newest first, over 3 pages.
        let (events, pages) = live.events(&a, None, None, SortOrder::Desc, 3).await;
        let mut found = column(&events, "id");
        let times = column(&events, "occured_at");
        assert_eq!(pages, 3);
        assert!(times.windows(2).all(|pair| pair[0] >= pair[1]), "{times:?}");
        assert!(events.iter().all(|event| event.get("org_id").is_none()));
        assert!(events.iter().all(|event| event["project_id"] == A));
        found.sort();
        a_ids.sort();
        assert_eq!(found, a_ids);

        // A client query narrows the scope, including on a `properties` key.
        let page_views = json!({ "term": { "name": "page_view" } });
        let (events, _) = live
            .events(&a, Some(page_views.clone()), None, SortOrder::Desc, 50)
            .await;
        assert_eq!(events.len(), 3);
        let on_plan = json!({ "term": { "properties.plan": "pro" } });
        assert_eq!(live.client.count(&a, Some(&on_plan)).await.unwrap(), 7);

        // Count.
        assert_eq!(live.client.count(&a, None).await.unwrap(), 7);
        assert_eq!(live.client.count(&a, Some(&page_views)).await.unwrap(), 3);
        assert_eq!(live.client.count(&b, None).await.unwrap(), 2);

        // Get: only documents of the project's own org and project.
        let own = live.client.get_event(&a, &a_ids[0]).await.unwrap();
        let own = serde_json::to_value(own.expect("A reads its own event")).unwrap();
        assert_eq!(own["id"], a_ids[0].as_str());
        assert!(own.get("org_id").is_none());
        for id in [b_id.as_str(), foreign_id.as_str(), "no-such-document"] {
            assert!(live.client.get_event(&a, id).await.unwrap().is_none());
        }

        // Sessions: one per page, summarised inside the requested range.
        let minute = json!({ "gte": "2026-09-01T10:00:00Z", "lt": "2026-09-01T10:01:00Z" });
        let (sessions, pages) = live.sessions(&a, minute, 1).await;
        assert_eq!(pages, 2);
        assert_eq!(
            sessions,
            json!([
                {
                    "session_id": "s1",
                    "first_event_at": "2026-09-01T10:00:00Z",
                    "last_event_at": "2026-09-01T10:00:02Z",
                    "event_count": 3,
                },
                {
                    "session_id": "s2",
                    "first_event_at": "2026-09-01T10:00:03Z",
                    "last_event_at": "2026-09-01T10:00:04Z",
                    "event_count": 3,
                },
            ])
        );
        let later = json!({ "gte": "2026-09-01T10:00:01Z", "lt": "2026-09-01T10:00:04Z" });
        let (sessions, _) = live.sessions(&a, later, 50).await;
        assert_eq!(sessions[0]["event_count"], 2);
        assert_eq!(sessions[1]["event_count"], 1);

        // Session events: oldest first, and only A's events of `s1`.
        let (events, pages) = live.events(&a, None, Some("s1"), SortOrder::Asc, 2).await;
        assert_eq!(pages, 2);
        assert!(column(&events, "id").iter().all(|id| a_ids.contains(id)));
        assert_eq!(
            column(&events, "occured_at"),
            [
                "2026-09-01T10:00:00Z",
                "2026-09-01T10:00:01Z",
                "2026-09-01T10:00:02Z"
            ]
        );

        // Histogram: the test minute in 1s buckets, empty ones included.
        let (from, to) = (at("2026-09-01T10:00:00Z"), at("2026-09-01T10:01:00Z"));
        let histogram = live.client.histogram(&a, None, from, to).await.unwrap();
        let counts: Vec<u64> = histogram
            .buckets
            .iter()
            .map(|bucket| bucket.count)
            .collect();
        assert_eq!(histogram.interval, "1s");
        assert_eq!(counts.len(), 60);
        assert_eq!(counts[..6], [1, 1, 1, 1, 2, 1]);
        assert_eq!(counts.iter().sum::<u64>(), 7);
        assert_eq!(histogram.buckets[0].time, from);
        let clicks = json!({ "term": { "name": "click" } });
        let histogram = live
            .client
            .histogram(&a, Some(&clicks), from, to)
            .await
            .unwrap();
        assert_eq!(
            histogram
                .buckets
                .iter()
                .map(|bucket| bucket.count)
                .sum::<u64>(),
            4
        );

        // Facets: exact counts over everything the query matches in A.
        let in_minute = json!({ "range": { "occured_at": {
            "gte": "2026-09-01T10:00:00Z", "lt": "2026-09-01T10:01:00Z",
        }}});
        let facet = |value: &str, count| FacetBucket {
            value: value.into(),
            count,
        };
        let names = live
            .client
            .facets(&a, Some(&in_minute), "name", None, None)
            .await
            .unwrap();
        assert_eq!(names.total, 7);
        assert_eq!(names.other, 0);
        assert_eq!(names.buckets, [facet("click", 4), facet("page_view", 3)]);
        let names = live
            .client
            .facets(&a, Some(&in_minute), "name", None, Some("pa"))
            .await
            .unwrap();
        assert_eq!(names.buckets, [facet("page_view", 3)]);
        let top_session = live
            .client
            .facets(&a, Some(&in_minute), "session_id", Some(1), None)
            .await
            .unwrap();
        assert_eq!(top_session.total, 7);
        assert_eq!(top_session.other, 3);
        assert_eq!(top_session.buckets.len(), 1);

        // Oldest first on request.
        let (events, _) = live.events(&a, None, None, SortOrder::Asc, 50).await;
        let times = column(&events, "occured_at");
        assert!(times.windows(2).all(|pair| pair[0] <= pair[1]), "{times:?}");

        // An invalid query is refused without revealing the org.
        let bad_date = json!({ "range": { "occured_at": { "gte": "not-a-date" } } });
        let error = live.client.count(&a, Some(&bad_date)).await.unwrap_err();
        let OpenSearchError::Request(RequestError::InvalidQuery(reason)) = error else {
            panic!("expected an invalid query, got {error:?}");
        };
        assert!(reason.contains("not-a-date"), "{reason}");
        assert!(!reason.contains(&org), "{reason}");

        // An org with no index yet has no events.
        let nobody = Scope {
            org_id: uuid::Uuid::new_v4().to_string(),
            project_id: A.into(),
        };
        assert_eq!(live.client.count(&nobody, None).await.unwrap(), 0);

        live.send(Method::DELETE, "", None).await;
    }
}
