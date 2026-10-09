//! A throwaway database on the dev Postgres, migrated from scratch and
//! dropped afterwards, so e2e runs never touch dev data.

use anyhow::Context;
use sqlx::{Connection, Executor, PgPool};

use crate::db::{database_url, load_migrator, mask_url, wait_for_database};

pub(crate) struct TestDb {
    admin_url: String,
    name: String,
    pub(crate) url: String,
    pub(crate) pool: PgPool,
}

impl TestDb {
    pub(crate) async fn create() -> anyhow::Result<Self> {
        let admin_url = database_url();
        let mut admin = wait_for_database(&admin_url).await;

        let name = format!("dashboard_e2e_{}", uuid::Uuid::new_v4().simple());
        admin
            .execute(format!(r#"CREATE DATABASE "{name}""#).as_str())
            .await
            .with_context(|| format!("failed to create database {name}"))?;
        admin.close().await?;

        let url = with_database(&admin_url, &name);
        println!("e2e database: {}", mask_url(&url));
        let db = Self {
            admin_url,
            name,
            pool: PgPool::connect(&url)
                .await
                .context("failed to connect to the e2e database")?,
            url,
        };

        // An empty database takes the whole migration chain, baseline included.
        let migrated = async {
            load_migrator()
                .await?
                .run(&db.pool)
                .await
                .context("failed to migrate the e2e database")
        }
        .await;
        if let Err(err) = migrated {
            db.drop_database().await?;
            return Err(err);
        }
        Ok(db)
    }

    pub(crate) async fn drop_database(&self) -> anyhow::Result<()> {
        self.pool.close().await;
        let mut admin = sqlx::PgConnection::connect(&self.admin_url).await?;
        admin
            .execute(format!(r#"DROP DATABASE IF EXISTS "{}" WITH (FORCE)"#, self.name).as_str())
            .await
            .with_context(|| format!("failed to drop database {}", self.name))?;
        Ok(())
    }
}

/// `url` with its database name replaced by `name`.
fn with_database(url: &str, name: &str) -> String {
    let (base, query) = match url.split_once('?') {
        Some((base, query)) => (base, Some(query)),
        None => (url, None),
    };
    let server = base.rsplit_once('/').map_or(base, |(server, _)| server);
    match query {
        Some(query) => format!("{server}/{name}?{query}"),
        None => format!("{server}/{name}"),
    }
}

#[cfg(test)]
mod tests {
    use super::with_database;

    #[test]
    fn replaces_the_database_name() {
        assert_eq!(
            with_database("postgres://u:p@localhost:5433/hyper_analytics", "e2e"),
            "postgres://u:p@localhost:5433/e2e"
        );
        assert_eq!(
            with_database("postgres://u:p@h/db?sslmode=disable", "e2e"),
            "postgres://u:p@h/e2e?sslmode=disable"
        );
    }
}
