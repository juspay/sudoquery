//! `cargo xtask db` — database migration management for the dev database.

mod migrate;
mod status;

use std::env;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Context;
use sqlx::migrate::{Migration, Migrator};
use sqlx::{Connection, PgConnection, Postgres};
use tokio::time::sleep;

const DEFAULT_DATABASE_URL: &str =
    "postgres://hyper_analytics_user:hyper_analytics@localhost:5433/hyper_analytics";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const RETRY_INTERVAL: Duration = Duration::from_secs(1);

pub fn main(args: Vec<String>) {
    let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    if let Err(err) = runtime.block_on(dispatch(&args)) {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

async fn dispatch(args: &[String]) -> anyhow::Result<()> {
    match args.first().map(String::as_str) {
        Some("status") => status::run().await,
        Some("migration") => match args.get(1).map(String::as_str) {
            None => migrate::run().await,
            Some("add") => match args.get(2).map(String::as_str) {
                Some(name) => migrate::add(name),
                None => usage_error("db migration add requires a <name> argument"),
            },
            Some(other) => usage_error(&format!("unknown `db migration` argument: {other:?}")),
        },
        _ => usage(),
    }
}

fn usage() -> ! {
    eprintln!(
        "Usage: cargo xtask db <COMMAND>

Commands:
  status                Check the database is up and all migrations are applied
  migration             Apply pending migrations (auto-baselines the pg_schema.sql base)
  migration add <name>  Create a new migration file"
    );
    std::process::exit(2);
}

pub(crate) fn usage_error(message: &str) -> ! {
    eprintln!("error: {message}");
    eprintln!("run `cargo xtask db` for usage");
    std::process::exit(2);
}

pub(crate) fn database_url() -> String {
    match env::var("DATABASE_URL") {
        Ok(url) if !url.trim().is_empty() => url,
        _ => DEFAULT_DATABASE_URL.to_owned(),
    }
}

/// Mask the password portion of a connection URL: `postgres://user:***@host:port/db`.
/// URLs without a `user:password@` segment are returned unchanged.
pub(crate) fn mask_url(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else {
        return url.to_owned();
    };
    let authority = &url[scheme_end + 3..];
    let Some(at) = authority.find('@') else {
        return url.to_owned();
    };
    let userinfo = &authority[..at];
    let Some(password_start) = userinfo.find(':').map(|colon| scheme_end + 3 + colon) else {
        return url.to_owned();
    };
    format!("{}***{}", &url[..password_start + 1], &authority[at..])
}

/// Try to connect until success or the 30s deadline; exit 1 with a hint on timeout.
/// Prints nothing extra on success — callers report status.
pub(crate) async fn wait_for_database(url: &str) -> PgConnection {
    println!("waiting for database (up to 30s): {}", mask_url(url));
    let deadline = Instant::now() + CONNECT_TIMEOUT;
    loop {
        match PgConnection::connect(url).await {
            Ok(conn) => return conn,
            Err(err) if Instant::now() >= deadline => {
                eprintln!("database: down ({err})");
                eprintln!("hint: start the dev database with `sudo docker compose up -d postgres`");
                std::process::exit(1);
            }
            Err(_) => sleep(RETRY_INTERVAL).await,
        }
    }
}

pub(crate) fn migrations_dir() -> PathBuf {
    crate::workspace_root().join("migrations")
}

pub(crate) async fn load_migrator() -> anyhow::Result<Migrator> {
    let dir = migrations_dir();
    Migrator::new(dir.clone())
        .await
        .with_context(|| format!("failed to load migrations from {}", dir.display()))
}

pub(crate) async fn tracking_table_exists<'c, E>(executor: E) -> anyhow::Result<bool>
where
    E: sqlx::Executor<'c, Database = Postgres>,
{
    sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM information_schema.tables
            WHERE table_schema = 'public' AND table_name = '_sqlx_migrations'
        )",
    )
    .fetch_one(executor)
    .await
    .context("failed to check whether the _sqlx_migrations table exists")
}

pub(crate) async fn public_schema_has_tables<'c, E>(executor: E) -> anyhow::Result<bool>
where
    E: sqlx::Executor<'c, Database = Postgres>,
{
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = 'public')",
    )
    .fetch_one(executor)
    .await
    .context("failed to check whether the public schema has tables")
}

/// `(version, success)` rows from `_sqlx_migrations`, ordered by version.
pub(crate) async fn applied_migrations<'c, E>(executor: E) -> anyhow::Result<Vec<(i64, bool)>>
where
    E: sqlx::Executor<'c, Database = Postgres>,
{
    sqlx::query_as("SELECT version, success FROM _sqlx_migrations ORDER BY version")
        .fetch_all(executor)
        .await
        .context("failed to read applied migrations from _sqlx_migrations")
}

/// Up-migrations from the source directory that are not recorded as applied.
pub(crate) fn pending_migrations<'m>(
    migrator: &'m Migrator,
    applied: &[i64],
) -> Vec<&'m Migration> {
    migrator
        .iter()
        .filter(|migration| !migration.migration_type.is_down_migration())
        .filter(|migration| !applied.contains(&migration.version))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::mask_url;

    #[test]
    fn masks_password_in_url() {
        let url = "postgres://hyper_analytics_user:hyper_analytics@localhost:5433/hyper_analytics";
        assert_eq!(
            mask_url(url),
            "postgres://hyper_analytics_user:***@localhost:5433/hyper_analytics"
        );
    }

    #[test]
    fn leaves_url_without_password_unchanged() {
        assert_eq!(
            mask_url("postgres://user@localhost:5433/db"),
            "postgres://user@localhost:5433/db"
        );
    }

    #[test]
    fn leaves_non_url_unchanged() {
        assert_eq!(mask_url("not a url"), "not a url");
    }
}
