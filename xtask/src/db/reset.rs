//! `cargo xtask db reset` — drop everything in the dev database and migrate
//! it from scratch, baseline included.

use std::io::{self, BufRead, Write};

use anyhow::Context;
use sqlx::{Executor, PgPool};

use super::{database_url, load_migrator, mask_url, wait_for_database};

pub(crate) async fn run(yes: bool) -> anyhow::Result<()> {
    let url = database_url();
    if !yes && !confirm(&url)? {
        println!("aborted");
        return Ok(());
    }

    wait_for_database(&url).await;
    let pool = PgPool::connect(url.as_str())
        .await
        .with_context(|| format!("failed to connect to database ({})", mask_url(&url)))?;

    // Dropping the schema also drops _sqlx_migrations, so the whole chain
    // re-runs instead of auto-baselining.
    pool.execute("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .await
        .context("failed to drop and recreate the public schema")?;
    println!("dropped and recreated schema public");

    let migrator = load_migrator().await?;
    migrator
        .run(&pool)
        .await
        .context("failed to apply migrations")?;

    let applied = migrator
        .iter()
        .filter(|migration| !migration.migration_type.is_down_migration())
        .count();
    println!("migrations: {applied} applied, 0 pending");
    Ok(())
}

fn confirm(url: &str) -> anyhow::Result<bool> {
    print!(
        "this drops ALL data in {} — continue? [y/N] ",
        mask_url(url)
    );
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes"))
}
