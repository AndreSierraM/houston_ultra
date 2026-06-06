#!/usr/bin/env bash
# Apaga y elimina TODOS los agentes cloud: control plane (sidebar app) + cluster K8s.
#
# La app no guarda agentes cloud en disco local; la lista viene de GET /v1/cloud/agents.
# DELETE en control plane = desaparecen de la app al recargar workspace.
#
# Uso (local k3d):
#   ./cloud/k8s/scripts/teardown-cloud-agents.sh --profile app/.env.local --yes
#
# Uso (VPS):
#   ./cloud/k8s/scripts/teardown-cloud-agents.sh \
#     --profile cloud/control-plane/deploy/profiles/cloudhouston.blyxlabs.dev.env \
#     --insecure --yes
#
# Flags:
#   --profile PATH    Carga token/base del perfil (.env)
#   --dry-run         Solo lista
#   --yes             Sin confirmación
#   --skip-kubectl    Solo API (no recomendado)
#   --skip-app        No limpiar last_agent_id en engine local
#   --insecure        curl -k (cert TLS del VPS)

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"

DRY_RUN=0
ASSUME_YES=0
SKIP_KUBECTL=0
SKIP_APP=0
PROFILE=""
CURL_EXTRA=()
KUBECTL_CONTEXT="${KUBECTL_CONTEXT:-}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile) PROFILE="${2:-}"; shift 2 ;;
    --dry-run) DRY_RUN=1; shift ;;
    --yes) ASSUME_YES=1; shift ;;
    --skip-kubectl) SKIP_KUBECTL=1; shift ;;
    --skip-app) SKIP_APP=1; shift ;;
    --insecure) CURL_EXTRA+=(-k); shift ;;
    -h|--help) sed -n '2,22p' "$0"; exit 0 ;;
    *) echo "argumento desconocido: $1" >&2; exit 1 ;;
  esac
done

load_profile() {
  local file="$1"
  [[ -f "$file" ]] || return 1
  set -a
  # shellcheck disable=SC1090
  source "$file"
  set +a
}

if [[ -n "$PROFILE" ]]; then
  load_profile "$PROFILE" || fail "no se pudo cargar perfil $PROFILE"
fi

CLOUD_BASE="${HOUSTON_CLOUD_BASE:-${VITE_HOUSTON_CLOUD_BASE:-http://127.0.0.1:8788}}"
CLOUD_BASE="${CLOUD_BASE%/}"
TOKEN="${HOUSTON_CLOUD_TOKEN:-${VITE_HOUSTON_CLOUD_TOKEN:-}}"

fail() { echo "error: $*" >&2; exit 1; }
info() { echo "$*"; }

[[ -n "$TOKEN" ]] || fail "falta HOUSTON_CLOUD_TOKEN (export o --profile)"

auth_hdr=(-H "Authorization: Bearer ${TOKEN}")

