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
    event_for("merchant-1", "acme-shop-42")
}

fn event_for(org: &str, workspace: &str) -> String {
    format!(
        r#"{{"envelop_version":"1.0","id":"{ID}","name":"payment_initiated","org_id":"{org}","workspace_id":"{workspace}","anon_id":"anon-42","occured_at":"2026-09-02T10:30:00Z","properties":{{"amount":100}}}}"#
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
fn writes_each_workspace_to_its_own_index() {
    let template = index("events-{org_id}-{workspace_id}");

    for (org, workspace) in [
        ("acme-k3x9qa", "web-shop-9x2k1a"),
        ("acme-k3x9qa", "app-store-4m7p2q"),
        ("globex-8d2l0z", "web-shop-1b5c9e"),
    ] {
        let payload = event_for(org, workspace);
        let doc = prepare(Some(payload.as_bytes()), &template, 1024).unwrap();

        let action: serde_json::Value = serde_json::from_str(&lines(&doc)[0]).unwrap();
        assert_eq!(
            action["create"]["_index"],
            format!("events-{org}-{workspace}")
        );
    }
}

#[test]
fn rejects_workspaces_that_cannot_form_an_index_name() {
    let template = index("events-{org_id}-{workspace_id}");

    for workspace in ["Web-shop", "", "a,b", "a*"] {
        let payload = event_for("acme-k3x9qa", workspace);
        let rejection = prepare(Some(payload.as_bytes()), &template, 1024).unwrap_err();

        assert_eq!(
            rejection.class, "invalid_workspace",
            "workspace `{workspace}`"
        );
    }
}

#[test]
fn rejects_orgs_that_cannot_form_an_index_name() {
    let template = index("events-{org_id}-{workspace_id}");

    for org in ["Merchant-1", "", "a,b", "a*"] {
        let payload = event_for(org, "acme-shop-42");
        let rejection = prepare(Some(payload.as_bytes()), &template, 1024).unwrap_err();

        assert_eq!(rejection.class, "invalid_org", "org `{org}`");
    }
}
