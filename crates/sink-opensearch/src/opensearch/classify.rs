//! Decides what happens after each kind of `_bulk` failure.
//!
//! Nothing is dropped: a failure is either retried or, when the document
//! itself is at fault, sent to the dead letter queue.

#[derive(Debug, PartialEq, Eq)]
pub enum RequestClass {
    /// HTTP 200: look at each item.
    Items,
    /// Resend the whole request. `alert` marks failures an operator must fix,
    /// such as bad credentials or a malformed request, where retrying alone
    /// won't help but dead-lettering good data would be worse.
    Retry { alert: bool },
}

#[derive(Debug, PartialEq, Eq)]
pub enum ItemClass {
    Done,
    /// 409 on `create`: a replay or a resent event found the document already
    /// written.
    AlreadyWritten,
    Retry {
        alert: bool,
    },
    /// The document can never be indexed as-is.
    Reject,
}

pub fn classify_request(status: u16) -> RequestClass {
    match status {
        200 => RequestClass::Items,
        429 | 502 | 503 | 504 => RequestClass::Retry { alert: false },
        _ => RequestClass::Retry { alert: true },
    }
}

pub fn classify_item(status: u16) -> ItemClass {
    match status {
        200 | 201 => ItemClass::Done,
        409 => ItemClass::AlreadyWritten,
        429 | 500..=599 => ItemClass::Retry { alert: false },
        // Mapping and parse errors: only this document is at fault.
        400 => ItemClass::Reject,
        // Anything else in 4xx is about the cluster, not the document: a
        // missing index with auto-create off (404), a disk-full or read-only
        // block (403), bad credentials. Dead-lettering would send every event
        // to the DLQ, so block until an operator fixes it.
        401..=499 => ItemClass::Retry { alert: true },
        _ => ItemClass::Retry { alert: false },
    }
}

#[cfg(test)]
mod tests;
