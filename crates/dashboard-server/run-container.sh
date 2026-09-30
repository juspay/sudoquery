#!/bin/bash
set -e

IMAGE_NAME="hyper-analytics-dashboard"
CONTAINER_NAME="hyper-analytics-dashboard-server"

# Build the image
echo "Building container image..."
podman build -t "$IMAGE_NAME" .

# Stop and remove existing container if running
if podman ps -a --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    echo "Stopping existing container..."
    podman stop "$CONTAINER_NAME" 2>/dev/null || true
    podman rm "$CONTAINER_NAME" 2>/dev/null || true
fi

# Get host IP for ClickHouse connection
HOST_IP=$(hostname -I | awk '{print $1}')
echo "Host IP: $HOST_IP"

# Run container with host network access
echo "Starting container..."
podman run \
    --name "$CONTAINER_NAME" \
    --network host \
    -e CLICKHOUSE_URL="http://localhost:8123" \
    -e RUST_LOG=info \
    "$IMAGE_NAME"
