//! AWS KMS on the compose localstack, for the collector's authenticated
//! endpoint: creates keys, encrypts tokens with them, and schedules every
//! key it created for deletion afterwards.

use std::sync::Mutex;

use anyhow::Context;
use aws_sdk_kms::Client;
use aws_sdk_kms::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_kms::primitives::Blob;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

/// Endpoint used when `LOCALSTACK_URL` is unset: the compose localstack.
const DEFAULT_ENDPOINT: &str = "http://localhost:4566";
/// localstack accepts any credentials; the collector needs some to sign.
const ACCESS_KEY: &str = "test";
const SECRET_KEY: &str = "test";
const REGION: &str = "us-east-1";
/// KMS's shortest deletion window.
const DELETION_WINDOW_DAYS: i32 = 7;

pub(crate) struct Kms {
    client: Client,
    endpoint: String,
    created: Mutex<Vec<String>>,
}

impl Kms {
    /// Connects to `LOCALSTACK_URL` (default: the compose localstack) and
    /// checks its KMS answers.
    pub(crate) async fn connect() -> anyhow::Result<Self> {
        let endpoint = std::env::var("LOCALSTACK_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_ENDPOINT.to_owned());
        let config = aws_sdk_kms::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(REGION))
            .endpoint_url(&endpoint)
            .credentials_provider(Credentials::new(
                ACCESS_KEY,
                SECRET_KEY,
                None,
                None,
                "xtask-e2e",
            ))
            .build();
        let client = Client::from_conf(config);
        client.list_keys().send().await.with_context(|| {
            format!("KMS at {endpoint} is unreachable (hint: `cargo xtask setup localstack`)")
        })?;
        println!("e2e kms: {endpoint}");
        Ok(Self {
            client,
            endpoint,
            created: Mutex::new(Vec::new()),
        })
    }

    /// The environment that makes the collector decrypt its token with this
    /// KMS (`TOKEN_PROVIDER=aws`), minus the ciphertext itself.
    pub(crate) fn collector_env(&self) -> Vec<(&'static str, String)> {
        vec![
            ("TOKEN_PROVIDER", "aws".to_owned()),
            ("AWS_ACCESS_KEY_ID", ACCESS_KEY.to_owned()),
            ("AWS_SECRET_ACCESS_KEY", SECRET_KEY.to_owned()),
            ("AWS_REGION", REGION.to_owned()),
            ("AWS_ENDPOINT_URL", self.endpoint.clone()),
        ]
    }

    /// Creates a symmetric key, remembered for [`Self::delete_created`].
    pub(crate) async fn create_key(&self) -> anyhow::Result<String> {
        let key_id = self
            .client
            .create_key()
            .description("xtask e2e: collector bearer token")
            .send()
            .await
            .context("failed to create a KMS key")?
            .key_metadata()
            .map(|metadata| metadata.key_id().to_owned())
            .context("KMS returned no key metadata")?;
        self.created.lock().unwrap().push(key_id.clone());
        Ok(key_id)
    }

    /// `plaintext` encrypted with `key_id`, base64-encoded as the collector
    /// expects in `KMS_CIPHERTEXT`.
    pub(crate) async fn encrypt(&self, key_id: &str, plaintext: &str) -> anyhow::Result<String> {
        let ciphertext = self
            .client
            .encrypt()
            .key_id(key_id)
            .plaintext(Blob::new(plaintext.as_bytes()))
            .send()
            .await
            .context("failed to encrypt with KMS")?
            .ciphertext_blob
            .context("KMS returned no ciphertext")?;
        Ok(STANDARD.encode(ciphertext.as_ref()))
    }

    /// Schedules every key created by this run for deletion.
    pub(crate) async fn delete_created(&self) -> anyhow::Result<()> {
        let keys = std::mem::take(&mut *self.created.lock().unwrap());
        for key_id in keys {
            self.client
                .schedule_key_deletion()
                .key_id(&key_id)
                .pending_window_in_days(DELETION_WINDOW_DAYS)
                .send()
                .await
                .with_context(|| format!("failed to schedule deletion of KMS key {key_id}"))?;
        }
        Ok(())
    }
}
