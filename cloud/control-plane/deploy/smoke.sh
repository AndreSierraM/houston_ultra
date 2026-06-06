#!/usr/bin/env bash
# Smoke-check a running Houston Cloud control plane (VPS or local compose).
#
# Usage:
#   export HOUSTON_CLOUD_BASE=https://cloud.example.com
#   export HOUSTON_CLOUD_JWT=<bearer-token>   # HOUSTON_CLOUD_TOKEN or minted JWT
#   ./smoke.sh
#
# Defaults HOUSTON_CLOUD_BASE from HOUSTON_CLOUD_DOMAIN when set in .env.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ -f "${SCRIPT_DIR}/.env" ]]; then
  # shellcheck disable=SC1091
  set -a
  source "${SCRIPT_DIR}/.env"
  set +a
fi

BASE="${HOUSTON_CLOUD_BASE:-}"
if [[ -z "${BASE}" && -n "${HOUSTON_CLOUD_DOMAIN:-}" ]]; then
  BASE="https://${HOUSTON_CLOUD_DOMAIN}"
fi
if [[ -z "${BASE}" ]]; then
  echo "error: set HOUSTON_CLOUD_BASE or HOUSTON_CLOUD_DOMAIN" >&2
  exit 1
fi

BASE="${BASE%/}"
JWT="${HOUSTON_CLOUD_JWT:-${HOUSTON_CLOUD_TOKEN:-REPLACE_WITH_CLOUD_TOKEN}}"

echo "== GET ${BASE}/health =="
HEALTH="$(curl -fsS "${BASE}/health")"
echo "${HEALTH}"
if [[ "${HEALTH}" != "ok" ]]; then
  echo "error: expected health body 'ok', got: ${HEALTH}" >&2
  exit 1
fi

echo ""
echo "== GET ${BASE}/v1/cloud/me =="
if [[ "${JWT}" == "REPLACE_WITH_CLOUD_TOKEN" ]]; then
  echo "skip: set HOUSTON_CLOUD_JWT or HOUSTON_CLOUD_TOKEN to verify auth"
  exit 0
fi

ME="$(curl -fsS -H "Authorization: Bearer ${JWT}" "${BASE}/v1/cloud/me")"
echo "${ME}"
