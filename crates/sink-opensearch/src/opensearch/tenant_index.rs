//! Finds each tenant's index in CAC.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use tokio::time::Instant;

use crate::cac::Cac;
use crate::config::IndexTemplate;

/// The CAC key resolved per tenant.
const INDEX_KEY: &str = "opensearch.index";

/// How long a tenant's resolved index is reused before CAC is asked again.
/// A CAC change reaches the sink within the file's 30-second refresh plus this.
const CACHE_TTL: Duration = Duration::from_secs(30);

#[derive(Clone, PartialEq, Eq, Hash)]
struct Tenant {
    tenant_id: String,
    workspace_id: Option<String>,
}

struct Cached {
    index: Result<IndexTemplate, String>,
    resolved_at: Instant,
}

pub struct TenantIndexes {
    cac: Cac,
    cache: Mutex<HashMap<Tenant, Cached>>,
}

impl TenantIndexes {
    pub fn new(cac: Cac) -> Self {
        Self {
            cac,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// The tenant's `opensearch.index`, with CAC overrides for its
    /// `tenant_id` and `workspace_id` applied. Fails when the configured value
    /// isn't a valid index template.
    pub async fn index_for(
        &self,
        tenant_id: &str,
        workspace_id: Option<&str>,
    ) -> Result<IndexTemplate, String> {
        let tenant = Tenant {
            tenant_id: tenant_id.to_owned(),
            workspace_id: workspace_id.map(str::to_owned),
        };
        if let Some(cached) = self.lock().get(&tenant)
            && cached.resolved_at.elapsed() < CACHE_TTL
        {
            return cached.index.clone();
        }

        let index = self.resolve(tenant_id, workspace_id).await;
        self.lock().insert(
            tenant,
            Cached {
                index: index.clone(),
                resolved_at: Instant::now(),
            },
        );
        index
    }

    async fn resolve(
        &self,
        tenant_id: &str,
        workspace_id: Option<&str>,
    ) -> Result<IndexTemplate, String> {
        let value = self
            .cac
            .resolve_for_tenant(INDEX_KEY, tenant_id, workspace_id)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("CAC has no `{INDEX_KEY}`"))?;
        let template = value
            .as_str()
            .ok_or_else(|| format!("`{INDEX_KEY}` must be a string, got {value}"))?;
        IndexTemplate::parse(template)
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<Tenant, Cached>> {
        self.cache.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// A CAC file where merchant-1 has a dedicated index and workspace eu of
    /// merchant-2 has another.
    const CAC: &str = r#"
[default-configs]
"opensearch.index" = { value = "events-{tenant_id}", schema = { type = "string" } }

[dimensions]
tenant_id = { position = 1, schema = { type = "string" } }
workspace_id = { position = 2, schema = { type = "string" } }

[[overrides]]
_context_ = { tenant_id = "merchant-1" }
"opensearch.index" = "events-merchant-1-dedicated"

[[overrides]]
_context_ = { tenant_id = "merchant-2", workspace_id = "eu" }
"opensearch.index" = "events-merchant-2-eu"

[[overrides]]
_context_ = { tenant_id = "broken" }
"opensearch.index" = "Not A Valid Index"
"#;

    async fn indexes() -> (TenantIndexes, PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "sink-opensearch-cac-{}.toml",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::write(&path, CAC).unwrap();
        (TenantIndexes::new(Cac::load(&path).await.unwrap()), path)
    }

    async fn index_name(
        indexes: &TenantIndexes,
        tenant_id: &str,
        workspace_id: Option<&str>,
    ) -> String {
        let template = indexes.index_for(tenant_id, workspace_id).await.unwrap();
        template.render(tenant_id).unwrap().into_owned()
    }

    #[tokio::test]
    async fn tenants_without_an_override_use_the_default() {
        let (indexes, path) = indexes().await;

        assert_eq!(
            index_name(&indexes, "merchant-9", None).await,
            "events-merchant-9"
        );
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn tenant_overrides_apply() {
        let (indexes, path) = indexes().await;

        assert_eq!(
            index_name(&indexes, "merchant-1", None).await,
            "events-merchant-1-dedicated"
        );
        assert_eq!(
            index_name(&indexes, "merchant-1", Some("eu")).await,
            "events-merchant-1-dedicated"
        );
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn workspace_overrides_apply() {
        let (indexes, path) = indexes().await;

        assert_eq!(
            index_name(&indexes, "merchant-2", Some("eu")).await,
            "events-merchant-2-eu"
        );
        assert_eq!(
            index_name(&indexes, "merchant-2", Some("us")).await,
            "events-merchant-2"
        );
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn an_invalid_override_is_an_error_for_that_tenant_only() {
        let (indexes, path) = indexes().await;

        let error = indexes.index_for("broken", None).await.unwrap_err();

        assert!(error.contains("not a valid index name"), "{error}");
        assert_eq!(
            index_name(&indexes, "merchant-9", None).await,
            "events-merchant-9"
        );
        std::fs::remove_file(path).unwrap();
    }
}
