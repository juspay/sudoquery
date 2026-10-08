//! Validated slug identifiers shared across the workspace.
//!
//! [`OrgId`] and [`ProjectId`] are newtypes over server-generated, URL-safe
//! slugs (see [`is_valid_slug`] for the exact rules). Generation lives in
//! `dashboard-server`; this module only validates and carries the values, so
//! the collector can reject malformed ids at the edge while sinks stay
//! lenient.

/// Rejection message carried by every [`InvalidSlugError`] built here.
const SLUG_RULES: &str = "slugs must be 6-30 characters long, start with an ASCII lowercase letter, end with an ASCII lowercase letter or digit, and contain only ASCII lowercase letters, digits, and hyphens";

/// Error returned when a string is not a valid slug (see [`is_valid_slug`]).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid slug {input:?}: {message}")]
pub struct InvalidSlugError {
    /// The rejected input.
    pub input: String,
    /// Why the input was rejected.
    pub message: &'static str,
}

/// Returns `true` when `s` is a valid slug:
///
/// * 6..=30 characters long (checked in bytes — valid slugs are pure ASCII),
/// * first character is an ASCII lowercase letter (`a`-`z`),
/// * last character is an ASCII lowercase letter or digit (`a`-`z`, `0`-`9`),
/// * every character is an ASCII lowercase letter, digit, or hyphen.
///
/// Equivalent to the regex `^[a-z][a-z0-9-]{4,28}[a-z0-9]$` (hand-rolled so
/// this crate does not depend on the `regex` crate).
pub fn is_valid_slug(s: &str) -> bool {
    let bytes = s.as_bytes();
    (6..=30).contains(&bytes.len())
        && bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes
            .last()
            .is_some_and(|&b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && bytes
            .iter()
            .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

macro_rules! slug_newtype {
    ($(#[$doc:meta])* $name:ident) => {
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
                if is_valid_slug(&value) {
                    Ok(Self(value))
                } else {
                    Err(InvalidSlugError {
                        input: value,
                        message: SLUG_RULES,
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
    /// Slug identifier for an organization (for example `acme-corp-42`).
    ///
    /// Generated by `dashboard-server`; accepted only when [`is_valid_slug`]
    /// passes. Serializes as a bare JSON string.
    OrgId
}

slug_newtype! {
    /// Slug identifier for a project (for example `blue-ocean-7`).
    ///
    /// Generated by `dashboard-server`; accepted only when [`is_valid_slug`]
    /// passes. Serializes as a bare JSON string.
    ProjectId
}

#[cfg(test)]
mod tests;
