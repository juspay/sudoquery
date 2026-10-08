//! Pure S3 object model: JSONL assembly, content hash, keys, metadata (D4, D5, D7, D10, D18).

use crate::buffer::Entry;
use crate::routing::GroupKey;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// zstd stream-compression level (D18: 3, single-threaded).
pub const ZSTD_LEVEL: i32 = 3;

/// Uncompressed JSONL body: each event's raw payload bytes + trailing newline, in
/// consumption order (D7 — never re-serialized).
pub fn jsonl_body(entries: &[Entry]) -> Vec<u8> {
    let mut body = Vec::new();
    for entry in entries {
        body.extend_from_slice(&entry.raw);
        body.push(b'\n');
    }
    body
}

/// First 16 lowercase hex chars of the sha256 of the uncompressed body (D5).
pub fn content_hash16(body: &[u8]) -> String {
    let digest = Sha256::digest(body);
    hex::encode(&digest[..8])
}

/// zstd stream-encode at level 3 (D18).
///
/// # Errors
/// Returns the zstd encoder's I/O error.
pub fn compress(body: &[u8]) -> std::io::Result<Vec<u8>> {
    zstd::stream::encode_all(body, ZSTD_LEVEL)
}

/// `{S3_PREFIX?}/{org}/{project_id}/dt=YYYY-MM-DD/hour=HH/{hash}.jsonl.zst` (D4).
pub fn group_object_key(prefix: Option<&str>, group: &GroupKey, hash16: &str) -> String {
    let path = format!(
        "{}/{}/dt={}/hour={:02}/{}.jsonl.zst",
        group.org, group.project_id, group.dt, group.hour, hash16
    );
    match prefix {
        Some(prefix) => format!("{prefix}/{path}"),
        None => path,
    }
}

/// `{S3_PREFIX?}/_quarantine/{hash}.jsonl.zst` (D12).
pub fn quarantine_object_key(prefix: Option<&str>, hash16: &str) -> String {
    match prefix {
        Some(prefix) => format!("{prefix}/_quarantine/{hash16}.jsonl.zst"),
        None => format!("_quarantine/{hash16}.jsonl.zst"),
    }
}

/// Object metadata (D10): `source` (omitted when `SOURCE_ID` is unset), `event-count`,
/// `ranges`, `created-at` (RFC3339).
pub fn object_metadata(
    source_id: Option<&str>,
    entries: &[Entry],
    created_at: DateTime<Utc>,
) -> HashMap<String, String> {
    let mut metadata = HashMap::new();
    if let Some(source) = source_id {
        metadata.insert("source".to_string(), source.to_string());
    }
    metadata.insert("event-count".to_string(), entries.len().to_string());
    metadata.insert("ranges".to_string(), offset_ranges(entries));
    metadata.insert("created-at".to_string(), created_at.to_rfc3339());
    metadata
}

