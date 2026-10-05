//! Checks a client's query and builds the request bodies sent to OpenSearch.

use chrono::{DateTime, SecondsFormat, TimeDelta, Utc};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::{RequestError, Scope};

/// Keys that make a query read from somewhere other than the index being
/// searched: terms lookups, `more_like_this` documents, indexed shapes and
/// percolators name an index, and `wrapper` hides a whole query in base64.
const FORBIDDEN_KEYS: [&str; 3] = ["index", "_index", "wrapper"];

/// `@timestamp` is an alias of `occured_at` in the index template.
const TIME_FIELDS: [&str; 2] = ["occured_at", "@timestamp"];

/// Fields a `terms` aggregation may run on: the keyword and boolean fields of
/// the index template, without the ones that name the org or are unique
/// per event. `properties` is a `flat_object`, which cannot be aggregated.
pub const FACET_FIELDS: [&str; 11] = [
    "name",
    "source",
    "session_id",
    "anon_id",
    "actor_id",
    "correlation_id",
    "trace_id",
    "authenticated",
    "envelop_version",
    "system_properties.geo.country",
    "system_properties.timezone",
];

pub const DEFAULT_FACET_SIZE: usize = 5;
pub const MAX_FACET_SIZE: usize = 50;

/// Most buckets a histogram is split into.
const MAX_HISTOGRAM_BUCKETS: i64 = 60;

/// Bucket widths a histogram may use, as OpenSearch `fixed_interval` values
/// and their length in seconds.
const HISTOGRAM_INTERVALS: [(&str, i64); 15] = [
    ("1s", 1),
    ("5s", 5),
    ("10s", 10),
    ("30s", 30),
    ("1m", 60),
    ("5m", 300),
    ("10m", 600),
    ("30m", 1_800),
    ("1h", 3_600),
    ("3h", 10_800),
    ("12h", 43_200),
    ("1d", 86_400),
    ("7d", 604_800),
    ("30d", 2_592_000),
    ("365d", 31_536_000),
];

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Asc,
    Desc,
}

