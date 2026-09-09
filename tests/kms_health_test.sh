#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/kms_test_common.sh"
trap cleanup EXIT

start_infra

echo "creating kms key and encrypting token"
KEY_ID="$(create_key)"
KMS_CIPHERTEXT="$(encrypt_token "${KEY_ID}")"

start_collector "${KMS_CIPHERTEXT}"

RESPONSE="$(health_response)"

if ! echo "${RESPONSE}" | jq -e '.status == "ok" and .authenticated_endpoint == "ok"' >/dev/null; then
  echo "FAIL: expected authenticated_endpoint \"ok\", got: ${RESPONSE}" >&2
  exit 1
fi

echo "PASS: ${RESPONSE}"
