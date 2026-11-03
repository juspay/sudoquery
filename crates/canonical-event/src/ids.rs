//! Validated identifiers shared across the workspace.
//!
//! An [`OrgId`] is the org's slug, chosen when the org is created (see
//! [`is_valid_slug`]), e.g. `north_star`. A [`WorkspaceId`] is the org's slug
//! and the workspace's own slug joined by `-`, e.g. `north_star-web_shop`
//! (see [`is_valid_workspace_id`]): unique across orgs, so it alone can name
//! the workspace's topic, database and so on, and its org can be read back
//! from it. `dashboard-server` creates both; this module only validates and
//! carries them, so the collector can reject malformed ids at the edge while
//! sinks stay lenient.

/// Rejection message for an [`OrgId`] or a workspace slug.
const SLUG_RULES: &str = "slugs must be 3-30 characters long, start with an ASCII lowercase letter, end with an ASCII lowercase letter or digit, and contain only ASCII lowercase letters, digits, and underscores";

/// Rejection message for a [`WorkspaceId`].
const WORKSPACE_ID_RULES: &str = "workspace ids must be the org's slug and the workspace's slug joined by `-`, where slugs are 3-30 characters long, start with an ASCII lowercase letter, end with an ASCII lowercase letter or digit, and contain only ASCII lowercase letters, digits, and underscores";

/// Joins an org's slug and a workspace's slug in a [`WorkspaceId`]; never
/// part of a slug.
pub const WORKSPACE_ID_SEPARATOR: char = '-';

/// Error returned when a string is not a valid slug or workspace id.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid id {input:?}: {message}")]
pub struct InvalidSlugError {
    /// The rejected input.
    pub input: String,
    /// Why the input was rejected.
    pub message: &'static str,
}

/// Returns `true` when `s` is a valid slug, as an [`OrgId`] or a
/// workspace's own slug:
///
/// * 3..=30 characters long (checked in bytes — valid slugs are pure ASCII),
/// * first character is an ASCII lowercase letter (`a`-`z`),
/// * last character is an ASCII lowercase letter or digit (`a`-`z`, `0`-`9`),
/// * every character is an ASCII lowercase letter, digit, or underscore.
///
/// Equivalent to the regex `^[a-z][a-z0-9_]{1,28}[a-z0-9]$` (hand-rolled so
/// this crate does not depend on the `regex` crate).
pub fn is_valid_slug(s: &str) -> bool {
    let bytes = s.as_bytes();
    (3..=30).contains(&bytes.len())
        && bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes
            .last()
            .is_some_and(|&b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && bytes
            .iter()
            .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Returns `true` when `s` is a valid workspace id: two valid slugs (the
/// org's, then the workspace's) joined by [`WORKSPACE_ID_SEPARATOR`].
pub fn is_valid_workspace_id(s: &str) -> bool {
    split_workspace_id(s).is_some()
}

/// The org slug and workspace slug of a valid workspace id.
fn split_workspace_id(s: &str) -> Option<(&str, &str)> {
    let (org, slug) = s.split_once(WORKSPACE_ID_SEPARATOR)?;
    (is_valid_slug(org) && is_valid_slug(slug)).then_some((org, slug))
}

macro_rules! slug_newtype {
    ($(#[$doc:meta])* $name:ident, $is_valid:path, $rules:expr) => {
        $(#[$doc])*
        #[derive(
            Debug,
            Clone,
            PartialEq,
            Eq,
            Hash,
            serde::Serialize,
            serde::Deserialize,
        )]
        #[serde(try_from = "String")]
        #[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
        #[cfg_attr(feature = "sqlx", sqlx(transparent))]
        pub struct $name(String);

        impl std::str::FromStr for $name {
            type Err = InvalidSlugError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                <Self as std::convert::TryFrom<String>>::try_from(s.to_owned())
            }
        }

        impl std::convert::TryFrom<String> for $name {
            type Error = InvalidSlugError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                if $is_valid(&value) {
                    Ok(Self(value))
                } else {
                    Err(InvalidSlugError {
                        input: value,
                        message: $rules,
                    })
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl std::convert::AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl std::convert::From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        #[cfg(feature = "sea-orm")]
        impl std::convert::From<$name> for sea_orm::Value {
            fn from(value: $name) -> Self {
                value.0.into()
            }
        }

        #[cfg(feature = "sea-orm")]
        impl sea_orm::TryGetable for $name {
            fn try_get_by<I: sea_orm::ColIdx>(
                res: &sea_orm::QueryResult,
                idx: I,
            ) -> Result<Self, sea_orm::TryGetError> {
                let raw = <String as sea_orm::TryGetable>::try_get_by(res, idx)?;
                <Self as std::convert::TryFrom<String>>::try_from(raw).map_err(|err| {
                    sea_orm::TryGetError::DbErr(sea_orm::DbErr::Custom(format!(
                        "invalid {} at {idx:?}: {err}",
                        stringify!($name),
                    )))
                })
            }
        }

        #[cfg(feature = "sea-orm")]
        impl sea_orm::sea_query::ValueType for $name {
            fn try_from(v: sea_orm::Value) -> Result<Self, sea_orm::sea_query::ValueTypeErr> {
                let raw = <String as sea_orm::sea_query::ValueType>::try_from(v)?;
                <Self as std::convert::TryFrom<String>>::try_from(raw)
                    .map_err(|_| sea_orm::sea_query::ValueTypeErr)
            }

            fn type_name() -> String {
                stringify!($name).to_owned()
            }

            fn array_type() -> sea_orm::sea_query::ArrayType {
                sea_orm::sea_query::ArrayType::String
            }

            fn column_type() -> sea_orm::sea_query::ColumnType {
                sea_orm::sea_query::ColumnType::String(sea_orm::sea_query::StringLen::None)
            }
        }

        #[cfg(feature = "sea-orm")]
        impl sea_orm::sea_query::Nullable for $name {
            fn null() -> sea_orm::Value {
                <String as sea_orm::sea_query::Nullable>::null()
            }
        }
    };
}

slug_newtype! {
    /// An organization's id: its slug (for example `north_star`).
    ///
    /// Chosen when the org is created; accepted only when [`is_valid_slug`]
    /// passes. Serializes as a bare JSON string.
    OrgId, is_valid_slug, SLUG_RULES
}

slug_newtype! {
    /// A workspace's id: its org's slug and its own joined by `-` (for
    /// example `north_star-web_shop`).
    ///
    /// Accepted only when [`is_valid_workspace_id`] passes. Serializes as a
    /// bare JSON string.
    WorkspaceId, is_valid_workspace_id, WORKSPACE_ID_RULES
}

impl WorkspaceId {
    /// The id of `org_id`'s workspace with slug `slug`.
    pub fn new(org_id: &OrgId, slug: &str) -> Result<Self, InvalidSlugError> {
        if !is_valid_slug(slug) {
            return Err(InvalidSlugError {
                input: slug.to_owned(),
                message: SLUG_RULES,
            });
        }
        Ok(Self(format!("{org_id}{WORKSPACE_ID_SEPARATOR}{slug}")))
    }

    /// The slug of the workspace's org: its [`OrgId`].
    pub fn org_slug(&self) -> &str {
        self.parts().0
    }

    /// The workspace's own slug, unique within its org.
    pub fn slug(&self) -> &str {
        self.parts().1
    }

    fn parts(&self) -> (&str, &str) {
        split_workspace_id(&self.0).expect("a WorkspaceId is valid")
    }
}

#[cfg(test)]
mod tests;
