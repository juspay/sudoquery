//! A workspace's ClickHouse objects: its database (`ckh/workspace.sql`,
//! fed from the workspace's Kafka topic) and its read-only user. Every
//! statement is idempotent in both directions, so a partial provisioning or
//! deprovisioning can simply be run again.

use crate::clickhouse::{ClickHouse, ClickHouseError};

/// Per-workspace ClickHouse objects; see the file header for placeholders.
const WORKSPACE_SQL: &str = include_str!("../../../ckh/workspace.sql"); 

/// The tables in a workspace's database that its ClickHouse user may read.
/// Only the tables: never its Kafka table, which a SELECT would consume from.
pub const WORKSPACE_TABLES: [&str; 3] = ["events", "user_events", "event_schema_catalog"];

/// Names ClickHouse already uses for databases or users; a workspace named
/// after one would read or replace it.
const RESERVED: [&str; 3] = ["default", "system", "information_schema"];

/// Kafka's limit on topic name length.
const MAX_TOPIC_LEN: usize = 249;

/// Checks that `id` can name a workspace's database and user: lowercase
/// letters, digits, `-` and `_`, starting with a letter or digit (both
/// sudoquery slugs and CDP workspace ids fit), and not a name ClickHouse
/// reserves. The id is used inside backticks and quotes in DDL, so nothing
/// else may get through.
pub fn check_workspace_id(id: &str) -> Result<(), String> {
    let valid = (1..=63).contains(&id.len())
        && id
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
    if !valid {
        return Err(format!(
            "workspace id {id:?} must be 1-63 lowercase letters, digits, `-` or `_`, starting with a letter or digit"
        ));
    }
    if RESERVED
        .iter()
        .any(|reserved| id.eq_ignore_ascii_case(reserved))
    {
        return Err(format!("workspace id {id:?} is reserved in ClickHouse"));
    }
    Ok(())
}

/// Checks a Kafka topic name, as Kafka itself would.
pub fn check_topic(topic: &str) -> Result<(), String> {
    if topic.is_empty() || topic.len() > MAX_TOPIC_LEN {
        return Err(format!("topic length must be 1..={MAX_TOPIC_LEN}"));
    }
    if topic == "." || topic == ".." {
        return Err("`.` and `..` are reserved topic names".to_owned());
    }
    if let Some(bad) = topic
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')))
    {
        return Err(format!("{bad:?} is not allowed in a topic name"));
    }
    Ok(())
}

