//! Reads the sink's settings from CAC (Superposition), the same way the event
//! collector reads its `cac.toml`: a local file resolved against the
//! `org_id` and `proj_id` dimensions.
//!
//! The file is checked for changes every 30 seconds. Process-wide settings
//! are read once at startup; per-org values are resolved as events arrive,
//! so an org override takes effect without a restart.

use std::path::PathBuf;

use serde_json::{Map, Value};
use superposition_provider::data_source::file::FileDataSource;
use superposition_provider::{
    AllFeatureProvider, EvaluationContext, LocalResolutionProvider, PollingStrategy,
    RefreshStrategy, SuperpositionError,
};
use tracing::warn;

const REFRESH_INTERVAL_MS: u64 = 30_000;

const ORG_DIMENSION: &str = "org_id";
const PROJ_DIMENSION: &str = "proj_id";

#[derive(Debug, thiserror::Error)]
pub enum CacError {
    #[error("failed to load CAC file `{path}`: {message}")]
    Load { path: String, message: String },

    #[error("failed to resolve CAC config: {0}")]
    Resolve(#[from] SuperpositionError),
}

/// Handle to the sink's CAC file. Cheap to clone.
#[derive(Clone)]
pub struct Cac {
    provider: LocalResolutionProvider,
}

impl Cac {
    pub async fn load(path: impl Into<PathBuf>) -> Result<Self, CacError> {
        let path = path.into();
        let shown = path.display().to_string();
        let source = FileDataSource::new(path).map_err(|message| CacError::Load {
            path: shown.clone(),
            message,
        })?;

        let provider = LocalResolutionProvider::new(
            Box::new(source),
            None,
            RefreshStrategy::Polling(PollingStrategy::new(REFRESH_INTERVAL_MS)),
        );
        provider
            .init(EvaluationContext::default())
            .await
            .map_err(|error| CacError::Load {
                path: shown,
                message: error.to_string(),
            })?;

        Ok(Self { provider })
    }

    /// Every key, resolved without an org: the process-wide settings.
    pub async fn resolve_defaults(&self) -> Result<Map<String, Value>, CacError> {
        Ok(self
            .provider
            .resolve_all_features(EvaluationContext::default())
            .await?)
    }

    /// One key, resolved for an org, so `[[overrides]]` on `org_id` or
    /// `proj_id` apply.
    pub async fn resolve_for_org(
        &self,
        key: &str,
        org_id: &str,
        proj_id: Option<&str>,
    ) -> Result<Option<Value>, CacError> {
        let mut context = EvaluationContext::default();
        context.add_custom_field(ORG_DIMENSION, org_id.to_owned());
        if let Some(proj_id) = proj_id {
            context.add_custom_field(PROJ_DIMENSION, proj_id.to_owned());
        }

        let mut values = self
            .provider
            .resolve_all_features_with_filter(context, Some(vec![key.to_owned()]), None)
            .await?;
        Ok(values.remove(key))
    }

    /// Stops watching the file for changes.
    pub async fn close(&self) {
        if let Err(error) = self.provider.close_provider().await {
            warn!(%error, "failed to stop CAC refresh");
        }
    }
}
