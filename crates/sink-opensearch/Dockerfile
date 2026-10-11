# syntax=docker/dockerfile:1

# The build context is the workspace root, because this crate depends on
# crates/canonical-event:
#   docker build -f crates/sink-opensearch/Dockerfile .

FROM rust:1-bookworm AS builder

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        cmake \
        libssl-dev \
        libsasl2-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY xtask ./xtask

RUN cargo build --release --locked -p sink-opensearch

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        libsasl2-2 \
        libssl3 \
        zlib1g \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/sink-opensearch /usr/local/bin/sink-opensearch
# Default SINK_CONFIG path. It points at the local stack; override it with
# SINK_CONFIG, or any setting's environment variable (OPENSEARCH_URL,
# KAFKA_BOOTSTRAP_SERVERS, BATCH_MAX_DOCS, ...).
COPY crates/sink-opensearch/cac.toml ./cac.toml

# /health and /metrics (server.addr in cac.toml)
EXPOSE 9464

CMD ["sink-opensearch"]