/// `ckh/workspace.sql` for `workspace_id`, one statement per element.
/// `workspace_id` and `topic` must have passed their checks.
pub fn workspace_statements(workspace_id: &str, topic: &str, kafka_brokers: &str) -> Vec<String> {
    let consumer_group = format!("clickhouse.{topic}");
    // Workspace ids and topics are restricted to characters that are safe
    // inside backticks and quotes; the broker list comes from configuration,
    // so it is escaped.
    let sql = strip_comments(WORKSPACE_SQL)
        .replace("{database}", workspace_id)
        .replace("{workspace_id}", workspace_id)
        .replace("{topic}", topic)
        .replace("{consumer_group}", &consumer_group)
        .replace("{kafka_brokers}", &escape_sql_string(kafka_brokers));
    sql.split(';')
        .map(str::trim)
        .filter(|statement| !statement.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Creates the workspace's database, tables, Kafka table and views.
pub async fn create_database(
    clickhouse: &ClickHouse,
    workspace_id: &str,
    topic: &str,
    kafka_brokers: &str,
) -> Result<(), ClickHouseError> {
    for statement in workspace_statements(workspace_id, topic, kafka_brokers) {
        clickhouse.admin(&statement).await?;
    }
    Ok(())
}

/// Creates the workspace's user, which may read its own tables and nothing
/// else.
pub async fn create_user(
    clickhouse: &ClickHouse,
    workspace_id: &str,
) -> Result<(), ClickHouseError> {
    clickhouse
        .admin(&format!(
            "CREATE USER IF NOT EXISTS '{workspace_id}' IDENTIFIED WITH sha256_password BY '{}'",
            escape_sql_string(clickhouse.workspace_password())
        ))
        .await?;
    for table in WORKSPACE_TABLES {
        clickhouse
            .admin(&format!(
                "GRANT SELECT ON `{workspace_id}`.{table} TO '{workspace_id}'"
            ))
            .await?;
    }
    Ok(())
}

/// Drops the workspace's database with its Kafka table and views,
/// permanently deleting its events. Dropping the Kafka table stops ClickHouse
/// consuming before the topic disappears.
pub async fn drop_database(
    clickhouse: &ClickHouse,
    workspace_id: &str,
) -> Result<(), ClickHouseError> {
    clickhouse
        .admin(&format!("DROP DATABASE IF EXISTS `{workspace_id}` SYNC"))
        .await
        .map(drop)
}

pub async fn drop_user(clickhouse: &ClickHouse, workspace_id: &str) -> Result<(), ClickHouseError> {
    clickhouse
        .admin(&format!("DROP USER IF EXISTS '{workspace_id}'"))
        .await
        .map(drop)
}

fn strip_comments(sql: &str) -> String {
    sql.lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn escape_sql_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORKSPACE: &str = "web-shop-9x2k1a";
    const TOPIC: &str = "events.acme-k3x9qa.web-shop-9x2k1a";

    #[test]
    fn statements_fill_every_placeholder() {
        let statements = workspace_statements(WORKSPACE, TOPIC, "redpanda:9092");

        assert_eq!(
            statements[0],
            "CREATE DATABASE IF NOT EXISTS `web-shop-9x2k1a`"
        );
        assert!(statements.iter().all(|s| s.contains("IF NOT EXISTS")));
        for placeholder in [
            "{database}",
            "{workspace_id}",
            "{topic}",
            "{kafka_brokers}",
            "{consumer_group}",
        ] {
            assert!(
                statements.iter().all(|s| !s.contains(placeholder)),
                "{placeholder} left"
            );
        }
        let kafka = statements
            .iter()
            .find(|s| s.contains("ENGINE = Kafka"))
            .expect("a Kafka table");
        assert!(
            kafka.contains("kafka_topic_list           = 'events.acme-k3x9qa.web-shop-9x2k1a'")
        );
        assert!(kafka.contains(
            "kafka_group_name           = 'clickhouse.events.acme-k3x9qa.web-shop-9x2k1a'"
        ));
        assert!(kafka.contains("kafka_broker_list          = 'redpanda:9092'"));
        assert!(
            statements
                .iter()
                .filter(|s| s.contains("WHERE workspace_id"))
                .all(|s| s.contains("WHERE workspace_id = 'web-shop-9x2k1a'"))
        );
    }

    #[test]
    fn broker_list_is_escaped() {
        let statements = workspace_statements(WORKSPACE, TOPIC, "evil', x='y");
        assert!(statements.iter().any(|s| s.contains(r"'evil\', x=\'y'")));
    }

    #[test]
    fn workspace_ids_that_are_safe_in_ddl_pass() {
        for id in ["web-shop-9x2k1a", "d2cmerino", "acme_shop", "9lives"] {
            assert_eq!(check_workspace_id(id), Ok(()), "{id}");
        }
    }

    #[test]
    fn workspace_ids_that_could_break_ddl_or_reach_clickhouses_own_objects_fail() {
        let too_long = "x".repeat(64);
        for id in [
            "",
            "Shop",
            "-shop",
            "_shop",
            "shop`; DROP",
            "shop'",
            "a b",
            too_long.as_str(),
            "default",
            "system",
            "information_schema",
        ] {
            assert!(check_workspace_id(id).is_err(), "{id:?}");
        }
    }

    #[test]
    fn topics_follow_kafkas_rules() {
        assert_eq!(check_topic(TOPIC), Ok(()));
        for topic in ["", ".", "..", "a b", "a'b", "a,b"] {
            assert!(check_topic(topic).is_err(), "{topic:?}");
        }
        assert!(check_topic(&"x".repeat(250)).is_err());
    }
}
