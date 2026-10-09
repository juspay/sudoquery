//! `cargo xtask e2e <suite> [filter]` — end-to-end tests that build and run a
//! real service binary against local infrastructure.
//!
//! * `dashboard`: dashboard-server against a throwaway database on the local
//!   compose Postgres, with an in-process dummy OIDC provider and dummy
//!   ClickHouse.
//! * `collector`: events-collector against the local compose Redpanda,
//!   reading published events back from per-test topics.

/// Declares a suite's `CASES` table from its async case functions, each
/// `async fn(&Ctx) -> anyhow::Result<()>`.
macro_rules! cases {
    ($ctx:ty: $($case:ident),* $(,)?) => {
        pub(crate) const CASES: &[$crate::e2e::Case<$ctx>] = &[$((stringify!($case), {
            fn run(ctx: &$ctx) -> $crate::e2e::CaseFuture<'_> {
                Box::pin($case(ctx))
            }
            run
        })),*];
    };
}

mod collector;
mod dashboard;
mod db;
mod kafka;
mod mock;
mod server;

use std::future::Future;
use std::pin::Pin;

use anyhow::Context;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;

pub(crate) type CaseFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<()>> + 'a>>;

/// A test case: a name (matched by the CLI filter) and the async body.
pub(crate) type Case<C> = (&'static str, for<'a> fn(&'a C) -> CaseFuture<'a>);

pub fn main(args: Vec<String>) {
    let filter = args.get(1).cloned();
    let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    let outcome = runtime.block_on(async {
        match args.first().map(String::as_str) {
            Some("dashboard") => dashboard::run(filter.as_deref()).await,
            Some("collector") => collector::run(filter.as_deref()).await,
            _ => usage(),
        }
    });
    match outcome {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(err) => {
            eprintln!("error: {err:#}");
            std::process::exit(1);
        }
    }
}

fn usage() -> ! {
    eprintln!(
        "Usage: cargo xtask e2e <SUITE> [filter]

Suites:
  dashboard   Org and project creation against dashboard-server
              (needs the compose postgres: `cargo xtask setup postgres`)
  collector   Event ingestion through events-collector into Kafka
              (needs the compose redpanda: `cargo xtask setup redpanda`)

[filter] runs only the cases whose name contains it."
    );
    std::process::exit(2);
}

/// The cases of `suite` whose name contains `filter` (all when `None`).
pub(crate) fn select<'s, C>(
    suite: &'s [Case<C>],
    filter: Option<&str>,
) -> anyhow::Result<Vec<&'s Case<C>>> {
    let cases: Vec<&Case<C>> = suite
        .iter()
        .filter(|(name, _)| filter.is_none_or(|filter| name.contains(filter)))
        .collect();
    if let (true, Some(filter)) = (cases.is_empty(), filter) {
        anyhow::bail!("no test case matches {filter:?}");
    }
    Ok(cases)
}

/// Runs `cases` sequentially, calling `before_each` ahead of every case and
/// `on_failure` once at the end if any failed. Returns whether all passed.
pub(crate) async fn run_cases<C>(
    ctx: &C,
    cases: &[&Case<C>],
    before_each: impl Fn(&C),
    on_failure: impl FnOnce(),
) -> bool {
    println!("\nrunning {} case(s)", cases.len());
    let mut failed = Vec::new();
    for (name, case) in cases {
        before_each(ctx);
        match case(ctx).await {
            Ok(()) => println!("case {name} ... ok"),
            Err(err) => {
                println!("case {name} ... FAILED");
                failed.push((*name, err));
            }
        }
    }

    if !failed.is_empty() {
        println!("\nfailures:");
        for (name, err) in &failed {
            println!("\n---- {name} ----\n{err:#}");
        }
        on_failure();
    }
    println!(
        "\nresult: {}. {} passed; {} failed",
        if failed.is_empty() { "ok" } else { "FAILED" },
        cases.len() - failed.len(),
        failed.len()
    );
    failed.is_empty()
}

/// An HTTP response reduced to what the tests assert on.
pub(crate) struct Response {
    pub(crate) status: u16,
    pub(crate) headers: HeaderMap,
    pub(crate) body: String,
}

impl Response {
    pub(crate) fn json(&self) -> anyhow::Result<Value> {
        serde_json::from_str(&self.body)
            .with_context(|| format!("response body is not JSON: {}", self.body))
    }
}

/// HTTP client bound to one service's base URL.
pub(crate) struct Http {
    base: String,
    client: reqwest::Client,
}

impl Http {
    pub(crate) fn new(base: String) -> Self {
        Self {
            base,
            client: reqwest::Client::new(),
        }
    }

    pub(crate) async fn post(
        &self,
        path: &str,
        token: Option<&str>,
        headers: &[(&str, &str)],
        body: Value,
    ) -> anyhow::Result<Response> {
        let request = self
            .client
            .post(format!("{}{path}", self.base))
            .headers(build_headers(token, headers)?)
            .json(&body);
        send(request).await
    }

    /// POSTs `body` verbatim, for endpoints that take non-JSON-document
    /// bodies (e.g. newline-delimited events).
    pub(crate) async fn post_raw(
        &self,
        path: &str,
        headers: &[(&str, &str)],
        body: String,
    ) -> anyhow::Result<Response> {
        let request = self
            .client
            .post(format!("{}{path}", self.base))
            .headers(build_headers(None, headers)?)
            .body(body);
        send(request).await
    }

    pub(crate) async fn get(
        &self,
        path: &str,
        token: Option<&str>,
        headers: &[(&str, &str)],
    ) -> anyhow::Result<Response> {
        let request = self
            .client
            .get(format!("{}{path}", self.base))
            .headers(build_headers(token, headers)?);
        send(request).await
    }
}

fn build_headers(token: Option<&str>, headers: &[(&str, &str)]) -> anyhow::Result<HeaderMap> {
    let mut map = HeaderMap::new();
    if let Some(token) = token {
        map.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}"))?,
        );
    }
    for (name, value) in headers {
        map.insert(
            HeaderName::from_bytes(name.as_bytes())?,
            HeaderValue::from_str(value)?,
        );
    }
    Ok(map)
}

async fn send(request: reqwest::RequestBuilder) -> anyhow::Result<Response> {
    let response = request.send().await.context("request to service failed")?;
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let body = response.text().await.unwrap_or_default();
    Ok(Response {
        status,
        headers,
        body,
    })
}
