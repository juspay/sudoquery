//! Parses `_bulk` responses. Items are matched to documents by position.

use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, PartialEq, Eq)]
pub struct ItemResult {
    pub status: u16,
    /// `type: reason` from the item's error, when there is one.
    pub error: Option<String>,
}

#[derive(Deserialize)]
struct BulkResponse {
    items: Vec<HashMap<String, BulkItem>>,
}

#[derive(Deserialize)]
struct BulkItem {
    status: u16,
    #[serde(default)]
    error: Option<serde_json::Value>,
}

/// Returns one result per document, in request order.
pub fn parse(body: &[u8], expected: usize) -> Result<Vec<ItemResult>, String> {
    let response: BulkResponse = serde_json::from_slice(body)
        .map_err(|error| format!("unparseable bulk response: {error}"))?;

    if response.items.len() != expected {
        return Err(format!(
            "bulk response has {} items for {expected} documents",
            response.items.len()
        ));
    }

    response
        .items
        .into_iter()
        .map(|item| {
            // Each item is keyed by its action, e.g. {"create": {...}}.
            let item = item
                .into_values()
                .next()
                .ok_or_else(|| "bulk response has an empty item".to_owned())?;
            Ok(ItemResult {
                status: item.status,
                error: item.error.map(describe_error),
            })
        })
        .collect()
}

fn describe_error(error: serde_json::Value) -> String {
    let field = |name: &str| error.get(name).and_then(serde_json::Value::as_str);
    match (field("type"), field("reason")) {
        (Some(kind), Some(reason)) => format!("{kind}: {reason}"),
        (Some(kind), None) => kind.to_owned(),
        _ => error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_items_by_position() {
        let body = br#"{
            "took": 3,
            "errors": true,
            "items": [
                {"create": {"_index": "events", "_id": "a", "status": 201}},
                {"create": {"_index": "events", "_id": "b", "status": 409,
                    "error": {"type": "version_conflict_engine_exception", "reason": "document already exists"}}},
                {"create": {"_index": "events", "_id": "c", "status": 400,
                    "error": {"type": "mapper_parsing_exception", "reason": "failed to parse field [properties.amount]",
                              "caused_by": {"type": "number_format_exception"}}}}
            ]
        }"#;

        let items = parse(body, 3).unwrap();

        assert_eq!(
            items,
            vec![
                ItemResult {
                    status: 201,
                    error: None
                },
                ItemResult {
                    status: 409,
                    error: Some(
                        "version_conflict_engine_exception: document already exists".into()
                    ),
                },
                ItemResult {
                    status: 400,
                    error: Some(
                        "mapper_parsing_exception: failed to parse field [properties.amount]"
                            .into()
                    ),
                },
            ]
        );
    }

    #[test]
    fn rejects_a_count_mismatch() {
        let body = br#"{"errors": false, "items": [{"create": {"status": 201}}]}"#;

        let error = parse(body, 2).unwrap_err();

        assert!(error.contains("1 items for 2 documents"));
    }

    #[test]
    fn rejects_unparseable_bodies() {
        for body in [&b"<html>gateway</html>"[..], br#"{"errors": false}"#] {
            assert!(parse(body, 1).unwrap_err().starts_with("unparseable"));
        }
    }

    #[test]
    fn rejects_empty_items() {
        let body = br#"{"errors": true, "items": [{}]}"#;

        assert!(parse(body, 1).is_err());
    }

    #[test]
    fn keeps_errors_that_are_not_objects() {
        let body = br#"{"errors": true, "items": [{"create": {"status": 500, "error": "boom"}}]}"#;

        let items = parse(body, 1).unwrap();

        assert_eq!(items[0].error.as_deref(), Some("\"boom\""));
    }
}
