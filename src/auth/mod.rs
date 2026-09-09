mod aws;
mod gcp;

use subtle::ConstantTimeEq;
use tokio::sync::OnceCell;

use crate::result;

const TOKEN_PROVIDER_ENV: &str = "TOKEN_PROVIDER";
const AUTHORIZATION_HEADER: &str = "authorization";
const BEARER_PREFIX: &str = "Bearer ";

static TOKEN: OnceCell<String> = OnceCell::const_new();

/// Validates that the `Authorization` header carries a Bearer token matching the
/// expected value, which is fetched by decrypting a cloud KMS ciphertext.
///
/// The cloud provider is selected via the `TOKEN_PROVIDER` environment variable
/// (`aws` or `gcp`, case-insensitive):
///
/// - `aws`: decrypts the base64 ciphertext from `KMS_CIPHERTEXT` via the AWS KMS
///   `Decrypt` API, with an encryption context parsed from `KMS_ENCRYPTION_CONTEXT`
///   (a JSON object of key-value pairs)
/// - `gcp`: decrypts the base64 ciphertext from `GCP_KMS_CIPHERTEXT` via the GCP
///   Cloud KMS `decrypt` API, using the key resource name from `GCP_KMS_KEY`
///   (`projects/P/locations/L/keyRings/R/cryptoKeys/K/cryptoKeyVersions/V`) and
///   optional additional authenticated data from `GCP_KMS_AAD` (plain string)
///
/// GCP credentials are resolved via Application Default Credentials
/// (`GOOGLE_APPLICATION_CREDENTIALS` or the metadata server). The token is
/// fetched once and cached for the lifetime of the process; failed fetches are
/// retried on the next request.
pub async fn validate_bearer_token(headers: &axum::http::HeaderMap) -> result::Result<()> {
    let provided = extract_bearer_token(headers).ok_or(result::AppError::MissingCredentials)?;

    let token = TOKEN.get_or_try_init(fetch_expected_token).await?;

    if constant_time_eq(provided.as_bytes(), token.as_bytes()) {
        Ok(())
    } else {
        Err(result::AppError::InvalidToken)
    }
}

/// Resolves the expected bearer token for health reporting on the authenticated
/// endpoint: `Ok(())` when the token source is configured and the ciphertext
/// decrypts, or the configuration/decryption error otherwise.
///
/// This warms the same cache used by `validate_bearer_token`, so a successful
/// check makes the next authenticated request skip the KMS call. No request
/// credentials are compared.
pub async fn health_check() -> result::Result<()> {
    TOKEN
        .get_or_try_init(fetch_expected_token)
        .await
        .map(|_| ())
}

/// Compares two byte slices in constant time using the `subtle` crate, so that
/// response timing does not reveal anything about the token or its length.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.ct_eq(b).into()
}

fn extract_bearer_token(headers: &axum::http::HeaderMap) -> Option<String> {
    let value = headers.get(AUTHORIZATION_HEADER)?.to_str().ok()?;
    let value = value.trim();
    let (scheme, token) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case(BEARER_PREFIX.trim()) {
        return None;
    }
    let token = token.trim();
    (!token.is_empty()).then(|| token.to_owned())
}

enum TokenProvider {
    Aws,
    Gcp,
}

async fn fetch_expected_token() -> result::Result<String> {
    match token_provider_from(non_empty_env(TOKEN_PROVIDER_ENV).as_deref())? {
        TokenProvider::Aws => aws::decrypt_token().await,
        TokenProvider::Gcp => gcp::decrypt_token().await,
    }
}

fn token_provider_from(raw: Option<&str>) -> result::Result<TokenProvider> {
    if raw.is_some_and(|value| value.eq_ignore_ascii_case("aws")) {
        Ok(TokenProvider::Aws)
    } else if raw.is_some_and(|value| value.eq_ignore_ascii_case("gcp")) {
        Ok(TokenProvider::Gcp)
    } else if let Some(value) = raw {
        Err(result::AppError::Config(format!(
            "`{}` must be `aws` or `gcp`, got `{}`",
            TOKEN_PROVIDER_ENV, value
        )))
    } else {
        Err(result::AppError::Config(format!(
            "`{}` environment variable must be set to `aws` or `gcp`",
            TOKEN_PROVIDER_ENV
        )))
    }
}

fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_bearer_token() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(AUTHORIZATION_HEADER, "Bearer abc123".parse().unwrap());
        assert_eq!(extract_bearer_token(&headers), Some("abc123".to_string()));
    }

    #[test]
    fn bearer_token_can_be_empty() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(AUTHORIZATION_HEADER, "Bearer   ".parse().unwrap());
        assert_eq!(extract_bearer_token(&headers), None);
    }

    #[test]
    fn bearer_scheme_is_case_insensitive() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(AUTHORIZATION_HEADER, "bEaReR abc123".parse().unwrap());
        assert_eq!(extract_bearer_token(&headers), Some("abc123".to_string()));
    }

    #[test]
    fn non_bearer_scheme_returns_none() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(AUTHORIZATION_HEADER, "Basic dXNlcjpwYXNz".parse().unwrap());
        assert_eq!(extract_bearer_token(&headers), None);
    }

    #[test]
    fn missing_authorization_header_returns_none() {
        assert_eq!(extract_bearer_token(&axum::http::HeaderMap::new()), None);
    }

    #[test]
    fn constant_time_eq_matches_equal_values() {
        assert!(constant_time_eq(b"token", b"token"));
        assert!(!constant_time_eq(b"token", b"tok3n"));
        assert!(!constant_time_eq(b"token", b"token-longer"));
    }

    #[test]
    fn selects_aws_provider() {
        assert!(matches!(
            token_provider_from(Some("aws")),
            Ok(TokenProvider::Aws)
        ));
    }

    #[test]
    fn selects_gcp_provider_case_insensitively() {
        assert!(matches!(
            token_provider_from(Some("GCP")),
            Ok(TokenProvider::Gcp)
        ));
    }

    #[test]
    fn unknown_provider_is_error() {
        assert!(matches!(
            token_provider_from(Some("azure")),
            Err(result::AppError::Config(_))
        ));
    }

    #[test]
    fn missing_provider_is_error() {
        assert!(matches!(
            token_provider_from(None),
            Err(result::AppError::Config(_))
        ));
    }
}