/// `topic:partition:first:last` per distinct `(topic, partition)` in first-seen
/// order, `;`-joined; first/last are the min/max offsets of this file's entries (D10).
fn offset_ranges(entries: &[Entry]) -> String {
    let mut ranges: Vec<((String, i32), i64, i64)> = Vec::new();
    for entry in entries {
        let coords = (entry.topic.clone(), entry.partition);
        match ranges
            .iter_mut()
            .find(|(existing, _, _)| *existing == coords)
        {
            Some((_, first, last)) => {
                if entry.offset < *first {
                    *first = entry.offset;
                }
                if entry.offset > *last {
                    *last = entry.offset;
                }
            }
            None => ranges.push((coords, entry.offset, entry.offset)),
        }
    }
    ranges
        .iter()
        .map(|((topic, partition), first, last)| format!("{topic}:{partition}:{first}:{last}"))
        .collect::<Vec<_>>()
        .join(";")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Origin;

    fn entry(topic: &str, partition: i32, offset: i64, raw: &str) -> Entry {
        Entry {
            origin: Origin::Quarantined,
            topic: topic.to_string(),
            partition,
            offset,
            raw: raw.as_bytes().to_vec(),
        }
    }

    fn group(org: &str, project_id: &str, dt: &str, hour: u32) -> GroupKey {
        GroupKey {
            org: org.to_string(),
            project_id: project_id.to_string(),
            dt: dt.to_string(),
            hour,
        }
    }

    #[test]
    fn jsonl_body_concatenates_raw_bytes_with_trailing_newlines() {
        let body = jsonl_body(&[
            entry("t", 0, 0, r#"{"id":"a"}"#),
            entry("t", 0, 1, r#"{"id":"b"}  "#),
        ]);
        assert_eq!(body, b"{\"id\":\"a\"}\n{\"id\":\"b\"}  \n");
    }

    #[test]
    fn content_hash16_is_16_lowercase_hex_and_content_addressed() {
        let hash = content_hash16(b"{\"id\":\"a\"}\n");
        assert_eq!(hash.len(), 16);
        assert!(
            hash.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );

        // Same bytes -> same hash; different bytes -> different hash (D5).
        assert_eq!(hash, content_hash16(b"{\"id\":\"a\"}\n"));
        assert_ne!(hash, content_hash16(b"{\"id\":\"b\"}\n"));
        // Newline separation changes content, so it changes the hash.
        assert_ne!(
            content_hash16(b"{\"id\":\"a\"}\n"),
            content_hash16(b"{\"id\":\"a\"}")
        );
    }

    #[test]
    fn group_key_includes_prefix_when_present() {
        let key = group_object_key(
            Some("env=prod"),
            &group("org-a", "proj-b", "2026-09-24", 3),
            "0123456789abcdef",
        );
        assert_eq!(
            key,
            "env=prod/org-a/proj-b/dt=2026-09-24/hour=03/0123456789abcdef.jsonl.zst"
        );
    }

    #[test]
    fn group_key_omits_prefix_when_absent() {
        let key = group_object_key(
            None,
            &group("org-a", "default", "2026-09-24", 15),
            "0123456789abcdef",
        );
        assert_eq!(
            key,
            "org-a/default/dt=2026-09-24/hour=15/0123456789abcdef.jsonl.zst"
        );
    }

    #[test]
    fn hour_is_zero_padded_to_two_digits() {
        for (hour, expected) in [(0, "00"), (5, "05"), (9, "09"), (23, "23")] {
            let key = group_object_key(
                None,
                &group("o", "p", "2026-09-24", hour),
                "x".repeat(16).as_str(),
            );
            assert!(key.contains(&format!("/hour={expected}/")), "{key}");
        }
    }

    #[test]
    fn quarantine_key_with_and_without_prefix() {
        assert_eq!(
            quarantine_object_key(Some("env=prod"), "abcdef0123456789"),
            "env=prod/_quarantine/abcdef0123456789.jsonl.zst"
        );
        assert_eq!(
            quarantine_object_key(None, "abcdef0123456789"),
            "_quarantine/abcdef0123456789.jsonl.zst"
        );
    }

    #[test]
    fn same_entries_produce_the_same_object_key() {
        let entries = vec![
            entry("t", 0, 0, r#"{"id":"a"}"#),
            entry("t", 0, 1, r#"{"id":"b"}"#),
        ];
        let group = group("o", "p", "2026-09-24", 9);
        let build = || {
            let body = jsonl_body(&entries);
            group_object_key(None, &group, &content_hash16(&body))
        };
        assert_eq!(build(), build());
    }

    #[test]
    fn metadata_carries_source_count_ranges_and_created_at() {
        let created_at = Utc::now();
        let entries = vec![
            entry("events", 0, 5, r#"{"id":"a"}"#),
            entry("events", 0, 3, r#"{"id":"b"}"#),
            entry("events", 1, 2, r#"{"id":"c"}"#),
        ];
        let metadata = object_metadata(Some("aws-prod-a"), &entries, created_at);

        assert_eq!(
            metadata.get("source").map(String::as_str),
            Some("aws-prod-a")
        );
        assert_eq!(metadata.get("event-count").map(String::as_str), Some("3"));
        assert_eq!(
            metadata.get("ranges").map(String::as_str),
            Some("events:0:3:5;events:1:2:2")
        );
        let stored = metadata.get("created-at").map(String::as_str).unwrap();
        assert_eq!(
            DateTime::parse_from_rfc3339(stored)
                .ok()
                .map(|parsed| parsed.with_timezone(&Utc)),
            Some(created_at)
        );
    }

    #[test]
    fn metadata_omits_source_when_unset() {
        let metadata = object_metadata(None, &[entry("t", 0, 0, "x")], Utc::now());
        assert!(!metadata.contains_key("source"));
        assert_eq!(metadata.get("event-count").map(String::as_str), Some("1"));
    }

    #[test]
    fn ranges_use_min_max_offsets_per_partition() {
        let entries = vec![
            entry("t", 0, 9, "a"),
            entry("t", 0, 2, "b"),
            entry("t", 0, 7, "c"),
        ];
        assert_eq!(offset_ranges(&entries), "t:0:2:9");

        let single = vec![entry("t", 3, 42, "a")];
        assert_eq!(offset_ranges(&single), "t:3:42:42");
    }

    #[test]
    fn zstd_round_trip_preserves_bytes() {
        let body = jsonl_body(&[
            entry("t", 0, 0, r#"{"id":"a","properties":{"amount":100}}"#),
            entry("t", 0, 1, r#"{"id":"b"}"#),
        ]);
        let compressed = compress(&body).unwrap();
        assert!(!compressed.is_empty());
        assert_ne!(compressed, body);
        let restored = zstd::stream::decode_all(compressed.as_slice()).unwrap();
        assert_eq!(restored, body);
    }
}
