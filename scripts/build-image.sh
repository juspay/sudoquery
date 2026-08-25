#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="${1:-event-collector:local}"

docker build \
  --file "${ROOT_DIR}/Dockerfile" \
  --tag "${IMAGE}" \
  "${ROOT_DIR}"

echo "Built Docker image: ${IMAGE}"
