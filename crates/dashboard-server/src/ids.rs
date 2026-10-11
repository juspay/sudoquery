//! Server-side generation of org and project slug identifiers.
//!
//! [`OrgId`] and [`ProjectId`] (in `canonical-event`) validate slugs but
//! cannot mint them, so this module is the single place that creates new
//! ones. A generated slug is `{base}-{suffix}`: `base` is the slugified
//! name (at most [`MAX_BASE_LEN`] chars), `suffix` is [`SUFFIX_LEN`]
//! random `[a-z0-9]` chars, keeping the total within the 30-char limit
//! enforced by `canonical_event::is_valid_slug`.

use canonical_event::{InvalidSlugError, OrgId, ProjectId};
use rand::{Rng, rng};

/// Longest base derived from a name; leaves room for `-` + suffix.
const MAX_BASE_LEN: usize = 23;
/// Number of random chars appended to the base.
const SUFFIX_LEN: usize = 6;
/// Alphabet of the random suffix.
const SUFFIX_CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
/// Base used when a name yields no usable characters.
const ORG_FALLBACK_BASE: &str = "org";
/// Base used when a name yields no usable characters.
const PROJECT_FALLBACK_BASE: &str = "proj";

/// Generates a new organization id from `name`.
pub fn generate_org_id(name: &str) -> OrgId {
    generate(
        name,
        ORG_FALLBACK_BASE,
        <OrgId as TryFrom<String>>::try_from,
    )
}

/// Generates a new project id from `name`.
pub fn generate_project_id(name: &str) -> ProjectId {
    generate(
        name,
        PROJECT_FALLBACK_BASE,
        <ProjectId as TryFrom<String>>::try_from,
    )
}

/// Mints `{base}-{suffix}` and converts it with `try_from`.
///
/// `slugify` clamps the base to the slug charset and length budget, so the
/// conversion always succeeds; if it ever did not, the next attempt uses
/// the always-valid `fallback_base` instead of panicking.
fn generate<T>(
    name: &str,
    fallback_base: &str,
    try_from: fn(String) -> Result<T, InvalidSlugError>,
) -> T {
    let mut base = slugify(name);
    if base.is_empty() {
        base = fallback_base.to_owned();
    }
    loop {
        let candidate = format!("{base}-{}", random_suffix());
        match try_from(candidate) {
            Ok(id) => return id,
            Err(_) => base = fallback_base.to_owned(),
        }
    }
}

/// ASCII-folds `name` with `deunicode`, lowercases the result (deunicode
/// romanizes e.g. 東京 to the mixed-case "Dong Jing"), collapses every run
/// of characters outside `[a-z0-9]` into a single `-`, drops leading digits
/// (slugs must start with a letter), and truncates to [`MAX_BASE_LEN`].
fn slugify(name: &str) -> String {
    let folded = deunicode::deunicode(name).to_lowercase();
    let mut slug = String::with_capacity(folded.len());
    let mut pending_dash = false;
    for ch in folded.chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            if pending_dash && !slug.is_empty() {
                slug.push('-');
            }
            pending_dash = false;
            slug.push(ch);
        } else {
            pending_dash = true;
        }
    }
    // A pending dash is only emitted after a kept character, so the slug
    // never starts with '-'; the digit-drop can still expose one, hence the
    // character set below.
    let mut base = slug
        .trim_start_matches(|c: char| c.is_ascii_digit() || c == '-')
        .to_owned();
    // `base` is ASCII-only, so byte truncation cannot split a character.
    base.truncate(MAX_BASE_LEN);
    base.trim_end_matches('-').to_owned()
}

/// Builds the random `[a-z0-9]` suffix of a generated slug.
fn random_suffix() -> String {
    let mut rng = rng();
    (0..SUFFIX_LEN)
        .map(|_| SUFFIX_CHARSET[rng.random_range(0..SUFFIX_CHARSET.len())] as char)
        .collect()
}

#[cfg(test)]
mod tests;
