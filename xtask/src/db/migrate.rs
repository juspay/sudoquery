//! `cargo xtask db migration` — apply pending migrations (with auto-baseline).
//! `cargo xtask db migration add <name>` — scaffold a new migration file.

use anyhow::Context;
use sqlx::PgPool;
use sqlx::migrate::Migration;
use sqlx::migrate::Migrator;

use super::usage_error;
use super::{
    applied_migrations, database_url, load_migrator, mask_url, migrations_dir, pending_migrations,
    public_schema_has_tables, tracking_table_exists, wait_for_database,
};

pub(crate) async fn run() -> anyhow::Result<()> {
    let url = database_url();
    wait_for_database(&url).await;
    let pool = PgPool::connect(url.as_str())
        .await
        .with_context(|| format!("failed to connect to database ({})", mask_url(&url)))?;

    let migrator = load_migrator().await?;

    // Auto-baseline: the dev DB is initialized from pg_schema.sql by docker, so the
    // base schema is already there — record known migrations as applied without
    // re-running them. A fresh empty DB (no schema) gets migrated from scratch by
    // `run` instead.
    let mut tracking = tracking_table_exists(&pool).await?;
    if !tracking && public_schema_has_tables(&pool).await? {
        baseline(&pool, &migrator).await?;
        tracking = true;
    }

    let applied: Vec<i64> = if tracking {
        applied_migrations(&pool)
            .await?
            .into_iter()
            .filter(|&(_, success)| success)
            .map(|(version, _)| version)
            .collect()
    } else {
        Vec::new()
    };
    let pending = pending_migrations(&migrator, &applied);

    if pending.is_empty() {
        println!("migrations: up to date ({} applied)", applied.len());
        return Ok(());
    }

    println!("applying {} migration(s):", pending.len());
    for migration in &pending {
        println!("  {} {}", migration.version, migration.description);
    }

    migrator
        .run(&pool)
        .await
        .context("failed to apply migrations")?;

    println!(
        "migrations: {} applied, 0 pending",
        applied.len() + pending.len()
    );
    Ok(())
}

async fn baseline(pool: &PgPool, migrator: &Migrator) -> anyhow::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS _sqlx_migrations (
            version BIGINT PRIMARY KEY,
            description TEXT NOT NULL,
            installed_on TIMESTAMPTZ NOT NULL DEFAULT now(),
            success BOOLEAN NOT NULL,
            checksum BYTEA NOT NULL,
            execution_time BIGINT NOT NULL
        )",
    )
    .execute(pool)
    .await
    .context("failed to create the _sqlx_migrations tracking table")?;

    let migrations: Vec<&Migration> = migrator
        .iter()
        .filter(|migration| !migration.migration_type.is_down_migration())
        .collect();

    for migration in &migrations {
        sqlx::query(
            "INSERT INTO _sqlx_migrations
                (version, description, installed_on, success, checksum, execution_time)
             VALUES ($1, $2, now(), true, $3, 0)
             ON CONFLICT (version) DO NOTHING",
        )
        .bind(migration.version)
        .bind(migration.description.as_ref())
        .bind(migration.checksum.as_ref())
        .execute(pool)
        .await
        .with_context(|| {
            format!(
                "failed to baseline migration {} {}",
                migration.version, migration.description
            )
        })?;
    }

    println!(
        "baselined {} existing migration(s) against the existing schema \
         (pg_schema.sql is the baseline; not re-run)",
        migrations.len()
    );
    Ok(())
}

pub(crate) fn add(name: &str) -> anyhow::Result<()> {
    if !is_valid_name(name) {
        usage_error(&format!(
            "invalid migration name {name:?}: must match ^[a-z0-9_]+$"
        ));
    }
    let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S");
    let path = migrations_dir().join(format!("{timestamp}_{name}.sql"));
    if path.exists() {
        anyhow::bail!("migration file already exists: {}", path.display());
    }
    std::fs::write(&path, "")
        .with_context(|| format!("failed to create migration file {}", path.display()))?;
    println!("{}", path.display());
    println!("write your SQL, then run `cargo xtask db migration`");
    Ok(())
}

fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::is_valid_name;

    #[test]
    fn accepts_lowercase_alphanumeric_and_underscore_names() {
        assert!(is_valid_name("add_users_table"));
        assert!(is_valid_name("2026_backfill"));
        assert!(is_valid_name("a"));
    }

    #[test]
    fn rejects_empty_uppercase_space_and_punctuation_names() {
        assert!(!is_valid_name(""));
        assert!(!is_valid_name("Bad Name!"));
        assert!(!is_valid_name("camelCase"));
        assert!(!is_valid_name("dash-name"));
    }
}
