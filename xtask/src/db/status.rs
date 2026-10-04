//! `cargo xtask db status` — verify the DB is up and all migrations are applied.

use super::{
    applied_migrations, database_url, load_migrator, mask_url, pending_migrations,
    public_schema_has_tables, tracking_table_exists, wait_for_database,
};

pub(crate) async fn run() -> anyhow::Result<()> {
    let url = database_url();
    let mut conn = wait_for_database(&url).await;
    println!("database: up ({})", mask_url(&url));

    let migrator = load_migrator().await?;

    if !tracking_table_exists(&mut conn).await? {
        let schema_present = public_schema_has_tables(&mut conn).await?;
        let migrations_defined = migrator.iter().next().is_some();
        if !schema_present && !migrations_defined {
            println!("migrations: none defined");
            return Ok(());
        }
        println!(
            "migrations: no migration history \
             — run `cargo xtask db migration` to baseline and apply"
        );
        std::process::exit(1);
    }

    let applied_rows = applied_migrations(&mut conn).await?;
    let mut applied = Vec::with_capacity(applied_rows.len());
    let mut dirty = Vec::new();
    for (version, success) in applied_rows {
        if success {
            applied.push(version);
        } else {
            dirty.push(version);
        }
    }
    if !dirty.is_empty() {
        let versions = dirty
            .iter()
            .map(|version| version.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!("warning: dirty migration(s) need a manual fix: {versions}");
    }

    let pending = pending_migrations(&migrator, &applied);
    println!(
        "migrations: {} applied, {} pending",
        applied.len(),
        pending.len()
    );
    for migration in &pending {
        println!("  pending: {} {}", migration.version, migration.description);
    }

    if dirty.is_empty() && pending.is_empty() {
        Ok(())
    } else {
        std::process::exit(1);
    }
}
