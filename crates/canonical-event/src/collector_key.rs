//! Collector API keys, sent as `Authorization: Bearer sk_<kid>_<secret>` to
//! the collector's authenticated endpoint.
//!
//! * `kid`: [`KID_BYTES`] random bytes as lowercase hex. Public; it is the
//!   primary key of the key's `collector_keys` row.
//! * `secret`: [`SECRET_BYTES`] random bytes as lowercase hex. Only
//!   `sha256(secret)` (of the raw bytes) is stored; the full key is shown to
//!   its creator once.
//!
//! `dashboard-server` generates keys; the collector parses and checks them.

use sha2::{Digest, Sha256};

pub const KID_BYTES: usize = 8;
pub const SECRET_BYTES: usize = 32;
const PREFIX: &str = "sk_";
const SEPARATOR: char = '_';

/// A parsed or freshly generated collector API key.
///
/// [`Display`](std::fmt::Display) renders the full key, secret included;
/// [`Debug`](std::fmt::Debug) redacts the secret.
#[derive(Clone, PartialEq, Eq)]
pub struct CollectorKey {
    kid: String,
    secret: [u8; SECRET_BYTES],
}

impl CollectorKey {
    /// A key from random bytes; the caller supplies them from a CSPRNG.
    pub fn new(kid: [u8; KID_BYTES], secret: [u8; SECRET_BYTES]) -> Self {
        Self {
            kid: hex::encode(kid),
            secret,
        }
    }

    /// Parses `sk_<kid>_<secret>`; `None` unless both parts are lowercase hex
    /// of exactly the right length.
    pub fn parse(key: &str) -> Option<Self> {
        let (kid, secret) = key.strip_prefix(PREFIX)?.split_once(SEPARATOR)?;
        if kid.len() != KID_BYTES * 2 || !is_lower_hex(kid) || !is_lower_hex(secret) {
            return None;
        }
        let mut bytes = [0; SECRET_BYTES];
        hex::decode_to_slice(secret, &mut bytes).ok()?;
        Some(Self {
            kid: kid.to_owned(),
            secret: bytes,
        })
    }

    pub fn kid(&self) -> &str {
        &self.kid
    }

    /// `sha256(secret)`: what `collector_keys.hashed_secret` stores.
    pub fn hashed_secret(&self) -> [u8; 32] {
        Sha256::digest(self.secret).into()
    }
}

impl std::fmt::Display for CollectorKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{PREFIX}{}{SEPARATOR}{}",
            self.kid,
            hex::encode(self.secret)
        )
    }
}

impl std::fmt::Debug for CollectorKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollectorKey")
            .field("kid", &self.kid)
            .field("secret", &"<redacted>")
            .finish()
    }
}

fn is_lower_hex(s: &str) -> bool {
    s.bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> CollectorKey {
        CollectorKey::new([0xab; KID_BYTES], [0x1c; SECRET_BYTES])
    }

    #[test]
    fn formats_as_prefix_kid_and_secret() {
        assert_eq!(
            key().to_string(),
            format!(
                "sk_{}_{}",
                "ab".repeat(KID_BYTES),
                "1c".repeat(SECRET_BYTES)
            )
        );
    }

    #[test]
    fn parses_what_it_formats() {
        let parsed = CollectorKey::parse(&key().to_string()).unwrap();
        assert_eq!(parsed, key());
        assert_eq!(parsed.kid(), "ab".repeat(KID_BYTES));
    }

    #[test]
    fn hashes_the_raw_secret_bytes() {
        assert_eq!(
            key().hashed_secret().as_slice(),
            Sha256::digest([0x1c; SECRET_BYTES]).as_slice()
        );
    }

    #[test]
    fn rejects_malformed_keys() {
        let kid = "ab".repeat(KID_BYTES);
        let secret = "1c".repeat(SECRET_BYTES);
        for invalid in [
            String::new(),
            format!("{kid}_{secret}"),
            format!("pk_{kid}_{secret}"),
            format!("sk_{kid}{secret}"),
            format!("sk_{}_{secret}", &kid[1..]),
            format!("sk_{kid}a_{secret}"),
            format!("sk_{kid}_{}", &secret[1..]),
            format!("sk_{kid}_{secret}00"),
            format!("sk_{}_{secret}", kid.to_uppercase()),
            format!("sk_{kid}_{}", secret.to_uppercase()),
            format!("sk_{kid}_{}g", &secret[1..]),
            format!("sk_{kid}_{secret}_"),
            format!(" sk_{kid}_{secret}"),
        ] {
            assert!(
                CollectorKey::parse(&invalid).is_none(),
                "{invalid:?} should be rejected"
            );
        }
    }

    #[test]
    fn debug_redacts_the_secret() {
        let debug = format!("{:?}", key());
        assert!(debug.contains(&"ab".repeat(KID_BYTES)), "{debug}");
        assert!(!debug.contains(&"1c".repeat(SECRET_BYTES)), "{debug}");
    }
}
