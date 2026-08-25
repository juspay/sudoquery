use crate::canonical_event::CanonicalEvent;
use crate::collector_event::CollectorEvent;
use serde::Deserialize;

#[derive(Default, Deserialize)]
pub struct EnrichmentConfig {
    pub country: Option<CountryConfig>,
    pub arrived_at: Option<ArrivedAtConfig>,
}

impl EnrichmentConfig {
    pub fn builder() -> EnrichmentConfigBuilder {
        EnrichmentConfigBuilder::default()
    }
}

#[derive(Default)]
pub struct EnrichmentConfigBuilder {
    country: Option<CountryConfig>,
    arrived_at: Option<ArrivedAtConfig>,
}

impl EnrichmentConfigBuilder {
    pub fn country(mut self, config: CountryConfig) -> Self {
        self.country = Some(config);
        self
    }

    pub fn arrived_at(mut self, config: ArrivedAtConfig) -> Self {
        self.arrived_at = Some(config);
        self
    }

    pub fn build(self) -> EnrichmentConfig {
        EnrichmentConfig {
            country: self.country,
            arrived_at: self.arrived_at,
        }
    }
}

#[derive(Deserialize)]
pub enum CountryResolution {
    IpAddress,
}

#[derive(Default, Deserialize)]
pub struct CountryConfig {
    pub override_existing: bool,
    pub resolution: CountryResolution,
}

impl CountryConfig {
    pub fn builder() -> CountryConfigBuilder {
        CountryConfigBuilder::default()
    }
}

#[derive(Default)]
pub struct CountryConfigBuilder {
    override_existing: bool,
    resolution: Option<CountryResolution>,
}

impl CountryConfigBuilder {
    pub fn override_existing(mut self, override_existing: bool) -> Self {
        self.override_existing = override_existing;
        self
    }

    pub fn resolution(mut self, resolution: CountryResolution) -> Self {
        self.resolution = Some(resolution);
        self
    }

    pub fn build(self) -> CountryConfig {
        CountryConfig {
            override_existing: self.override_existing,
            resolution: self.resolution.unwrap_or_default(),
        }
    }
}

impl Default for CountryResolution {
    fn default() -> Self {
        CountryResolution::IpAddress
    }
}

#[derive(Default, Deserialize)]
pub struct ArrivedAtConfig {
    pub override_existing: bool,
}

impl ArrivedAtConfig {
    pub fn builder() -> ArrivedAtConfigBuilder {
        ArrivedAtConfigBuilder::default()
    }
}

#[derive(Default)]
pub struct ArrivedAtConfigBuilder {
    override_existing: bool,
}

impl ArrivedAtConfigBuilder {
    pub fn override_existing(mut self, override_existing: bool) -> Self {
        self.override_existing = override_existing;
        self
    }

    pub fn build(self) -> ArrivedAtConfig {
        ArrivedAtConfig {
            override_existing: self.override_existing,
        }
    }
}

pub fn enrich_event(
    event: &CollectorEvent,
    config: Option<&EnrichmentConfig>,
    ip_address: Option<&str>,
) -> CanonicalEvent {
    CanonicalEvent::from_collector_event_with_enrichment(event.clone(), config, ip_address)
}
