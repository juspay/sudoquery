use std::collections::HashMap;

use aws_sdk_kms::Client;
use aws_sdk_kms::primitives::Blob;
use base64::Engine as _;

use super::non_empty_env;
use crate::result;

const KMS_CIPHERTEXT_ENV: &str = "KMS_CIPHERTEXT";
const KMS_ENCRYPTION_CONTEXT_ENV: &str = "KMS_ENCRYPTION_CONTEXT";

pub(crate) async fn decrypt_token() -> result::Result<String> {
    let config = aws_config::load_from_env().await;
    let client = Client::new(&config);

    let ciphertext = ciphertext_from_env()?;
    let encryption_context = encryption_context_from_env()?;

    let response = client
        .decrypt()
        .ciphertext_blob(Blob::new(ciphertext))
        .set_encryption_context(encryption_context)
        .send()
        .await
        .map_err(|error| {
            result::AppError::Config(format!(
                "failed to decrypt auth token via AWS KMS: {}",
                error
            ))
        })?;

    response
        .plaintext()
        .map(|plaintext| {
            String::from_utf8_lossy(plaintext.as_ref())
                .trim()
                .to_string()
        })
        .ok_or_else(|| {
            result::AppError::Config("AWS KMS decrypt response contained no plaintext".to_string())
        })
}

fn ciphertext_from_env() -> result::Result<Vec<u8>> {
    let raw = non_empty_env(KMS_CIPHERTEXT_ENV).ok_or_else(|| {
        result::AppError::Config(format!(
            "`{}` environment variable must be set to the base64 KMS ciphertext",
            KMS_CIPHERTEXT_ENV
        ))
    })?;
    ciphertext(&raw)
}

fn ciphertext(raw: &str) -> result::Result<Vec<u8>> {
    base64::engine::general_purpose::STANDARD
        .decode(raw)
        .map_err(|error| {
            result::AppError::Config(format!(
                "`{}` is not valid base64: {}",
                KMS_CIPHERTEXT_ENV, error
            ))
        })
}

fn encryption_context_from_env() -> result::Result<Option<HashMap<String, String>>> {
    let Some(raw) = non_empty_env(KMS_ENCRYPTION_CONTEXT_ENV) else {
        return Ok(None);
    };
    encryption_context(&raw)
}

fn encryption_context(raw: &str) -> result::Result<Option<HashMap<String, String>>> {
    let parsed: HashMap<String, String> = serde_json::from_str(raw).map_err(|error| {
        result::AppError::Config(format!(
            "invalid JSON in `{}` environment variable: {}",
            KMS_ENCRYPTION_CONTEXT_ENV, error
        ))
    })?;

    Ok(Some(parsed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_encryption_context_from_env_json() {
        let parsed = encryption_context(r#"{"app":"my-app","var":"API_KEY"}"#)
            .unwrap()
            .unwrap();
        assert_eq!(parsed.get("app"), Some(&"my-app".to_string()));
        assert_eq!(parsed.get("var"), Some(&"API_KEY".to_string()));
    }

    #[test]
    fn parses_empty_encryption_context_json() {
        let result = encryption_context(r#"{}"#).unwrap();
        assert!(result.is_some_and(|map| map.is_empty()));
    }

    #[test]
    fn invalid_encryption_context_json_is_error() {
        assert!(matches!(
            encryption_context("not-json"),
            Err(result::AppError::Config(_))
        ));
    }

    #[test]
    fn decodes_base64_ciphertext() {
        assert_eq!(ciphertext("aGVsbG8=").unwrap(), b"hello".to_vec());
    }

    #[test]
    fn invalid_base64_ciphertext_is_error() {
        assert!(matches!(
            ciphertext("!!not-base64!!"),
            Err(result::AppError::Config(_))
        ));
    }
}
