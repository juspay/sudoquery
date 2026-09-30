use std::env;
use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn cargo(args: &[&str]) {
    let status = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(args)
        .current_dir(workspace_root())
        .status()
        .expect("failed to spawn cargo");
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
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

fn help() -> ! {
    eprintln!(
        "Usage: cargo xtask <TASK>

Tasks:
  fmt [--check]   Format the workspace (check only with --check)
  clippy          Lint the workspace, warnings are errors
  test            Run all workspace tests
  ci              fmt --check, clippy and test"
    );
    std::process::exit(2);
}

fn main() {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("fmt") => fmt(args.any(|arg| arg == "--check")),
        Some("clippy") => clippy(),
        Some("test") => test(),
        Some("ci") => ci(),
        _ => help(),
    }
}
