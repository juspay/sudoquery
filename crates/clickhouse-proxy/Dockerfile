# syntax=docker/dockerfile:1

# The build context is the workspace root, because this crate depends on
# crates/canonical-event:
#   docker build -f crates/clickhouse-proxy/Dockerfile .

FROM rust:1-bookworm AS builder

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        libssl-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY xtask ./xtask
# Per-workspace ClickHouse DDL, embedded with include_str! (src/provisioning.rs)
COPY ckh/workspace.sql ./ckh/workspace.sql

RUN cargo build --release --locked -p clickhouse-proxy

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/clickhouse-proxy /usr/local/bin/clickhouse-proxy

# DATABASE_URL, CLICKHOUSE_URL / CLICKHOUSE_ADMIN_*, CLICKHOUSE_WORKSPACE_PASSWORD,
# CLICKHOUSE_KAFKA_BROKERS, CLICKHOUSE_PROXY_TOKEN; see the root README.
# Migrations are embedded and run at startup.
ENV CLICKHOUSE_PROXY_ADDR=0.0.0.0:3003
EXPOSE 3003

CMD ["clickhouse-proxy"]
