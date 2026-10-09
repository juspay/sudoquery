//! Builds and runs a workspace service binary for an e2e suite.

use std::fs::File;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::Context;

const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const LOG_TAIL_LINES: usize = 40;

pub(crate) struct Service {
    package: &'static str,
    child: Child,
    addr: String,
    log_path: PathBuf,
}

impl Service {
    /// Builds `package`'s binary; exits the process if the build fails.
    pub(crate) fn build(package: &str) {
        crate::cargo(&["build", "--quiet", "-p", package]);
    }

    /// Starts the binary built by [`Self::build`] on a free localhost port,
    /// passed to it through the `addr_env` variable, and waits until it
    /// accepts connections. Runs from `cwd`, or from a scratch directory
    /// when `None` (so no repo `.env` is picked up).
    pub(crate) async fn start(
        package: &'static str,
        addr_env: &str,
        cwd: Option<PathBuf>,
        envs: &[(&str, String)],
    ) -> anyhow::Result<Self> {
        let addr = free_local_addr()?;
        let scratch = std::env::temp_dir().join(format!("{package}-e2e-{}", std::process::id()));
        std::fs::create_dir_all(&scratch)?;
        let log_path = scratch.join("server.log");
        let log = File::create(&log_path)?;

        let child = Command::new(binary_path(package))
            .current_dir(cwd.unwrap_or(scratch))
            .env(addr_env, &addr)
            .envs(envs.iter().map(|(key, value)| (*key, value)))
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .spawn()
            .with_context(|| format!("failed to start {package}"))?;

        let mut service = Self {
            package,
            child,
            addr,
            log_path,
        };
        service.wait_until_listening().await?;
        Ok(service)
    }

    pub(crate) fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    async fn wait_until_listening(&mut self) -> anyhow::Result<()> {
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        loop {
            if let Some(status) = self.child.try_wait()? {
                self.print_log_tail();
                anyhow::bail!("{} exited during startup ({status})", self.package);
            }
            if tokio::net::TcpStream::connect(&self.addr).await.is_ok() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                self.print_log_tail();
                anyhow::bail!(
                    "{} did not listen on {} within 30s",
                    self.package,
                    self.addr
                );
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    pub(crate) fn print_log_tail(&self) {
        let log = std::fs::read_to_string(&self.log_path).unwrap_or_default();
        let lines: Vec<&str> = log.lines().collect();
        let tail = &lines[lines.len().saturating_sub(LOG_TAIL_LINES)..];
        println!(
            "\n{} log ({}, last {} lines):\n{}",
            self.package,
            self.log_path.display(),
            tail.len(),
            tail.join("\n")
        );
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn binary_path(package: &str) -> PathBuf {
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::workspace_root().join("target"));
    target_dir.join("debug").join(package)
}

/// A localhost address with a port that was free a moment ago.
fn free_local_addr() -> anyhow::Result<String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.to_string())
}
