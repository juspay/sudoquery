//! Turns an event payload into one `_bulk` `create` operation.

use std::borrow::Cow;

use bytes::{Bytes, BytesMut};
use canonical_event::CanonicalEvent;
use serde::Serialize;

use crate::config::IndexTemplate;
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

/// Builds the event's `create` operation into its tenant's index, with the
/// event's `id` as the document ID so a replay can't create a duplicate.
pub fn build(
    decoded: &Decoded<'_>,
    index: &IndexTemplate,
    max_bytes: usize,
) -> Result<BulkDoc, Rejection> {
    let index = index
        .render(&decoded.event.tenant_id)
        .map_err(|reason| Rejection::new("invalid_tenant", reason))?;
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
mod tests {
    use super::*;

    fn prepare(
        payload: Option<&[u8]>,
        index: &IndexTemplate,
        max_bytes: usize,
    ) -> Result<BulkDoc, Rejection> {
        build(&decode(payload)?, index, max_bytes)
    }

    const ID: &str = "0b6bd7e7-1a4b-4d12-8fd3-9f8f0f2a1b2c";

    fn event() -> String {
        event_for("merchant-1")
    }

    fn event_for(tenant: &str) -> String {
        format!(
            r#"{{"envelop_version":"1.0","id":"{ID}","name":"payment_initiated","tenant_id":"{tenant}","anon_id":"anon-42","occured_at":"2026-09-02T10:30:00Z","properties":{{"amount":100}}}}"#
        )
    }

    fn index(template: &str) -> IndexTemplate {
        IndexTemplate::parse(template).unwrap()
    }

    fn lines(doc: &BulkDoc) -> Vec<String> {
        let text = std::str::from_utf8(doc.as_bytes()).unwrap();
        assert!(text.ends_with('\n'));
        text.lines().map(str::to_owned).collect()
    }

    #[test]
    fn builds_a_create_operation_keyed_by_event_id() {
        let payload = event();

        let doc = prepare(Some(payload.as_bytes()), &index("events"), 1024).unwrap();

        assert_eq!(
            lines(&doc),
            vec![
                format!(r#"{{"create":{{"_index":"events","_id":"{ID}"}}}}"#),
                payload,
            ]
        );
        assert_eq!(doc.len(), doc.as_bytes().len());
    }

    #[test]
    fn re_encodes_pretty_printed_payloads_on_one_line() {
        let value: serde_json::Value = serde_json::from_str(&event()).unwrap();
        let pretty = serde_json::to_string_pretty(&value).unwrap();

        let doc = prepare(Some(pretty.as_bytes()), &index("events"), 4096).unwrap();

        let lines = lines(&doc);
        assert_eq!(lines.len(), 2);
        let source: serde_json::Value = serde_json::from_str(&lines[1]).unwrap();
        assert_eq!(source, value);
    }

    #[test]
    fn rejects_missing_and_empty_payloads() {
        for payload in [None, Some(&b""[..])] {
            let rejection = prepare(payload, &index("events"), 1024).unwrap_err();

            assert_eq!(rejection.class, "decode");
            assert_eq!(rejection.reason, "empty payload");
        }
    }

    #[test]
    fn rejects_payloads_that_are_not_canonical_events() {
        for payload in [
            &b"not json"[..],
            br#"{"id":"not-a-uuid"}"#,
            br#"{"name":"x"}"#,
        ] {
            let rejection = prepare(Some(payload), &index("events"), 1024).unwrap_err();

            assert_eq!(rejection.class, "decode");
            assert!(rejection.reason.starts_with("not a canonical event"));
        }
    }

    #[test]
    fn rejects_documents_over_the_batch_byte_limit() {
        let payload = event();

        let rejection = prepare(Some(payload.as_bytes()), &index("events"), 100).unwrap_err();

        assert_eq!(rejection.class, "too_large");
    }

    #[test]
    fn writes_each_tenant_to_its_own_index() {
        let template = index("events-{tenant_id}");

        for tenant in ["merchant-1", "merchant-2"] {
            let payload = event_for(tenant);
            let doc = prepare(Some(payload.as_bytes()), &template, 1024).unwrap();

            let action: serde_json::Value = serde_json::from_str(&lines(&doc)[0]).unwrap();
            assert_eq!(action["create"]["_index"], format!("events-{tenant}"));
        }
    }

    #[test]
    fn rejects_tenants_that_cannot_form_an_index_name() {
        let template = index("events-{tenant_id}");

        for tenant in ["Merchant-1", "", "a,b", "a*"] {
            let payload = event_for(tenant);
            let rejection = prepare(Some(payload.as_bytes()), &template, 1024).unwrap_err();

            assert_eq!(rejection.class, "invalid_tenant", "tenant `{tenant}`");
        }
    }
}
