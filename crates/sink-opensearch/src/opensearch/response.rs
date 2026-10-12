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
mod tests;