curl_cp() {
  if [[ ${#CURL_EXTRA[@]} -gt 0 ]]; then
    curl "${CURL_EXTRA[@]}" -sfS "${auth_hdr[@]}" "$@"
  else
    curl -sfS "${auth_hdr[@]}" "$@"
  fi
}

curl_post() {
  if [[ ${#CURL_EXTRA[@]} -gt 0 ]]; then
    curl "${CURL_EXTRA[@]}" -sfS -X POST "${auth_hdr[@]}" "$@"
  else
    curl -sfS -X POST "${auth_hdr[@]}" "$@"
  fi
}

curl_delete() {
  local url="$1"
  local code
  if [[ ${#CURL_EXTRA[@]} -gt 0 ]]; then
    code="$(curl "${CURL_EXTRA[@]}" -sS --max-time 45 -o /dev/null -w '%{http_code}' -X DELETE "${auth_hdr[@]}" "$url" || true)"
  else
    code="$(curl -sS --max-time 45 -o /dev/null -w '%{http_code}' -X DELETE "${auth_hdr[@]}" "$url" || true)"
  fi
  case "$code" in
    200|204) return 0 ;;
    404) info "  aviso: ya no existe en control plane (404)"; return 0 ;;
    *) fail "DELETE API falló (${code}) para ${url}" ;;
  esac
}

kubectl_cmd() {
  if [[ -n "$KUBECTL_CONTEXT" ]]; then
    kubectl --context="$KUBECTL_CONTEXT" "$@"
  else
    kubectl "$@"
  fi
}

cluster_delete_agent_resources() {
  local ns="$1"
  local agent_id="$2"
  local deploy="hou-cloud-agent-${agent_id}"
  local pvc="${deploy}-home"
  local secret="${deploy}-token"
  info "  cluster: borrando ${deploy}"
  kubectl_cmd -n "$ns" delete deployment,service "$deploy" \
    --ignore-not-found --wait=false >/dev/null 2>&1 || true
  kubectl_cmd -n "$ns" delete pvc "$pvc" secret "$secret" \
    --ignore-not-found --wait=false >/dev/null 2>&1 || true
  kubectl_cmd -n "$ns" delete pod -l "houston.ai/agent-id=${agent_id}" \
    --ignore-not-found --grace-period=0 --force >/dev/null 2>&1 || true
}

cluster_sweep_org() {
  local ns="$1"
  info "cluster: barrido final en ${ns}"
  kubectl_cmd -n "$ns" delete deployment,service,secret,pvc \
    -l 'houston.ai/agent-id' --ignore-not-found --wait=false >/dev/null 2>&1 || true
  while IFS= read -r name; do
    [[ -z "$name" ]] && continue
    kubectl_cmd -n "$ns" delete "$name" --ignore-not-found --wait=false >/dev/null 2>&1 || true
  done < <(
    kubectl_cmd -n "$ns" get deploy,service,secret,pvc -o name 2>/dev/null \
      | grep 'hou-cloud-agent-' || true
  )
  while IFS= read -r pod; do
    [[ -z "$pod" ]] && continue
    kubectl_cmd -n "$ns" delete "$pod" --ignore-not-found --grace-period=0 --force >/dev/null 2>&1 || true
  done < <(
    kubectl_cmd -n "$ns" get pods -o name 2>/dev/null | grep 'hou-cloud-agent-' || true
  )
}

count_cluster_agents() {
  local ns="$1"
  kubectl_cmd -n "$ns" get deploy,service,secret,pvc,pod -o name 2>/dev/null \
    | grep -c 'hou-cloud-agent-' || true
}

app_clear_cloud_prefs() {
  local deleted_ids_csv="$1"
  info "app: limpiando preferencias locales"
  ENGINE_MANIFEST="${HOUSTON_HOME:-$HOME/.houston}/engine.json"
  [[ -f "$ENGINE_MANIFEST" ]] || {
    info "  sin engine.json; recarga la app (Cmd+R) para refrescar sidebar"
    return 0
  }
  read -r ENGINE_BASE ENGINE_TOKEN < <(
    python3 - <<'PY' "$ENGINE_MANIFEST"
import json, sys
with open(sys.argv[1]) as f:
    m = json.load(f)
print(m.get("baseUrl", ""), m.get("token", ""))
PY
  )
  [[ -n "$ENGINE_BASE" && -n "$ENGINE_TOKEN" ]] || {
    info "  engine.json incompleto; recarga la app"
    return 0
  }
  LAST_ID="$(curl -sfS -H "Authorization: Bearer ${ENGINE_TOKEN}" \
    "${ENGINE_BASE%/}/v1/preferences/last_agent_id" \
    | python3 -c 'import sys,json; print(json.load(sys.stdin).get("value") or "")' 2>/dev/null || true)"
  if [[ -n "$LAST_ID" ]]; then
    case ",${deleted_ids_csv}," in
      *,"${LAST_ID}",*)
        info "  limpiando last_agent_id=${LAST_ID}"
        curl -sfS -X PUT -H "Authorization: Bearer ${ENGINE_TOKEN}" \
          -H "Content-Type: application/json" -d '{"value":""}' \
          "${ENGINE_BASE%/}/v1/preferences/last_agent_id" >/dev/null 2>&1 \
          || info "  aviso: engine local no respondió (¿tauri dev apagado?)"
        ;;
      *)
        info "  last_agent_id=${LAST_ID} (agente local, se conserva)"
        ;;
    esac
  fi
  info "  recarga Houston desktop o cambia workspace para vaciar sidebar cloud"
}

info "== preflight =="
info "control plane (app): ${CLOUD_BASE}"
if ! curl_cp "${CLOUD_BASE}/health" 2>/dev/null | grep -q ok; then
  curl_cp "${CLOUD_BASE}/v1/cloud/me" >/dev/null \
    || fail "control plane no responde"
fi

ME_JSON="$(curl_cp "${CLOUD_BASE}/v1/cloud/me")"
ORG_ID="$(python3 -c 'import sys,json; print(json.load(sys.stdin)["orgId"])' <<<"$ME_JSON")"
ORG_NS="hou-org-${ORG_ID}"
info "org: ${ORG_ID}"
info "namespace cluster: ${ORG_NS}"

AGENTS_JSON="$(curl_cp "${CLOUD_BASE}/v1/cloud/agents" || echo '[]')"
AGENT_IDS=()
AGENT_NAMES=()
while IFS=$'\t' read -r agent_id agent_name; do
  [[ -z "$agent_id" ]] && continue
  AGENT_IDS+=("$agent_id")
  AGENT_NAMES+=("$agent_name")
done < <(
  python3 -c 'import sys,json
for a in json.load(sys.stdin):
    print(a["id"] + "\t" + a.get("name", ""))' <<<"$AGENTS_JSON"
)

COUNT="${#AGENT_IDS[@]}"
CLUSTER_BEFORE=0
if [[ "$SKIP_KUBECTL" -eq 0 ]] && command -v kubectl >/dev/null 2>&1 \
  && kubectl_cmd get namespace "${ORG_NS}" >/dev/null 2>&1; then
  CLUSTER_BEFORE="$(count_cluster_agents "${ORG_NS}")"
fi

info "agentes en control plane (app): ${COUNT}"
info "recursos hou-cloud-agent-* en cluster: ${CLUSTER_BEFORE}"

if [[ "$COUNT" -eq 0 && "$CLUSTER_BEFORE" -eq 0 ]]; then
  info "nada que eliminar"
  exit 0
fi

for i in "${!AGENT_IDS[@]}"; do
  info "  - ${AGENT_NAMES[$i]:-<sin nombre>} (${AGENT_IDS[$i]})"
done

if [[ "$DRY_RUN" -eq 1 ]]; then
  info "dry-run: sin cambios"
  exit 0
fi

if [[ "$ASSUME_YES" -ne 1 ]]; then
  echo ""
  read -r -p "Eliminar ${COUNT} agente(s) de app + cluster en ${CLOUD_BASE}? [y/N] " ans
  case "$ans" in
    [yY]|[yY][eE][sS]) ;;
    *) info "cancelado"; exit 0 ;;
  esac
fi

deleted_ids=()

info ""
info "== fase 1: cluster K8s (apagar pods y borrar recursos) =="
if [[ "$SKIP_KUBECTL" -eq 0 ]] && command -v kubectl >/dev/null 2>&1 \
  && kubectl_cmd get namespace "${ORG_NS}" >/dev/null 2>&1; then
  for id in "${AGENT_IDS[@]:-}"; do
    cluster_delete_agent_resources "${ORG_NS}" "$id"
  done
  cluster_sweep_org "${ORG_NS}"
else
  info "kubectl omitido (--skip-kubectl, sin kubectl, o namespace ausente)"
fi

info ""
info "== fase 2: control plane (app / DB) =="
for i in "${!AGENT_IDS[@]}"; do
  id="${AGENT_IDS[$i]}"
  name="${AGENT_NAMES[$i]:-$id}"
  info "${name} (${id})"
  curl_post "${CLOUD_BASE}/v1/cloud/agents/${id}/stop" >/dev/null 2>&1 || true
  curl_delete "${CLOUD_BASE}/v1/cloud/agents/${id}"
  deleted_ids+=("$id")
done

info ""
info "== fase 3: cluster (barrido final) =="
if [[ "$SKIP_KUBECTL" -eq 0 ]] && command -v kubectl >/dev/null 2>&1 \
  && kubectl_cmd get namespace "${ORG_NS}" >/dev/null 2>&1; then
  cluster_sweep_org "${ORG_NS}"
fi

DELETED_CSV="$(IFS=,; echo "${deleted_ids[*]:-}")"

if [[ "$SKIP_APP" -eq 0 && ${#deleted_ids[@]} -gt 0 ]]; then
  info ""
  info "== fase 4: app local =="
  app_clear_cloud_prefs "$DELETED_CSV"
fi

info ""
info "== verificación =="
REMAINING_API="$(curl_cp "${CLOUD_BASE}/v1/cloud/agents" | python3 -c 'import sys,json; print(len(json.load(sys.stdin)))')"
CLUSTER_AFTER=0
if [[ "$SKIP_KUBECTL" -eq 0 ]] && kubectl_cmd get namespace "${ORG_NS}" >/dev/null 2>&1; then
  CLUSTER_AFTER="$(count_cluster_agents "${ORG_NS}")"
fi
info "agentes restantes en app (API): ${REMAINING_API}"
info "recursos restantes en cluster: ${CLUSTER_AFTER}"

if [[ "$REMAINING_API" -ne 0 || "$CLUSTER_AFTER" -ne 0 ]]; then
  fail "limpieza incompleta (app=${REMAINING_API}, cluster=${CLUSTER_AFTER})"
fi

info ""
info "listo: app y cluster vacíos (${#deleted_ids[@]} agente(s) eliminados)"
