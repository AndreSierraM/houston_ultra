#!/usr/bin/env bash
# E2E local: delete existing cloud agents, create 4 Store agents, verify pods + bootstrap.
# Requires: k3d-houston-local, control plane on :8788, houston/engine:dev in cluster.
#
# Usage:
#   export HOUSTON_CLOUD_TOKEN=hst_...   # same as profiles/local.env
#   ./cloud/k8s/scripts/e2e-four-agents.sh

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"

CLOUD_BASE="${HOUSTON_CLOUD_BASE:-http://127.0.0.1:8788}"
CLOUD_BASE="${CLOUD_BASE%/}"
ENGINE_PORT="${E2E_ENGINE_PORT:-59998}"
ENGINE_TOKEN="${E2E_ENGINE_TOKEN:-e2e-local-bootstrap-token}"
ENGINE_BIN="${E2E_ENGINE_BIN:-$ROOT/target/debug/houston-engine}"
E2E_HOME="${E2E_HOME:-/tmp/houston-e2e-$$}"
STORE_AGENTS=(bookkeeping operations sales support)
AGENT_NAMES=("E2E Bookkeeping" "E2E Operations" "E2E Sales" "E2E Support")

fail() { echo "FAIL: $*" >&2; exit 1; }
ok() { echo "OK: $*"; }

[[ -n "${HOUSTON_CLOUD_TOKEN:-}" ]] || fail "export HOUSTON_CLOUD_TOKEN"

auth_hdr=(-H "Authorization: Bearer ${HOUSTON_CLOUD_TOKEN}")

curl_cp() {
  /usr/bin/curl -sfS "${auth_hdr[@]}" "$@"
}

curl_cp_json() {
  /usr/bin/curl -sfS "${auth_hdr[@]}" -H "Content-Type: application/json" "$@"
}

echo "== preflight =="
curl_cp "${CLOUD_BASE}/health" | grep -q ok || fail "control plane health"
kubectl config current-context | grep -q k3d-houston-local || fail "kubectl context not k3d-houston-local"
ORG_ID="$(curl_cp "${CLOUD_BASE}/v1/cloud/me" | python3 -c 'import sys,json; print(json.load(sys.stdin)["orgId"])')"
ORG_NS="hou-org-${ORG_ID}"
echo "org namespace: ${ORG_NS}"

echo "== start temp local engine for bootstrap bundles =="
mkdir -p "${E2E_HOME}/.houston" "${E2E_HOME}/workspace"
HOUSTON_HOME="${E2E_HOME}/.houston" \
HOUSTON_DOCS="${E2E_HOME}/workspace" \
HOUSTON_BIND="127.0.0.1:${ENGINE_PORT}" \
HOUSTON_ENGINE_TOKEN="${ENGINE_TOKEN}" \
HOUSTON_TUNNEL_URL="http://127.0.0.1:1" \
HOUSTON_NO_PARENT_WATCHDOG=1 \
  "${ENGINE_BIN}" </dev/null >/tmp/houston-e2e-engine-$$.log 2>&1 &
ENGINE_PID=$!
cleanup() {
  kill "${ENGINE_PID}" 2>/dev/null || true
  wait "${ENGINE_PID}" 2>/dev/null || true
  rm -rf "${E2E_HOME}"
}
trap cleanup EXIT

for _ in $(seq 1 60); do
  if /usr/bin/curl -sf -H "Authorization: Bearer ${ENGINE_TOKEN}" "http://127.0.0.1:${ENGINE_PORT}/v1/health" >/dev/null 2>&1; then
    break
  fi
  sleep 0.5
done
/usr/bin/curl -sf -H "Authorization: Bearer ${ENGINE_TOKEN}" "http://127.0.0.1:${ENGINE_PORT}/v1/health" >/dev/null \
  || fail "temp engine did not start (see /tmp/houston-e2e-engine-$$.log)"

build_bundle() {
  local config_id="$1" name="$2"
  local installed="${ROOT}/store/agents/${config_id}"
  [[ -d "${installed}" ]] || fail "missing store agent ${installed}"
  /usr/bin/curl -sfS -H "Authorization: Bearer ${ENGINE_TOKEN}" -H "Content-Type: application/json" \
    -d "{\"configId\":\"${config_id}\",\"name\":\"${name}\",\"installedPath\":\"${installed}\",\"provider\":\"anthropic\",\"model\":\"claude-sonnet-4-6\"}" \
    "http://127.0.0.1:${ENGINE_PORT}/v1/agents/bootstrap-bundle"
}

echo "== delete existing cloud agents =="
existing="$(curl_cp "${CLOUD_BASE}/v1/cloud/agents" || echo '[]')"
count="$(echo "${existing}" | python3 -c 'import sys,json; print(len(json.load(sys.stdin)))')"
echo "found ${count} existing agent(s)"
while IFS= read -r id; do
  [[ -z "${id}" ]] && continue
  echo "deleting ${id}"
  /usr/bin/curl -sfS -X DELETE "${auth_hdr[@]}" "${CLOUD_BASE}/v1/cloud/agents/${id}" >/dev/null || true
  kubectl -n "${ORG_NS}" delete deploy "hou-cloud-agent-${id}" --ignore-not-found --wait=false 2>/dev/null || true
done < <(echo "${existing}" | python3 -c 'import sys,json; [print(a["id"]) for a in json.load(sys.stdin)]')

sleep 5

declare -a CREATED_IDS=()
declare -a CREATED_PATHS=()

