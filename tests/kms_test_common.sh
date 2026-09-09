ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="${ROOT_DIR}/tests/docker-compose.yml"

LOCALSTACK_ADDR="localhost:4566"
AWS_REGION="us-east-1"
TOKEN="local-test-secret-token"

SERVER_PID=""
WORK_DIR=""
COLLECTOR_ADDR=""

cleanup() {
  if [[ -n "${SERVER_PID}" ]]; then
    kill "${SERVER_PID}" 2>/dev/null || true
    wait "${SERVER_PID}" 2>/dev/null || true
  fi
  if [[ -n "${WORK_DIR}" ]]; then
    rm -rf "${WORK_DIR}"
  fi
}

wait_until() {
  local description="$1"
  local attempts="$2"
  shift 2

  local _
  for _ in $(seq 1 "${attempts}"); do
    if "$@" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done

  echo "timed out waiting for ${description}" >&2
  return 1
}

find_free_port() {
  local port="$1"
  while curl -s -o /dev/null --max-time 1 "http://localhost:${port}/"; do
    port=$((port + 1))
  done
  echo "${port}"
}

start_infra() {
  echo "starting redpanda and localstack"
  docker compose -f "${COMPOSE_FILE}" up -d redpanda localstack

  wait_until "localstack" 60 curl -sf "http://${LOCALSTACK_ADDR}/_localstack/health"
  wait_until "redpanda" 60 docker exec redpanda rpk cluster info
}

restart_localstack() {
  docker restart localstack >/dev/null
  wait_until "localstack" 60 curl -sf "http://${LOCALSTACK_ADDR}/_localstack/health"
}

create_key() {
  docker exec localstack awslocal kms create-key \
    --description "event-collector health test key" \
    --query 'KeyMetadata.KeyId' \
    --output text
}

encrypt_token() {
  local key_id="$1"
  docker exec localstack awslocal kms encrypt \
    --key-id "${key_id}" \
    --plaintext "${TOKEN}" \
    --query 'CiphertextBlob' \
    --output text
}

start_collector() {
  local kms_ciphertext="$1"
  local collector_port

  echo "building collector"
  cargo build --manifest-path "${ROOT_DIR}/Cargo.toml" --bin event-collector

  WORK_DIR="$(mktemp -d)"
  collector_port="$(find_free_port 3000)"
  COLLECTOR_ADDR="localhost:${collector_port}"

  sed -E "s/addr = \"[^\"]*\"/addr = \"127.0.0.1:${collector_port}\"/" \
    "${ROOT_DIR}/cac.toml" > "${WORK_DIR}/cac.toml"
  if ! grep -q "addr = \"127.0.0.1:${collector_port}\"" "${WORK_DIR}/cac.toml"; then
    echo "failed to override server addr in cac.toml" >&2
    exit 1
  fi

  echo "starting collector on ${COLLECTOR_ADDR}"
  (cd "${WORK_DIR}" && exec env \
    KAFKA_BOOTSTRAP_SERVERS="localhost:19092" \
    TOKEN_PROVIDER="aws" \
    KMS_CIPHERTEXT="${kms_ciphertext}" \
    AWS_ACCESS_KEY_ID="test" \
    AWS_SECRET_ACCESS_KEY="test" \
    AWS_REGION="${AWS_REGION}" \
    AWS_ENDPOINT_URL="http://${LOCALSTACK_ADDR}" \
    "${ROOT_DIR}/target/debug/event-collector") &
  SERVER_PID=$!

  wait_until "collector health endpoint" 60 curl -sf -o /dev/null "http://${COLLECTOR_ADDR}/cdp/collect/health"
}

health_response() {
  curl -sf "http://${COLLECTOR_ADDR}/cdp/collect/health"
}
