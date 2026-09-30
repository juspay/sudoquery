# --- Stage 1: Build ---
FROM public.ecr.aws/docker/library/rust:1.88-slim AS builder

WORKDIR /app

# Copy Cargo manifests
COPY Cargo.toml Cargo.lock ./

# Copy Rust source
COPY src/ ./src/
COPY migrations/ ./migrations/

RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Build Rust binary
RUN cargo build --release

# --- Stage 2: Runtime ---
FROM public.ecr.aws/docker/library/debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy Rust binary
COPY --from=builder /app/target/release/hyper-analytics-dashboard-server ./server

EXPOSE 3000

CMD ["./server"]