echo "== create 4 store cloud agents =="
for i in "${!STORE_AGENTS[@]}"; do
  cid="${STORE_AGENTS[$i]}"
  name="${AGENT_NAMES[$i]}"
  echo "--- creating ${name} (${cid}) ---"
  bundle="$(build_bundle "${cid}" "${name}")"
  skill_count="$(echo "${bundle}" | python3 -c 'import sys,json; b=json.load(sys.stdin); print(len(b.get("skills") or []))')"
  seed_keys="$(echo "${bundle}" | python3 -c 'import sys,json; b=json.load(sys.stdin); s=b.get("seeds") or {}; print(len(s), "routines" in s)')" 
  echo "  bootstrap: skills=${skill_count} seeds=${seed_keys}"

  resp="$(curl_cp_json -d "{\"name\":\"${name}\",\"configId\":\"${cid}\",\"color\":\"navy\",\"bootstrapBundle\":${bundle}}" \
    "${CLOUD_BASE}/v1/cloud/agents")"
  aid="$(echo "${resp}" | python3 -c 'import sys,json; print(json.load(sys.stdin)["id"])')"
  fpath="$(echo "${resp}" | python3 -c 'import sys,json; print(json.load(sys.stdin)["folderPath"])')"
  CREATED_IDS+=("${aid}")
  CREATED_PATHS+=("${fpath}")
  ok "created ${name} id=${aid} path=${fpath}"
done

echo "== wait for 4 agent pods Running =="
for _ in $(seq 1 120); do
  running="$(kubectl -n "${ORG_NS}" get pods --no-headers 2>/dev/null | grep -c 'Running' || true)"
  [[ "${running}" -ge 4 ]] && break
  sleep 2
done
kubectl -n "${ORG_NS}" get pods -o wide
running="$(kubectl -n "${ORG_NS}" get pods --no-headers 2>/dev/null | grep -c 'Running' || true)"
[[ "${running}" -ge 4 ]] || fail "expected 4 Running pods, got ${running}"

echo "== verify bootstrap + proxy per agent =="
for i in "${!CREATED_IDS[@]}"; do
  aid="${CREATED_IDS[$i]}"
  fpath="${CREATED_PATHS[$i]}"
  cid="${STORE_AGENTS[$i]}"
  proxy="${CLOUD_BASE}/v1/cloud/agents/${aid}/proxy"

  health="$(curl_cp "${proxy}/v1/health")"
  echo "${health}" | grep -q '"status":"ok"' || fail "${aid} health: ${health}"

  claude="$(/usr/bin/curl -sfS -X POST "${auth_hdr[@]}" -H "Content-Type: application/json" \
    -d "{\"agent_path\":\"${fpath}\",\"rel_path\":\"CLAUDE.md\"}" \
    "${proxy}/v1/agents/files/read")"
  echo "${claude}" | python3 -c 'import sys,json; c=json.load(sys.stdin).get("content",""); sys.exit(0 if len(c)>50 else 1)' \
    || fail "${aid} CLAUDE.md empty"

  routines="$(/usr/bin/curl -sfS -X POST "${auth_hdr[@]}" -H "Content-Type: application/json" \
    -d "{\"agent_path\":\"${fpath}\",\"rel_path\":\".houston/routines/routines.json\"}" \
    "${proxy}/v1/agents/files/read")"
  rc="$(echo "${routines}" | python3 -c 'import sys,json; import json as J; c=json.load(sys.stdin).get("content","[]"); a=J.loads(c) if c.strip() else []; print(len(a))')"
  [[ "${rc}" -gt 0 ]] || fail "${aid} (${cid}) routines empty"

  skills="$(/usr/bin/curl -sfS -G "${auth_hdr[@]}" \
    --data-urlencode "workspacePath=${fpath}" \
    "${proxy}/v1/skills")"
  sc="$(echo "${skills}" | python3 -c 'import sys,json; print(len(json.load(sys.stdin)))')"
  [[ "${sc}" -gt 0 ]] || fail "${aid} (${cid}) skills empty (got ${sc})"

  activity="$(/usr/bin/curl -sfS -X POST "${auth_hdr[@]}" -H "Content-Type: application/json" \
    -d "{\"agent_path\":\"${fpath}\",\"rel_path\":\".houston/activity/activity.json\"}" \
    "${proxy}/v1/agents/files/read")"
  echo "${activity}" | grep -q '\[\]' || fail "${aid} activity not empty array"

  ok "${cid}: health+CLAUDE+routines(${rc})+skills(${sc})+activity[]"
done

echo "== lifecycle: pods stay up after idle (not request-scoped) =="
sleep 10
for aid in "${CREATED_IDS[@]}"; do
  phase="$(kubectl -n "${ORG_NS}" get pods -l "houston.ai/agent-id=${aid}" -o jsonpath='{.items[0].status.phase}' 2>/dev/null || echo Missing)"
  [[ "${phase}" == "Running" ]] || fail "pod ${aid} not Running after idle: ${phase}"
  restarts="$(kubectl -n "${ORG_NS}" get pods -l "houston.ai/agent-id=${aid}" -o jsonpath='{.items[0].status.containerStatuses[0].restartCount}' 2>/dev/null || echo ?)"
  ok "pod ${aid} phase=${phase} restarts=${restarts}"
done

echo "== burst proxy requests (pods must not terminate) =="
for aid in "${CREATED_IDS[@]}"; do
  for _ in 1 2 3; do
    curl_cp "${CLOUD_BASE}/v1/cloud/agents/${aid}/proxy/v1/health" >/dev/null
  done
done
sleep 3
running_after="$(kubectl -n "${ORG_NS}" get pods --no-headers 2>/dev/null | grep -c 'Running' || true)"
[[ "${running_after}" -ge 4 ]] || fail "pods dropped after burst: ${running_after} Running"

echo ""
echo "e2e-four-agents: ALL PASSED"
echo "agents: ${CREATED_IDS[*]}"
