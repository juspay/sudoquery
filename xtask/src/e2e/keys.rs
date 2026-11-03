//! The collector's key store for the e2e run: a throwaway database on the
//! dev Postgres with `collector_keys` (from the collector's own
//! `collector_keys.sql`), and a role that may only read it, which the
//! collector connects as — as it would in a deployment. Both are dropped
//! afterwards.

use anyhow::Context;
use chrono::{DateTime, Utc};
use rand::RngCore;
use sha2::{Digest, Sha256};
use sqlx::{Connection, Executor, PgPool};

use super::db::with_database;
use crate::db::{database_url, mask_url, wait_for_database};

pub(crate) struct KeyStore {
    admin_url: String,
    name: String,
    pool: PgPool,
    /// What the collector connects with: the read-only role.
    pub(crate) collector_url: String,
}

/// A key as a client holds it, and its id.
pub(crate) struct Key {
    pub(crate) kid: String,
    pub(crate) bearer: String,
}

impl KeyStore {
    pub(crate) async fn create() -> anyhow::Result<Self> {
        let admin_url = database_url();
        let mut admin = wait_for_database(&admin_url).await;

        let name = format!("collector_e2e_{}", uuid::Uuid::new_v4().simple());
        admin
            .execute(format!(r#"CREATE DATABASE "{name}""#).as_str())
            .await
            .with_context(|| format!("failed to create database {name}"))?;
        // Same name as the database; it can read collector_keys, nothing else.
        let password = uuid::Uuid::new_v4().simple().to_string();
        admin
            .execute(format!(r#"CREATE ROLE "{name}" LOGIN PASSWORD '{password}'"#).as_str())
            .await
            .with_context(|| format!("failed to create role {name}"))?;
        admin.close().await?;

        let url = with_database(&admin_url, &name);
        println!("e2e key store: {}", mask_url(&url));
        let store = Self {
            collector_url: with_credentials(&url, &name, &password)?,
            pool: PgPool::connect(&url)
                .await
                .context("failed to connect to the key store")?,
            admin_url,
            name,
        };
        let schema = async {
            let ddl = std::fs::read_to_string(
                crate::workspace_root().join("crates/events-collector/collector_keys.sql"),
            )?;
            sqlx::raw_sql(&ddl).execute(&store.pool).await?;
            sqlx::raw_sql(&format!(
                r#"GRANT SELECT ON collector_keys TO "{0}";"#,
                store.name
            ))
            .execute(&store.pool)
            .await?;
            anyhow::Ok(())
        }
        .await
        .context("failed to create collector_keys");
        if let Err(err) = schema {
            store.drop().await?;
            return Err(err);
        }
        Ok(store)
    }

    /// A new key for `org_id` / `workspace_id`, expiring at `expires_at`.
    pub(crate) async fn create_key(
        &self,
        org_id: &str,
        workspace_id: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<Key> {
        let mut kid = [0u8; 8];
        let mut secret = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut kid);
        rand::thread_rng().fill_bytes(&mut secret);
        let kid = hex::encode(kid);
        sqlx::query(
            "INSERT INTO collector_keys (kid, org_id, workspace_id, hashed_secret, key_name, expires_at)
             VALUES ($1, $2, $3, $4, 'e2e', $5)",
        )
        .bind(&kid)
        .bind(org_id)
        .bind(workspace_id)
        .bind(Sha256::digest(secret).to_vec())
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .context("failed to insert a collector key")?;
        Ok(Key {
            bearer: format!("Bearer sk_{kid}_{}", hex::encode(secret)),
            kid,
        })
    }

    pub(crate) async fn revoke(&self, kid: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE collector_keys SET revoked_at = now() WHERE kid = $1")
            .bind(kid)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub(crate) async fn drop(&self) -> anyhow::Result<()> {
        self.pool.close().await;
        let mut admin = sqlx::PgConnection::connect(&self.admin_url).await?;
        admin
            .execute(format!(r#"DROP DATABASE IF EXISTS "{}" WITH (FORCE)"#, self.name).as_str())
            .await
            .with_context(|| format!("failed to drop database {}", self.name))?;
        admin
            .execute(format!(r#"DROP ROLE IF EXISTS "{}""#, self.name).as_str())
            .await
            .with_context(|| format!("failed to drop role {}", self.name))?;
        Ok(())
    }
}

/// `url` connecting as `user` / `password` instead.
fn with_credentials(url: &str, user: &str, password: &str) -> anyhow::Result<String> {
    let mut url = reqwest::Url::parse(url).context("unparseable DATABASE_URL")?;
    url.set_username(user)
        .and_then(|()| url.set_password(Some(password)))
        .map_err(|()| anyhow::anyhow!("DATABASE_URL can't carry credentials"))?;
    Ok(url.into())
}

#[cfg(test)]
mod tests {
    use super::with_credentials;

    #[test]
    fn replaces_the_credentials() {
        assert_eq!(
            with_credentials("postgres://u:p@localhost:5433/db?sslmode=disable", "r", "s").unwrap(),
            "postgres://r:s@localhost:5433/db?sslmode=disable"
        );
    }
}
