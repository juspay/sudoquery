//! Finds each org's index in CAC.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use tokio::time::Instant;

use crate::cac::Cac;
use crate::config::{INDEX_KEY, IndexTemplate};

/// How long an org's resolved index is reused before CAC is asked again.
/// A CAC change reaches the sink within the file's 30-second refresh plus this.
const CACHE_TTL: Duration = Duration::from_secs(30);

#[derive(Clone, PartialEq, Eq, Hash)]
struct Org {
    org_id: String,
    workspace_id: Option<String>,
}

struct Cached {
    index: Result<IndexTemplate, String>,
    resolved_at: Instant,
}

pub struct OrgIndexes {
    cac: Cac,
    /// The index from `OPENSEARCH_INDEX`, used for every org instead of CAC.
    pinned: Option<IndexTemplate>,
    cache: Mutex<HashMap<Org, Cached>>,
}

impl OrgIndexes {
    pub fn new(cac: Cac, pinned: Option<IndexTemplate>) -> Self {
        Self {
            cac,
            pinned,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// The org's `opensearch.index`, with CAC overrides for its
    /// `org_id` and `workspace_id` applied, unless the index is pinned. Fails
    /// when the configured value isn't a valid index template.
    pub async fn index_for(
        &self,
        org_id: &str,
        workspace_id: Option<&str>,
    ) -> Result<IndexTemplate, String> {
        if let Some(pinned) = &self.pinned {
            return Ok(pinned.clone());
        }
        let org = Org {
            org_id: org_id.to_owned(),
            workspace_id: workspace_id.map(str::to_owned),
        };
        if let Some(cached) = self.lock().get(&org)
            && cached.resolved_at.elapsed() < CACHE_TTL
        {
            return cached.index.clone();
        }

        let index = self.resolve(org_id, workspace_id).await;
        self.lock().insert(
            org,
            Cached {
                index: index.clone(),
                resolved_at: Instant::now(),
            },
        );
        index
    }

    async fn resolve(
        &self,
        org_id: &str,
        workspace_id: Option<&str>,
    ) -> Result<IndexTemplate, String> {
        let value = self
            .cac
            .resolve_for_org(INDEX_KEY, org_id, workspace_id)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("CAC has no `{INDEX_KEY}`"))?;
        let template = value
            .as_str()
            .ok_or_else(|| format!("`{INDEX_KEY}` must be a string, got {value}"))?;
        IndexTemplate::parse(template)
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<Org, Cached>> {
        self.cache.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests;
