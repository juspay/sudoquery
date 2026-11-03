# syntax=docker/dockerfile:1

# The build context is the workspace root, because this crate depends on
# crates/canonical-event:
#   docker build -f crates/opensearch-proxy/Dockerfile .

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

RUN cargo build --release --locked -p opensearch-proxy

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/opensearch-proxy /usr/local/bin/opensearch-proxy

# OPENSEARCH_URL, OPENSEARCH_INDEX, OPENSEARCH_USERNAME / OPENSEARCH_PASSWORD,
# OPENSEARCH_PROXY_TOKEN; see the root README.
ENV OPENSEARCH_PROXY_ADDR=0.0.0.0:3002
EXPOSE 3002

CMD ["opensearch-proxy"]
