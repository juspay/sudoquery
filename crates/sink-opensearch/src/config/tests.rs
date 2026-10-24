use serde_json::json;

use super::*;

fn minimal() -> Map<String, Value> {
    let value = json!({
        "kafka.topics": ["events.generic"],
        "kafka.group_id": "sink-opensearch",
        "kafka.client_config": { "bootstrap.servers": "cac-broker:9092" },
        "opensearch.url": "http://localhost:9200",
        "opensearch.index": "events",
        "dlq.topic": "events.generic.dlq",
    });
    value.as_object().cloned().unwrap()
}

fn config_with(changes: &[(&str, Value)]) -> Result<Config, ConfigError> {
    let mut values = minimal();
    for (key, value) in changes {
        values.insert((*key).to_owned(), value.clone());
    }
    Config::from_cac(values, EnvOverrides::default())
}

fn config_without(key: &str) -> Result<Config, ConfigError> {
    let mut values = minimal();
    values.remove(key);
    Config::from_cac(values, EnvOverrides::default())
}

fn env(vars: &[(&str, &str)]) -> Result<EnvOverrides, ConfigError> {
    let vars: HashMap<&str, &str> = vars.iter().copied().collect();
    EnvOverrides::from_lookup(|name| vars.get(name).map(|value| (*value).to_owned()))
}

fn config_from_env(vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
    Config::from_cac(minimal(), env(vars)?)
}

fn assert_invalid(result: Result<Config, ConfigError>, needle: &str) {
    match result {
        Err(ConfigError::Invalid(message)) => assert!(
            message.contains(needle),
            "expected `{needle}` in `{message}`"
        ),
        other => panic!("expected an invalid config error, got {other:?}"),
    }
}

fn assert_rejected(result: Result<Config, ConfigError>, needle: &str) {
    match result {
        Err(ConfigError::Parse(error)) => assert!(
            error.to_string().contains(needle),
            "expected `{needle}` in `{error}`"
        ),
        other => panic!("expected a parse error, got {other:?}"),
    }
}

#[test]
fn minimal_config_gets_defaults() {
    let config = config_with(&[]).unwrap();

    assert_eq!(config.batch.max_docs, 1000);
    assert_eq!(config.batch.max_bytes, 5 * 1024 * 1024);
    assert_eq!(config.batch.linger(), Duration::from_secs(1));
    assert_eq!(config.batch.max_in_flight, 8);
    assert_eq!(config.retry.initial_backoff(), Duration::from_millis(100));
    assert_eq!(config.retry.max_backoff(), Duration::from_secs(30));
    assert_eq!(config.commit.interval(), Duration::from_secs(5));
    assert_eq!(config.shutdown.grace(), Duration::from_secs(25));
    assert_eq!(config.opensearch.request_timeout(), Duration::from_secs(30));
    assert_eq!(config.server.addr, "0.0.0.0:9464".parse().unwrap());
    assert!(config.opensearch.username.is_none());
}

#[test]
fn dotted_keys_fill_their_sections() {
    let config = config_with(&[
        ("batch.max_docs", json!(10)),
        ("opensearch.request_timeout_ms", json!(2000)),
    ])
    .unwrap();

    assert_eq!(config.batch.max_docs, 10);
    assert_eq!(config.batch.linger_ms, 1000);
    assert_eq!(config.opensearch.request_timeout_ms, 2000);
}

#[test]
fn keys_need_a_section() {
    assert_invalid(
        config_with(&[("kafka_connector", json!({}))]),
        "keys look like `section.name`",
    );
}

#[test]
fn env_overrides_take_priority() {
    let config = config_from_env(&[
        ("KAFKA_BOOTSTRAP_SERVERS", "env-broker:9092"),
        (
            "KAFKA_CLIENT_CONFIG",
            r#"{"bootstrap.servers": "json-broker:9092", "security.protocol": "SSL"}"#,
        ),
        ("OPENSEARCH_URL", "https://search.internal:443"),
        ("OPENSEARCH_USERNAME", "sink"),
        ("OPENSEARCH_PASSWORD", "hunter2"),
    ])
    .unwrap();

    let client_config = &config.kafka.client_config;
    assert_eq!(client_config["bootstrap.servers"], "env-broker:9092");
    assert_eq!(client_config["security.protocol"], "SSL");
    assert_eq!(config.opensearch.url, "https://search.internal:443");
    assert_eq!(config.opensearch.username.as_deref(), Some("sink"));
    assert_eq!(
        config.opensearch.password.as_ref().map(Secret::expose),
        Some("hunter2")
    );
}

