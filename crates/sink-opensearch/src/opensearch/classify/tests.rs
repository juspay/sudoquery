use super::*;

#[test]
fn request_statuses() {
    let cases = [
        (200, RequestClass::Items),
        (429, RequestClass::Retry { alert: false }),
        (502, RequestClass::Retry { alert: false }),
        (503, RequestClass::Retry { alert: false }),
        (504, RequestClass::Retry { alert: false }),
        (400, RequestClass::Retry { alert: true }),
        (401, RequestClass::Retry { alert: true }),
        (403, RequestClass::Retry { alert: true }),
        (404, RequestClass::Retry { alert: true }),
        (413, RequestClass::Retry { alert: true }),
        (500, RequestClass::Retry { alert: true }),
    ];

    for (status, expected) in cases {
        assert_eq!(classify_request(status), expected, "HTTP {status}");
    }
}

#[test]
fn item_statuses() {
    let cases = [
        (200, ItemClass::Done),
        (201, ItemClass::Done),
        (409, ItemClass::AlreadyWritten),
        (429, ItemClass::Retry { alert: false }),
        (500, ItemClass::Retry { alert: false }),
        (503, ItemClass::Retry { alert: false }),
        (400, ItemClass::Reject),
        (401, ItemClass::Retry { alert: true }),
        (403, ItemClass::Retry { alert: true }),
        (404, ItemClass::Retry { alert: true }),
        (413, ItemClass::Retry { alert: true }),
        (202, ItemClass::Retry { alert: false }),
        (0, ItemClass::Retry { alert: false }),
    ];

    for (status, expected) in cases {
        assert_eq!(classify_item(status), expected, "item status {status}");
    }
}
