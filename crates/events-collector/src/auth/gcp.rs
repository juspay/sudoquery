use base64::Engine as _;
use google_cloud_googleapis::cloud::kms::v1::DecryptRequest;
use google_cloud_kms::client::{Client, ClientConfig};

use super::non_empty_env;
use crate::result;

const GCP_KMS_KEY_ENV: &str = "GCP_KMS_KEY";
const GCP_KMS_CIPHERTEXT_ENV: &str = "GCP_KMS_CIPHERTEXT";
const GCP_KMS_AAD_ENV: &str = "GCP_KMS_AAD";

pub(crate) async fn decrypt_token() -> result::Result<String> {
    let request = decrypt_request()?;

    let config = ClientConfig::default().with_auth().await.map_err(|error| {
        result::AppError::Config(format!(
            "failed to resolve GCP credentials for KMS: {}",
            error
        ))
    })?;
    let client = Client::new(config).await.map_err(|error| {
        result::AppError::Config(format!("failed to create GCP KMS client: {}", error))
    })?;

    let response = client.decrypt(request, None).await.map_err(|error| {
        result::AppError::Config(format!(
            "failed to decrypt auth token via GCP KMS: {}",
            error
        ))
    })?;

    Ok(String::from_utf8_lossy(&response.plaintext)
        .trim()
        .to_string())
}

fn decrypt_request() -> result::Result<DecryptRequest> {
    Ok(DecryptRequest {
        name: key_name_from(non_empty_env(GCP_KMS_KEY_ENV).as_deref())?,
        ciphertext: ciphertext_from(non_empty_env(GCP_KMS_CIPHERTEXT_ENV).as_deref())?,
        additional_authenticated_data: aad_from(non_empty_env(GCP_KMS_AAD_ENV).as_deref()),
        ..Default::default()
    })
}

fn key_name_from(raw: Option<&str>) -> result::Result<String> {
    raw.map(ToOwned::to_owned).ok_or_else(|| {
        result::AppError::Config(format!(
            "`{}` environment variable must be set to the full CryptoKeyVersion resource name \
             (projects/PROJECT/locations/LOCATION/keyRings/RING/cryptoKeys/KEY/cryptoKeyVersions/VERSION)",
            GCP_KMS_KEY_ENV
        ))
    })
}

fn ciphertext_from(raw: Option<&str>) -> result::Result<Vec<u8>> {
    let raw = raw.ok_or_else(|| {
        result::AppError::Config(format!(
            "`{}` environment variable must be set to the base64 KMS ciphertext",
            GCP_KMS_CIPHERTEXT_ENV
        ))
    })?;
    ciphertext(raw)
}

fn ciphertext(raw: &str) -> result::Result<Vec<u8>> {
    base64::engine::general_purpose::STANDARD
        .decode(raw)
        .map_err(|error| {
            result::AppError::Config(format!(
                "`{}` is not valid base64: {}",
                GCP_KMS_CIPHERTEXT_ENV, error
            ))
        })
}

fn aad_from(raw: Option<&str>) -> Vec<u8> {
    raw.map(str::as_bytes).unwrap_or_default().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_NAME: &str = "projects/p/locations/l/keyRings/r/cryptoKeys/k/cryptoKeyVersions/1";

    #[test]
    fn builds_decrypt_request_from_parsed_values() {
        let request = DecryptRequest {
            name: key_name_from(Some(KEY_NAME)).unwrap(),
            ciphertext: ciphertext_from(Some("aGVsbG8=")).unwrap(),
            additional_authenticated_data: aad_from(Some("my-app")),
            ..Default::default()
        };

        assert_eq!(request.name, KEY_NAME);
        assert_eq!(request.ciphertext, b"hello".to_vec());
        assert_eq!(request.additional_authenticated_data, b"my-app".to_vec());
    }

    #[test]
    fn missing_key_name_is_error() {
        assert!(matches!(
            key_name_from(None),
            Err(result::AppError::Config(_))
        ));
    }

    #[test]
    fn missing_ciphertext_is_error() {
        assert!(matches!(
            ciphertext_from(None),
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

    #[test]
    fn missing_aad_defaults_to_empty() {
        assert!(aad_from(None).is_empty());
    }
}