#[test]
fn every_setting_can_come_from_the_environment() {
    let config = config_from_env(&[
        ("KAFKA_TOPICS", "events.a, events.b"),
        ("KAFKA_GROUP_ID", "env-group"),
        ("KAFKA_CLIENT_CONFIG", r#"{"security.protocol": "SSL"}"#),
        ("OPENSEARCH_URL", "https://search.internal"),
        ("OPENSEARCH_INDEX", "events-env-{org_id}"),
        ("OPENSEARCH_REQUEST_TIMEOUT_MS", "2000"),
        ("BATCH_MAX_DOCS", "10"),
        ("BATCH_MAX_BYTES", "2048"),
        ("BATCH_LINGER_MS", "50"),
        ("BATCH_MAX_IN_FLIGHT", "2"),
        ("RETRY_INITIAL_BACKOFF_MS", "5"),
        ("RETRY_MAX_BACKOFF_MS", "500"),
        ("DLQ_TOPIC", "events.env.dlq"),
        ("COMMIT_INTERVAL_MS", "700"),
        ("SHUTDOWN_GRACE_MS", "900"),
        ("SERVER_ADDR", "127.0.0.1:9000"),
    ])
    .unwrap();

    assert_eq!(config.kafka.topics, vec!["events.a", "events.b"]);
    assert_eq!(config.kafka.group_id, "env-group");
    // Merged into the file's client config, not replacing it.
    assert_eq!(
        config.kafka.client_config["bootstrap.servers"],
        "cac-broker:9092"
    );
    assert_eq!(config.kafka.client_config["security.protocol"], "SSL");
    assert_eq!(config.opensearch.url, "https://search.internal");
    assert_eq!(
        config.opensearch.index,
        IndexTemplate::parse("events-env-{org_id}").unwrap()
    );
    assert!(config.opensearch.index_from_env);
    assert_eq!(config.opensearch.request_timeout_ms, 2000);
    assert_eq!(config.batch.max_docs, 10);
    assert_eq!(config.batch.max_bytes, 2048);
    assert_eq!(config.batch.linger_ms, 50);
    assert_eq!(config.batch.max_in_flight, 2);
    assert_eq!(config.retry.initial_backoff_ms, 5);
    assert_eq!(config.retry.max_backoff_ms, 500);
    assert_eq!(config.dlq.topic, "events.env.dlq");
    assert_eq!(config.commit.interval_ms, 700);
    assert_eq!(config.shutdown.grace_ms, 900);
    assert_eq!(config.server.addr, "127.0.0.1:9000".parse().unwrap());
}

#[test]
fn env_names_are_the_keys_in_upper_case() {
    for (key, name, _) in ENV_SETTINGS {
        assert_eq!(*name, key.to_uppercase().replace('.', "_"));
    }
}

#[test]
fn the_index_from_the_file_is_not_pinned() {
    let config = config_from_env(&[("BATCH_MAX_DOCS", "10")]).unwrap();

    assert!(!config.opensearch.index_from_env);
}

#[test]
fn empty_env_values_are_ignored() {
    let config = config_from_env(&[("BATCH_MAX_DOCS", "  "), ("DLQ_TOPIC", "")]).unwrap();

    assert_eq!(config.batch.max_docs, 1000);
    assert_eq!(config.dlq.topic, "events.generic.dlq");
}

#[test]
fn env_numbers_must_be_whole_numbers() {
    for value in ["ten", "-1", "1.5"] {
        let error = env(&[("BATCH_MAX_DOCS", value)]).unwrap_err();

        assert!(
            error.to_string().contains("BATCH_MAX_DOCS"),
            "expected the variable in `{error}`"
        );
    }
}

#[test]
fn env_values_are_validated_like_file_values() {
    assert_invalid(
        config_from_env(&[("DLQ_TOPIC", "events.generic")]),
        "dead letters",
    );
    assert_invalid(config_from_env(&[("KAFKA_TOPICS", "a,,b")]), "kafka.topics");
}

#[test]
fn debug_output_hides_the_password() {
    let config = config_from_env(&[
        ("OPENSEARCH_USERNAME", "sink"),
        ("OPENSEARCH_PASSWORD", "hunter2"),
    ])
    .unwrap();

    assert!(!format!("{config:?}").contains("hunter2"));
}

#[test]
fn unknown_keys_are_rejected() {
    assert_rejected(config_with(&[("batch.max_doc", json!(10))]), "max_doc");
}

#[test]
fn credentials_are_not_accepted_from_cac() {
    assert_rejected(
        config_with(&[("opensearch.password", json!("hunter2"))]),
        "password",
    );
}

#[test]
fn dlq_topic_must_not_be_consumed() {
    assert_invalid(
        config_with(&[("dlq.topic", json!("events.generic"))]),
        "dead letters",
    );
}

#[test]
fn dlq_topic_must_not_match_a_topic_pattern() {
    assert_invalid(
        config_with(&[
            ("kafka.topics", json!([r"^events\..*"])),
            ("dlq.topic", json!("events.dlq")),
        ]),
        "dead letters",
    );
    let config = config_with(&[
        ("kafka.topics", json!([r"^events\..*"])),
        ("dlq.topic", json!("dlq.events")),
    ]);
    assert!(config.is_ok(), "{config:?}");
}

#[test]
fn invalid_topic_pattern_is_rejected() {
    assert_invalid(
        config_with(&[("kafka.topics", json!(["^events.("]))]),
        "pattern",
    );
}

#[test]
fn bootstrap_servers_are_required() {
    assert_invalid(config_without("kafka.client_config"), "bootstrap.servers");
}

#[test]
fn index_names_follow_opensearch_rules() {
    for index in ["Events", "_events", "events,logs", "my events", ""] {
        assert_rejected(
            config_with(&[("opensearch.index", json!(index))]),
            "not a valid index name",
        );
    }
    for index in ["events", "events-v1", ".events", "logs.2026"] {
        assert!(
            config_with(&[("opensearch.index", json!(index))]).is_ok(),
            "`{index}` should be valid"
        );
    }
}

#[test]
fn index_can_be_per_workspace() {
    let config =
        config_with(&[("opensearch.index", json!("events-{org_id}-{workspace_id}"))]).unwrap();

    assert_eq!(
        config
            .opensearch
            .index
            .render("acme-k3x9qa", "web-shop-9x2k1a")
            .unwrap(),
        "events-acme-k3x9qa-web-shop-9x2k1a"
    );
}

#[test]
fn index_template_checks_its_fixed_parts() {
    for (index, needle) in [
        ("Events-{org_id}", "not a valid index name"),
        ("Events-{org_id}-{workspace_id}", "not a valid index name"),
        ("_{org_id}", "not a valid index name"),
        (
            "events-{org}",
            "the only placeholders are `{org_id}` and `{workspace_id}`",
        ),
        ("events-{date}-{org_id}", "the only placeholders"),
    ] {
        assert_rejected(config_with(&[("opensearch.index", json!(index))]), needle);
    }
}

#[test]
fn fixed_index_ignores_the_ids() {
    let template = IndexTemplate::parse("events").unwrap();

    assert_eq!(
        template
            .render("Anything, really", "Any workspace")
            .unwrap(),
        "events"
    );
}

#[test]
fn a_template_without_workspace_id_ignores_the_workspace() {
    let template = IndexTemplate::parse("events-{org_id}").unwrap();

    assert_eq!(
        template.render("merchant-1", "Not a valid name").unwrap(),
        "events-merchant-1"
    );
}

#[test]
fn orgs_that_cannot_form_an_index_name_are_refused() {
    let template = IndexTemplate::parse("events-{org_id}-{workspace_id}").unwrap();

    for org in [
        "",
        "Merchant-1",
        "a,b",
        "a b",
        "a*",
        "a/b",
        &"x".repeat(250),
    ] {
        assert!(
            matches!(template.render(org, "web-shop"), Err(InvalidId::Org(_))),
            "`{org}` should be refused"
        );
    }
    assert_eq!(
        template.render("m_1.eu", "web-shop").unwrap(),
        "events-m_1.eu-web-shop"
    );
}

#[test]
fn workspaces_that_cannot_form_an_index_name_are_refused() {
    let template = IndexTemplate::parse("events-{org_id}-{workspace_id}").unwrap();

    for workspace in ["", "Web", "a,b", "a b", "a*", "a/b", &"x".repeat(240)] {
        assert!(
            matches!(
                template.render("merchant-1", workspace),
                Err(InvalidId::Workspace(_))
            ),
            "`{workspace}` should be refused"
        );
    }
    // When both are bad, the org is reported.
    assert!(matches!(
        template.render("Merchant-1", "Web"),
        Err(InvalidId::Org(_))
    ));
}

#[test]
fn url_must_be_http() {
    assert_invalid(
        config_with(&[("opensearch.url", json!("ftp://localhost"))]),
        "http or https",
    );
}

#[test]
fn username_requires_password() {
    assert_invalid(
        config_from_env(&[("OPENSEARCH_USERNAME", "sink")]),
        "OPENSEARCH_PASSWORD",
    );
}

#[test]
fn backoff_bounds_are_checked() {
    assert_invalid(
        config_with(&[
            ("retry.initial_backoff_ms", json!(500)),
            ("retry.max_backoff_ms", json!(100)),
        ]),
        "retry.initial_backoff_ms",
    );
}

#[test]
fn zero_linger_is_rejected() {
    assert_invalid(config_with(&[("batch.linger_ms", json!(0))]), "linger_ms");
}

#[test]
fn invalid_client_config_json_names_the_variable() {
    let error = env(&[("KAFKA_CLIENT_CONFIG", "not-json")]).unwrap_err();

    assert!(error.to_string().contains("KAFKA_CLIENT_CONFIG"));
}

#[tokio::test]
async fn the_shipped_cac_file_is_valid() {
    let cac = Cac::load(concat!(env!("CARGO_MANIFEST_DIR"), "/cac.toml"))
        .await
        .unwrap();

    let values = cac.resolve_defaults().await.unwrap();
    for key in values.keys() {
        assert!(
            ENV_SETTINGS.iter().any(|(setting, _, _)| setting == key),
            "`{key}` has no environment variable in ENV_SETTINGS"
        );
    }
    let config = Config::from_cac(values, EnvOverrides::default()).unwrap();

    assert_eq!(config.kafka.topics, vec![r"^events\..*"]);
    assert_eq!(config.dlq.topic, "dlq.events");
    assert_eq!(
        config.opensearch.index,
        IndexTemplate::parse("events-{org_id}-{workspace_id}").unwrap()
    );
    cac.close().await;
}
