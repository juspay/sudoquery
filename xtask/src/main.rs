mod compose;
mod db;

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn run(program: &str, args: &[&str]) {
    let status = Command::new(program)
        .args(args)
        .current_dir(workspace_root())
        .status()
        .unwrap_or_else(|err| panic!("failed to spawn {program}: {err}"));
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn cargo(args: &[&str]) {
    run(&env::var("CARGO").unwrap_or_else(|_| "cargo".into()), args);
}

fn fmt(check: bool) {
    if check {
        cargo(&["fmt", "--all", "--", "--check"]);
    } else {
        cargo(&["fmt", "--all"]);
    }
}

fn clippy() {
    cargo(&[
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ]);
}

fn test() {
    cargo(&["test", "--workspace"]);
}

fn ci() {
    fmt(true);
    clippy();
    test();
}

const HELP: &str = "Usage: cargo xtask <TASK>

Tasks:
  fmt [--check]   Format the workspace (check only with --check)
  clippy          Lint the workspace, warnings are errors
  test            Run all workspace tests
  ci              fmt --check, clippy and test
  db status       Check the database is up and all migrations are applied
  db migration    Apply pending migrations (auto-baselines the pg_schema.sql base)
  db migration add <name>  Create a new migration file
  ls              List available tasks
  ps              Show this repo's docker compose containers and status
  setup [svc...]  Choose docker compose services to start";

fn help() -> ! {
    eprintln!("{HELP}");
    std::process::exit(2);
}

fn main() {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("fmt") => fmt(args.any(|arg| arg == "--check")),
        Some("clippy") => clippy(),
        Some("test") => test(),
        Some("ci") => ci(),
        Some("ls") => println!("{HELP}"),
        Some("ps") => compose::ps(),
        Some("setup") => compose::setup(&args.collect::<Vec<String>>()),
        Some("db") => db::main(args.collect()),
        _ => help(),
    }
}