impl SortOrder {
    fn as_str(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

/// Rejects a client query that names another index or another org.
///
/// This exists to answer such a query with an error rather than an empty
/// result. Isolation itself comes from the filter `scoped_query` adds, which
/// no client clause can widen.
pub fn check_query(query: &Value, scope: &Scope) -> Result<(), RequestError> {
    if !query.is_object() {
        return Err(RequestError::QueryNotAnObject);
    }
    walk(query, scope)
}

fn walk(value: &Value, scope: &Scope) -> Result<(), RequestError> {
    match value {
        Value::Object(fields) => fields.iter().try_for_each(|(key, child)| {
            if FORBIDDEN_KEYS.contains(&key.as_str()) {
                return Err(RequestError::ForbiddenKey(key.clone()));
            }
            if key == "org_id" {
                return if names_org(child, &scope.org_id) {
                    Ok(())
                } else {
                    Err(RequestError::WrongOrg)
                };
            }
            walk(child, scope)
        }),
        Value::Array(items) => items.iter().try_for_each(|item| walk(item, scope)),
        _ => Ok(()),
    }
}

/// Whether a clause on `org_id` matches exactly `org_id`: the forms
/// `"t"`, `["t"]`, `{"value": "t"}` and `{"query": "t"}`. Anything else, such
/// as a range or a wildcard, could match other orgs.
fn names_org(clause: &Value, org_id: &str) -> bool {
    match clause {
        Value::String(value) => value == org_id,
        Value::Array(values) => {
            !values.is_empty() && values.iter().all(|value| value.as_str() == Some(org_id))
        }
        Value::Object(fields) => ["value", "query"]
            .iter()
            .filter_map(|key| fields.get(*key))
            .any(|value| value.as_str() == Some(org_id)),
        _ => false,
    }
}

/// Requires a range on the event time with a lower and an upper bound, in a
/// position where it always applies. Aggregating sessions reads every event
/// the query matches, so an unbounded query is refused.
pub fn require_time_range(query: Option<&Value>) -> Result<(), RequestError> {
    if query.is_some_and(is_time_bounded) {
        Ok(())
    } else {
        Err(RequestError::MissingTimeRange)
    }
}

fn is_time_bounded(query: &Value) -> bool {
    if query.get("range").is_some_and(range_is_bounded) {
        return true;
    }
    // Only `must` and `filter` restrict every match; a range under `should`
    // or `must_not` bounds nothing.
    let Some(clauses) = query.get("bool").and_then(Value::as_object) else {
        return false;
    };
    ["must", "filter"]
        .iter()
        .filter_map(|occur| clauses.get(*occur))
        .any(|clause| match clause {
            Value::Array(items) => items.iter().any(is_time_bounded),
            single => is_time_bounded(single),
        })
}

fn range_is_bounded(range: &Value) -> bool {
    TIME_FIELDS
        .iter()
        .filter_map(|field| range.get(*field))
        .any(|bounds| {
            let has = |keys: [&str; 2]| {
                keys.iter()
                    .any(|key| bounds.get(*key).is_some_and(|bound| !bound.is_null()))
            };
            has(["gte", "gt"]) && has(["lte", "lt"])
        })
}

/// Wraps the client's query in the project's scope. `narrowing` holds the
/// filters an endpoint adds on top, such as one session or a time range.
pub fn scoped_query(client: Option<&Value>, scope: &Scope, narrowing: Vec<Value>) -> Value {
    let mut filter = vec![
        json!({ "term": { "org_id": scope.org_id } }),
        json!({ "term": { "proj_id": scope.proj_id } }),
    ];
    filter.extend(narrowing);
    let must = client.cloned().unwrap_or_else(|| json!({ "match_all": {} }));

    json!({ "bool": { "must": [must], "filter": filter } })
}

pub fn session_filter(session_id: &str) -> Value {
    json!({ "term": { "session_id": session_id } })
}

/// Events that occurred in `[from, to)`.
pub fn time_filter(from: DateTime<Utc>, to: DateTime<Utc>) -> Value {
    json!({ "range": { "occured_at": { "gte": timestamp(from), "lt": timestamp(to) } } })
}

/// Values of `field` that start with `prefix`.
pub fn prefix_filter(field: &str, prefix: &str) -> Value {
    json!({ "prefix": { field: prefix } })
}

fn timestamp(instant: DateTime<Utc>) -> String {
    instant.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// `_search` body for one page of events. `id` breaks ties between events
/// with the same timestamp, so `search_after` never skips or repeats one.
pub fn events_body(
    query: Value,
    order: SortOrder,
    page_size: usize,
    after: Option<Vec<Value>>,
) -> Value {
    let mut body = Map::new();
    body.insert("size".into(), json!(page_size + 1));
    body.insert("track_total_hits".into(), json!(false));
    body.insert("query".into(), query);
    body.insert(
        "sort".into(),
        json!([
            { "occured_at": { "order": order.as_str() } },
            { "id": { "order": "asc" } },
        ]),
    );
    if let Some(after) = after {
        body.insert("search_after".into(), Value::Array(after));
    }
    Value::Object(body)
}

/// `_search` body for one page of session summaries.
pub fn sessions_body(query: Value, page_size: usize, after: Option<&str>) -> Value {
    let mut composite = json!({
        "size": page_size + 1,
        "sources": [{ "session_id": { "terms": { "field": "session_id" } } }],
    });
    if let Some(session_id) = after {
        composite["after"] = json!({ "session_id": session_id });
    }

    json!({
        "size": 0,
        "track_total_hits": false,
        "query": query,
        "aggs": {
            "sessions": {
                "composite": composite,
                "aggs": {
                    "first_event_at": { "min": { "field": "occured_at" } },
                    "last_event_at": { "max": { "field": "occured_at" } },
                },
            },
        },
    })
}

/// The narrowest bucket width that splits `[from, to)` into at most
/// `MAX_HISTOGRAM_BUCKETS` buckets. Picking it here, not in the client, keeps
/// a request from asking OpenSearch for millions of buckets.
pub fn histogram_interval(
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<&'static str, RequestError> {
    let seconds = (to - from).num_seconds();
    if seconds <= 0 {
        return Err(RequestError::InvalidTimeRange);
    }
    HISTOGRAM_INTERVALS
        .iter()
        .find(|(_, width)| seconds <= width * MAX_HISTOGRAM_BUCKETS)
        .map(|(interval, _)| *interval)
        .ok_or(RequestError::InvalidTimeRange)
}

/// `_search` body counting events per `interval` over `[from, to)`. The
/// bounds make OpenSearch return the empty buckets too.
pub fn histogram_body(
    query: Value,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    interval: &str,
) -> Value {
    json!({
        "size": 0,
        "track_total_hits": false,
        "query": query,
        "aggs": {
            "events": {
                "date_histogram": {
                    "field": "occured_at",
                    "fixed_interval": interval,
                    "min_doc_count": 0,
                    "extended_bounds": {
                        "min": timestamp(from),
                        // `to` is exclusive.
                        "max": timestamp(to - TimeDelta::milliseconds(1)),
                    },
                },
            },
        },
    })
}

/// Checks that `field` can be aggregated and resolves the number of values
/// to return.
pub fn facet_size(field: &str, requested: Option<usize>) -> Result<usize, RequestError> {
    if !FACET_FIELDS.contains(&field) {
        return Err(RequestError::UnknownFacetField);
    }
    match requested {
        None => Ok(DEFAULT_FACET_SIZE),
        Some(size) if (1..=MAX_FACET_SIZE).contains(&size) => Ok(size),
        Some(_) => Err(RequestError::InvalidFacetSize),
    }
}

/// `_search` body for the most frequent values of `field`.
pub fn facets_body(query: Value, field: &str, size: usize) -> Value {
    json!({
        "size": 0,
        "track_total_hits": true,
        "query": query,
        "aggs": { "values": { "terms": { "field": field, "size": size } } },
    })
}

/// Removes the index name and the org id from text OpenSearch produced,
/// so an error shown to the client does not reveal how events are partitioned.
pub fn redact(text: &str, index: &str, scope: &Scope) -> String {
    text.replace(index, "[index]")
        .replace(&scope.org_id, "[redacted]")
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORG: &str = "0198c0de-0000-7000-8000-000000000001";
    const PROJ: &str = "0198c0de-0000-7000-8000-000000000002";

    fn scope() -> Scope {
        Scope {
            org_id: ORG.into(),
            proj_id: PROJ.into(),
        }
    }

    fn check(query: Value) -> Result<(), RequestError> {
        check_query(&query, &scope())
    }

    fn bounded(query: Value) -> bool {
        require_time_range(Some(&query)).is_ok()
    }

    fn range(field: &str, bounds: Value) -> Value {
        json!({ "range": { field: bounds } })
    }

    fn full_range() -> Value {
        range("occured_at", json!({ "gte": "now-1d", "lt": "now" }))
    }

    #[test]
    fn ordinary_queries_pass_the_check() {
        for query in [
            json!({ "match_all": {} }),
            json!({ "term": { "name": "checkout_viewed" } }),
            json!({ "bool": {
                "must": [{ "match": { "properties.plan": "pro" } }],
                "should": [{ "wildcard": { "anon_id": { "value": "anon-*" } } }],
                "must_not": [{ "exists": { "field": "actor_id" } }],
            }}),
            json!({ "nested_type_we_do_not_know": { "anything": [1, 2, 3] } }),
        ] {
            assert_eq!(check(query.clone()), Ok(()), "{query}");
        }
    }

    #[test]
    fn a_query_must_be_an_object() {
        for query in [json!("match_all"), json!([]), json!(null), json!(1)] {
            assert_eq!(check(query), Err(RequestError::QueryNotAnObject));
        }
    }

    #[test]
    fn the_projects_own_org_id_is_accepted() {
        for query in [
            json!({ "term": { "org_id": ORG } }),
            json!({ "term": { "org_id": { "value": ORG, "boost": 2.0 } } }),
            json!({ "terms": { "org_id": [ORG] } }),
            json!({ "match": { "org_id": { "query": ORG, "operator": "and" } } }),
            json!({ "bool": { "filter": [{ "term": { "org_id": ORG } }] } }),
        ] {
            assert_eq!(check(query.clone()), Ok(()), "{query}");
        }
    }

    #[test]
    fn another_org_id_is_rejected_at_any_depth() {
        for query in [
            json!({ "term": { "org_id": "someone-else" } }),
            json!({ "term": { "org_id": { "value": "someone-else" } } }),
            json!({ "terms": { "org_id": [ORG, "someone-else"] } }),
            json!({ "terms": { "org_id": [] } }),
            json!({ "wildcard": { "org_id": { "value": "*" } } }),
            json!({ "range": { "org_id": { "gte": "a" } } }),
            json!({ "bool": { "should": [
                { "bool": { "must_not": [{ "term": { "org_id": "someone-else" } }] } },
            ]}}),
        ] {
            assert_eq!(check(query.clone()), Err(RequestError::WrongOrg), "{query}");
        }
    }

    #[test]
    fn queries_that_name_another_index_are_rejected() {
        let cases = [
            (
                json!({ "terms": { "anon_id": { "index": "events-other", "id": "1", "path": "anon_id" } } }),
                "index",
            ),
            (
                json!({ "more_like_this": { "fields": ["name"], "like": [{ "_index": "events-other", "_id": "1" }] } }),
                "_index",
            ),
            (json!({ "term": { "_index": "events-other" } }), "_index"),
            (json!({ "wrapper": { "query": "eyJtYXRjaF9hbGwiOnt9fQ==" } }), "wrapper"),
            (
                json!({ "bool": { "filter": [{ "bool": { "should": [
                    { "wrapper": { "query": "eyJtYXRjaF9hbGwiOnt9fQ==" } },
                ]}}]}}),
                "wrapper",
            ),
        ];

        for (query, key) in cases {
            assert_eq!(
                check(query.clone()),
                Err(RequestError::ForbiddenKey(key.into())),
                "{query}"
            );
        }
    }

    #[test]
    fn a_field_whose_name_only_contains_index_is_allowed() {
        assert_eq!(check(json!({ "term": { "properties.index": "3" } })), Ok(()));
    }

    #[test]
    fn a_missing_query_has_no_time_range() {
        assert_eq!(
            require_time_range(None),
            Err(RequestError::MissingTimeRange)
        );
    }

    #[test]
    fn a_two_sided_range_bounds_the_query() {
        assert!(bounded(full_range()));
        assert!(bounded(range(
            "occured_at",
            json!({ "gt": "2026-09-01", "lte": "2026-09-02" })
        )));
        assert!(bounded(range(
            "@timestamp",
            json!({ "gte": "now-1h", "lte": "now" })
        )));
    }

    #[test]
    fn a_range_inside_must_or_filter_bounds_the_query() {
        assert!(bounded(json!({ "bool": { "filter": [full_range()] } })));
        assert!(bounded(json!({ "bool": { "filter": full_range() } })));
        assert!(bounded(json!({ "bool": {
            "must": [
                { "term": { "name": "checkout_viewed" } },
                { "bool": { "filter": [full_range()] } },
            ],
        }})));
    }

    #[test]
    fn queries_without_a_bounding_range_are_refused() {
        for query in [
            json!({ "match_all": {} }),
            json!({ "term": { "name": "checkout_viewed" } }),
            // One-sided, or an explicit null bound.
            range("occured_at", json!({ "gte": "now-1d" })),
            range("occured_at", json!({ "lte": "now" })),
            range("occured_at", json!({ "gte": "now-1d", "lte": null })),
            // A range on some other field.
            range("arrived_at", json!({ "gte": "now-1d", "lte": "now" })),
            // Positions that do not restrict every match.
            json!({ "bool": { "should": [full_range()] } }),
            json!({ "bool": { "must_not": [full_range()] } }),
            json!({ "bool": { "must": [{ "bool": { "should": [full_range()] } }] } }),
            json!({ "constant_score": { "filter": full_range() } }),
        ] {
            assert!(!bounded(query.clone()), "{query}");
        }
    }

    #[test]
    fn the_scope_wraps_the_client_query() {
        let client = json!({ "term": { "name": "checkout_viewed" } });

        assert_eq!(
            scoped_query(Some(&client), &scope(), Vec::new()),
            json!({ "bool": {
                "must": [{ "term": { "name": "checkout_viewed" } }],
                "filter": [
                    { "term": { "org_id": ORG } },
                    { "term": { "proj_id": PROJ } },
                ],
            }})
        );
    }

    #[test]
    fn no_client_query_matches_everything_in_scope() {
        let query = scoped_query(None, &scope(), Vec::new());

        assert_eq!(query["bool"]["must"], json!([{ "match_all": {} }]));
        assert_eq!(query["bool"]["filter"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn narrowing_filters_follow_the_scope() {
        let query = scoped_query(None, &scope(), vec![session_filter("session-1")]);

        assert_eq!(
            query["bool"]["filter"][2],
            json!({ "term": { "session_id": "session-1" } })
        );
    }

    #[test]
    fn the_events_body_asks_for_one_extra_hit() {
        let body = events_body(json!({ "match_all": {} }), SortOrder::Desc, 50, None);

        assert_eq!(
            body,
            json!({
                "size": 51,
                "track_total_hits": false,
                "query": { "match_all": {} },
                "sort": [
                    { "occured_at": { "order": "desc" } },
                    { "id": { "order": "asc" } },
                ],
            })
        );
    }

    #[test]
    fn the_events_body_resumes_after_a_cursor() {
        let after = vec![json!(1790675429805_i64), json!("959bd8a7")];

        let body = events_body(
            json!({ "match_all": {} }),
            SortOrder::Asc,
            10,
            Some(after.clone()),
        );

        assert_eq!(body["search_after"], Value::Array(after));
        assert_eq!(body["sort"][0]["occured_at"]["order"], "asc");
    }

    #[test]
    fn the_sessions_body_aggregates_without_hits() {
        let body = sessions_body(json!({ "match_all": {} }), 50, None);

        assert_eq!(body["size"], 0);
        let sessions = &body["aggs"]["sessions"];
        assert_eq!(
            sessions["composite"],
            json!({
                "size": 51,
                "sources": [{ "session_id": { "terms": { "field": "session_id" } } }],
            })
        );
        assert_eq!(
            sessions["aggs"]["first_event_at"],
            json!({ "min": { "field": "occured_at" } })
        );
        assert_eq!(
            sessions["aggs"]["last_event_at"],
            json!({ "max": { "field": "occured_at" } })
        );
    }

    #[test]
    fn the_sessions_body_resumes_after_a_session() {
        let body = sessions_body(json!({ "match_all": {} }), 50, Some("session-7"));

        assert_eq!(
            body["aggs"]["sessions"]["composite"]["after"],
            json!({ "session_id": "session-7" })
        );
    }

    fn at(time: &str) -> DateTime<Utc> {
        time.parse().unwrap()
    }

    #[test]
    fn the_time_filter_is_half_open() {
        assert_eq!(
            time_filter(at("2026-09-01T10:00:00Z"), at("2026-09-01T11:00:00Z")),
            json!({ "range": { "occured_at": {
                "gte": "2026-09-01T10:00:00.000Z",
                "lt": "2026-09-01T11:00:00.000Z",
            }}})
        );
    }

    #[test]
    fn the_histogram_interval_keeps_the_bucket_count_bounded() {
        let from = at("2026-09-01T00:00:00Z");
        let cases = [
            (TimeDelta::seconds(1), "1s"),
            (TimeDelta::seconds(60), "1s"),
            (TimeDelta::seconds(61), "5s"),
            (TimeDelta::minutes(15), "30s"),
            (TimeDelta::hours(1), "1m"),
            (TimeDelta::hours(24), "30m"),
            (TimeDelta::days(7), "3h"),
            (TimeDelta::days(30), "12h"),
            (TimeDelta::days(90), "7d"),
            (TimeDelta::days(365), "7d"),
            (TimeDelta::days(365 * 5), "365d"),
        ];

        for (span, interval) in cases {
            assert_eq!(histogram_interval(from, from + span), Ok(interval), "{span}");
        }
    }

    #[test]
    fn an_empty_backwards_or_endless_range_has_no_interval() {
        let from = at("2026-09-01T00:00:00Z");

        for to in [
            from,
            from - TimeDelta::hours(1),
            from + TimeDelta::days(365 * 61),
        ] {
            assert_eq!(
                histogram_interval(from, to),
                Err(RequestError::InvalidTimeRange)
            );
        }
    }

    #[test]
    fn the_histogram_body_asks_for_empty_buckets_inside_the_range() {
        let body = histogram_body(
            json!({ "match_all": {} }),
            at("2026-09-01T10:00:00Z"),
            at("2026-09-01T11:00:00Z"),
            "1m",
        );

        assert_eq!(body["size"], 0);
        assert_eq!(
            body["aggs"]["events"]["date_histogram"],
            json!({
                "field": "occured_at",
                "fixed_interval": "1m",
                "min_doc_count": 0,
                "extended_bounds": {
                    "min": "2026-09-01T10:00:00.000Z",
                    "max": "2026-09-01T10:59:59.999Z",
                },
            })
        );
    }

    #[test]
    fn only_aggregatable_fields_can_be_facets() {
        assert_eq!(facet_size("name", None), Ok(DEFAULT_FACET_SIZE));
        assert_eq!(facet_size("system_properties.geo.country", Some(50)), Ok(50));

        for field in ["org_id", "proj_id", "id", "properties.plan", "occured_at", ""] {
            assert_eq!(
                facet_size(field, None),
                Err(RequestError::UnknownFacetField),
                "{field}"
            );
        }
    }

    #[test]
    fn the_facet_size_is_bounded() {
        for size in [0, MAX_FACET_SIZE + 1] {
            assert_eq!(
                facet_size("name", Some(size)),
                Err(RequestError::InvalidFacetSize)
            );
        }
    }

    #[test]
    fn the_facets_body_counts_the_top_values() {
        let body = facets_body(json!({ "match_all": {} }), "name", 5);

        assert_eq!(
            body,
            json!({
                "size": 0,
                "track_total_hits": true,
                "query": { "match_all": {} },
                "aggs": { "values": { "terms": { "field": "name", "size": 5 } } },
            })
        );
    }

    #[test]
    fn redaction_hides_the_index_and_the_org() {
        let index = format!("events-{ORG}");
        let text = format!("[{index}/xjCVa9zQ] failed to create query for org {ORG}");

        assert_eq!(
            redact(&text, &index, &scope()),
            "[[index]/xjCVa9zQ] failed to create query for org [redacted]"
        );
    }
}
