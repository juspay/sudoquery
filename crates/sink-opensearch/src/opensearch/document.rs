//! Turns an event payload into one `_bulk` `create` operation.

use std::borrow::Cow;

use bytes::{Bytes, BytesMut};
use canonical_event::CanonicalEvent;
use serde::Serialize;

use crate::config::{IndexTemplate, InvalidId};
use crate::runtime::Rejection;

/// A `create` action line followed by its source line, both
/// newline-terminated, ready to append to a `_bulk` body.
#[derive(Clone, Debug)]
pub struct BulkDoc(Bytes);

impl BulkDoc {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

#[derive(Serialize)]
struct Action<'a> {
    create: Target<'a>,
}

#[derive(Serialize)]
struct Target<'a> {
    #[serde(rename = "_index")]
    index: &'a str,
    #[serde(rename = "_id")]
    id: &'a str,
}

/// A payload that parsed as a canonical event.
pub struct Decoded<'a> {
    pub event: CanonicalEvent,
    /// The payload on one line: the original bytes, unless they held a line
    /// break.
    source: Cow<'a, [u8]>,
}

/// Validates the payload as a canonical event.
pub fn decode(payload: Option<&[u8]>) -> Result<Decoded<'_>, Rejection> {
    let payload = payload
        .filter(|payload| !payload.is_empty())
        .ok_or_else(|| Rejection::new("decode", "empty payload"))?;
    let event: CanonicalEvent = serde_json::from_slice(payload)
        .map_err(|error| Rejection::new("decode", format!("not a canonical event: {error}")))?;

    // NDJSON can't hold a line break inside a document; pretty-printed JSON
    // is re-encoded on one line.
    let source = if payload.contains(&b'\n') {
        let value: serde_json::Value = serde_json::from_slice(payload)
            .map_err(|error| Rejection::new("decode", format!("not JSON: {error}")))?;
        let compacted = serde_json::to_vec(&value)
            .map_err(|error| Rejection::new("decode", format!("failed to re-encode: {error}")))?;
        Cow::Owned(compacted)
    } else {
        Cow::Borrowed(payload)
    };

    Ok(Decoded { event, source })
}

/// Builds the event's `create` operation into its workspace's index, with the
/// event's `id` as the document ID so a replay can't create a duplicate.
pub fn build(
    decoded: &Decoded<'_>,
    index: &IndexTemplate,
    max_bytes: usize,
) -> Result<BulkDoc, Rejection> {
    let event = &decoded.event;
    let index =
        index
            .render(&event.org_id, &event.workspace_id)
            .map_err(|invalid| match invalid {
                InvalidId::Org(reason) => Rejection::new("invalid_org", reason),
                InvalidId::Workspace(reason) => Rejection::new("invalid_workspace", reason),
            })?;
    let id = decoded.event.id().to_string();
    let action = serde_json::to_vec(&Action {
        create: Target {
            index: &index,
            id: &id,
        },
    })
    .map_err(|error| Rejection::new("decode", format!("failed to encode action: {error}")))?;

    let size = action.len() + decoded.source.len() + 2;
    if size > max_bytes {
        return Err(Rejection::new(
            "too_large",
            format!("document is {size} bytes; the limit is `batch.max_bytes` ({max_bytes})"),
        ));
    }

    let mut doc = BytesMut::with_capacity(size);
    doc.extend_from_slice(&action);
    doc.extend_from_slice(b"\n");
    doc.extend_from_slice(&decoded.source);
    doc.extend_from_slice(b"\n");
    Ok(BulkDoc(doc.freeze()))
}

#[cfg(test)]
mod tests;
