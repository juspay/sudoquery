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
                error: Some("version_conflict_engine_exception: document already exists".into()),
            },
            ItemResult {
                status: 400,
                error: Some(
                    "mapper_parsing_exception: failed to parse field [properties.amount]".into()
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
