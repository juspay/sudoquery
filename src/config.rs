use std::collections::HashSet;
use std::path::PathBuf;

use serde::Deserialize;
use superposition_provider::data_source::file::FileDataSource;
use superposition_provider::{
    AllFeatureProvider, EvaluationContext, LocalResolutionProvider, RefreshStrategy,
};

use crate::collector_event::CollectorEvent;
use crate::enrichment::EnrichmentConfig;
use crate::kafka_connector::KafkaConnectorConfig;
use crate::result;

#[derive(Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server_config: ServerConfig,
    pub kafka_connector: KafkaConnectorConfig,
    #[serde(default)]
    allowed_events: Option<HashSet<String>>,
    pub enrichment: Option<EnrichmentConfig>,
}

#[derive(Deserialize)]
pub struct ServerConfig {
    pub addr: String,
    pub accept_cors: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            addr: "0.0.0.0:3000".to_string(),
            accept_cors: false,
        }
    }
}

impl Config {
    pub fn new(kafka_connector: KafkaConnectorConfig) -> Self {
        Config {
            server_config: ServerConfig::default(),
            kafka_connector,
            allowed_events: None,
            enrichment: None,
        }
    }

    pub fn is_event_allowed(&self, event: &CollectorEvent) -> bool {
        match &self.allowed_events {
            None => true,
            Some(allowed) => allowed.contains(&event.name),
        }
    }
}

pub struct ConfigBuilder {
    server_config: Option<ServerConfig>,
    kafka_connector: KafkaConnectorConfig,
    allowed_events: Option<HashSet<String>>,
    enrichment: Option<EnrichmentConfig>,
}

impl ConfigBuilder {
    pub fn server_config(mut self, server_config: ServerConfig) -> Self {
        self.server_config = Some(server_config);
        self
    }

    pub fn allowed_events(mut self, events: HashSet<String>) -> Self {
        self.allowed_events = Some(events);
        self
    }

    pub fn build(self) -> Config {
        Config {
            server_config: self.server_config.unwrap_or_default(),
            kafka_connector: self.kafka_connector,
            allowed_events: self.allowed_events,
            enrichment: self.enrichment,
        }
    }
}

pub async fn get_config_from_local_file(
    tenant_id: String,
    workspace_id: Option<String>,
) -> result::Result<Config> {
    let mut evaluation_context = EvaluationContext::default();
    evaluation_context.add_custom_field("tenant_id", tenant_id);
    if let Some(workspace_id) = workspace_id {
        evaluation_context.add_custom_field("workspace_id", workspace_id);
    }

    get_config_from_local_file_for_context(evaluation_context).await
}

pub async fn get_default_config_from_local_file() -> result::Result<Config> {
    get_config_from_local_file_for_context(EvaluationContext::default()).await
}

async fn get_config_from_local_file_for_context(
    evaluation_context: EvaluationContext,
) -> result::Result<Config> {
    let file_path = PathBuf::from("cac.toml");
    let data_source = FileDataSource::new(file_path).map_err(result::AppError::Config)?;

    let provider =
        LocalResolutionProvider::new(Box::new(data_source), None, RefreshStrategy::Manual);

    provider.init(evaluation_context.clone()).await?;
    let resolved_config = provider.resolve_all_features(evaluation_context).await?;

    serde_json::from_value(serde_json::Value::Object(resolved_config)).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn loads_config_from_cac_toml() {
        let config = get_config_from_local_file("tenant-1".to_string(), None)
            .await
            .unwrap();

        assert_eq!(config.kafka_connector.topic, "events");
        assert_eq!(
            config
                .kafka_connector
                .client_config
                .get("bootstrap.servers"),
            Some(&"host.docker.internal:19092".to_string())
        );
        assert!(config.allowed_events.is_none());
    }

    #[tokio::test]
    async fn loads_default_config_from_cac_toml() {
        let config = get_default_config_from_local_file().await.unwrap();

        assert_eq!(config.kafka_connector.topic, "events");
        assert_eq!(config.server_config.addr, "0.0.0.0:3000");
        assert!(config.server_config.accept_cors);
        assert!(config.allowed_events.is_none());
        assert!(
            config
                .enrichment
                .as_ref()
                .and_then(|enrichment| enrichment.ip_address.as_ref())
                .is_some()
        );
        assert!(
            config
                .enrichment
                .as_ref()
                .is_some_and(|enrichment| enrichment.arrived_at.enabled)
        );
    }
}
