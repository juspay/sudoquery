use crate::canonical_event::CanonicalEvent;
use crate::collector_event::CollectorEvent;
use serde::Deserialize;

#[derive(Default, Deserialize)]
pub struct EnrichmentConfig {
    pub country: Option<CountryConfig>,
    #[serde(default)]
    pub arrived_at: ArrivedAtConfig,
    pub ip_address: Option<IpAddressConfig>,
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
    ip_address: Option<IpAddressConfig>,
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

    pub fn ip_address(mut self, config: IpAddressConfig) -> Self {
        self.ip_address = Some(config);
        self
    }

    pub fn build(self) -> EnrichmentConfig {
        EnrichmentConfig {
            country: self.country,
            arrived_at: self.arrived_at.unwrap_or_default(),
            ip_address: self.ip_address,
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

#[derive(Deserialize)]
pub struct ArrivedAtConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub override_existing: bool,
}

impl Default for ArrivedAtConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            override_existing: false,
        }
    }
}

impl ArrivedAtConfig {
    pub fn builder() -> ArrivedAtConfigBuilder {
        ArrivedAtConfigBuilder::default()
    }
}

#[derive(Default)]
pub struct ArrivedAtConfigBuilder {
    enabled: Option<bool>,
    override_existing: bool,
}

impl ArrivedAtConfigBuilder {
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = Some(enabled);
        self
    }

    pub fn override_existing(mut self, override_existing: bool) -> Self {
        self.override_existing = override_existing;
        self
    }

    pub fn build(self) -> ArrivedAtConfig {
        ArrivedAtConfig {
            enabled: self.enabled.unwrap_or(true),
            override_existing: self.override_existing,
        }
    }
}

#[derive(Default, Deserialize)]
pub struct IpAddressConfig {
    pub override_existing: bool,
}

impl IpAddressConfig {
    pub fn builder() -> IpAddressConfigBuilder {
        IpAddressConfigBuilder::default()
    }
}

#[derive(Default)]
pub struct IpAddressConfigBuilder {
    override_existing: bool,
}

impl IpAddressConfigBuilder {
    pub fn override_existing(mut self, override_existing: bool) -> Self {
        self.override_existing = override_existing;
        self
    }

    pub fn build(self) -> IpAddressConfig {
        IpAddressConfig {
            override_existing: self.override_existing,
        }
    }
}

fn default_true() -> bool {
    true
}

pub fn enrich_event(
    event: &CollectorEvent,
    config: Option<&EnrichmentConfig>,
    ip_address: Option<&str>,
) -> CanonicalEvent {
    crate::canonical_event::from_collector_event_with_enrichment(
        event.clone(),
        false,
        config,
        ip_address,
    )
}
