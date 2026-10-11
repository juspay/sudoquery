//! Cursor pagination shared by every OpenSearch-backed endpoint.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{RequestError, SortOrder};

pub const DEFAULT_PAGE_SIZE: usize = 50;
pub const MAX_PAGE_SIZE: usize = 100;

/// Body of every paginated query endpoint. All fields are optional.
#[derive(Debug, Default, Deserialize)]
pub struct QueryRequest {
    /// An OpenSearch query DSL clause, forwarded once checked and scoped.
    #[serde(default)]
    pub query: Option<Value>,
    /// `next_cursor` of the previous page; absent for the first page.
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub page_size: Option<usize>,
    /// Time order of events; each endpoint has its own default. A cursor is
    /// only valid for the order it was issued under.
    #[serde(default)]
    pub order: Option<SortOrder>,
}

/// One page of results and the cursor that fetches the next one.
#[derive(Debug, Serialize)]
pub struct CursorPage<T> {
    pub items: Vec<T>,
    /// `null` on the last page.
    pub next_cursor: Option<String>,
}

impl<T> CursorPage<T> {
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            next_cursor: None,
        }
    }

    /// Builds a page from rows fetched with a limit of `page_size + 1`, each
    /// paired with the sort key that resumes after it. The extra row only
    /// tells whether another page exists.
    pub fn from_rows(mut rows: Vec<(T, Vec<Value>)>, page_size: usize) -> Self {
        let has_more = rows.len() > page_size;
        rows.truncate(page_size);
        let next_cursor = match rows.last() {
            Some((_, key)) if has_more => Some(encode(key)),
            _ => None,
        };
        Self {
            items: rows.into_iter().map(|(item, _)| item).collect(),
            next_cursor,
        }
    }
}

/// Resolves the requested page size, rejecting values outside
/// `1..=MAX_PAGE_SIZE`.
pub fn page_size(requested: Option<usize>) -> Result<usize, RequestError> {
    match requested {
        None => Ok(DEFAULT_PAGE_SIZE),
        Some(size) if (1..=MAX_PAGE_SIZE).contains(&size) => Ok(size),
        Some(_) => Err(RequestError::InvalidPageSize),
    }
}

pub fn encode(key: &[Value]) -> String {
    URL_SAFE_NO_PAD.encode(Value::from(key).to_string())
}

/// Decodes a cursor into its sort key, which must hold `len` values.
pub fn decode(cursor: &str, len: usize) -> Result<Vec<Value>, RequestError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|_| RequestError::InvalidCursor)?;
    let key: Vec<Value> =
        serde_json::from_slice(&bytes).map_err(|_| RequestError::InvalidCursor)?;
    if key.len() != len {
        return Err(RequestError::InvalidCursor);
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn rows(count: usize) -> Vec<(usize, Vec<Value>)> {
        (0..count)
            .map(|n| (n, vec![json!(n), json!("id")]))
            .collect()
    }

    #[test]
    fn a_cursor_round_trips() {
        let key = vec![json!(1790675429805_i64), json!("959bd8a7")];

        assert_eq!(decode(&encode(&key), 2).unwrap(), key);
    }

    #[test]
    fn malformed_cursors_are_rejected() {
        let not_an_array = URL_SAFE_NO_PAD.encode(r#"{"a":1}"#);

        for cursor in ["not base64!", "bm90IGpzb24", not_an_array.as_str()] {
            assert_eq!(decode(cursor, 2), Err(RequestError::InvalidCursor));
        }
    }

    #[test]
    fn a_cursor_of_the_wrong_length_is_rejected() {
        let session_cursor = encode(&[json!("session-1")]);

        assert_eq!(decode(&session_cursor, 2), Err(RequestError::InvalidCursor));
        assert!(decode(&session_cursor, 1).is_ok());
    }

    #[test]
    fn page_size_defaults_and_is_bounded() {
        assert_eq!(page_size(None), Ok(DEFAULT_PAGE_SIZE));
        assert_eq!(page_size(Some(1)), Ok(1));
        assert_eq!(page_size(Some(MAX_PAGE_SIZE)), Ok(MAX_PAGE_SIZE));
        assert_eq!(page_size(Some(0)), Err(RequestError::InvalidPageSize));
        assert_eq!(
            page_size(Some(MAX_PAGE_SIZE + 1)),
            Err(RequestError::InvalidPageSize)
        );
    }

    #[test]
    fn an_extra_row_becomes_the_next_cursor() {
        let page = CursorPage::from_rows(rows(3), 2);

        assert_eq!(page.items, vec![0, 1]);
        // The cursor resumes after the last row returned, not the extra one.
        assert_eq!(page.next_cursor, Some(encode(&[json!(1), json!("id")])));
    }

    #[test]
    fn a_full_page_with_nothing_after_it_is_the_last() {
        let page = CursorPage::from_rows(rows(2), 2);

        assert_eq!(page.items, vec![0, 1]);
        assert_eq!(page.next_cursor, None);
    }

    #[test]
    fn no_rows_make_an_empty_last_page() {
        let page = CursorPage::from_rows(rows(0), 2);

        assert!(page.items.is_empty());
        assert_eq!(page.next_cursor, None);
    }
}
