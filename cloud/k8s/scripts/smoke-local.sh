#!/usr/bin/env bash
# Smoke-check Houston Cloud control plane en cluster k3d local (Mac).
#
# Uso:
#   export HOUSTON_CLOUD_TOKEN=hst_...
#   ./cloud/k8s/scripts/smoke-local.sh
#
# Variables opcionales:
#   KUBECTL_CONTEXT   default: k3d-houston-local
#   HOUSTON_CLOUD_BASE  default: http://127.0.0.1:8788

set -euo pipefail

CONTEXT="${KUBECTL_CONTEXT:-k3d-houston-local}"
NS="${KUBECTL_NAMESPACE:-houston-system}"
DEPLOY="${CONTROL_PLANE_DEPLOY:-houston-control-plane}"
BASE="${HOUSTON_CLOUD_BASE:-http://127.0.0.1:8788}"
BASE="${BASE%/}"

fail() {
  echo "error: $*" >&2
  exit 1
}

echo "== kubectl context =="
if ! kubectl config get-contexts -o name 2>/dev/null | grep -qx "${CONTEXT}"; then
  fail "contexto '${CONTEXT}' no existe; ejecuta setup-local-k3d.sh primero"
fi

CURRENT="$(kubectl config current-context 2>/dev/null || true)"
if [[ "${CURRENT}" != "${CONTEXT}" ]]; then
  echo "cambiando contexto: ${CURRENT:-<ninguno>} -> ${CONTEXT}"
  kubectl config use-context "${CONTEXT}" >/dev/null
fi
echo "ok: contexto ${CONTEXT}"

echo ""
echo "== nodos Ready =="
kubectl wait --for=condition=Ready node --all --timeout=30s >/dev/null
echo "ok: nodos Ready"

echo ""
echo "== kube-system pods =="
BAD="$(kubectl -n kube-system get pods --no-headers 2>/dev/null \
  | awk '$3 != "Running" && $3 != "Completed" {print}' || true)"
if [[ -n "${BAD}" ]]; then
  echo "${BAD}" >&2
  fail "pods en kube-system no Running/Completed; CNI o storage del cluster no listo"
fi
echo "ok: kube-system pods Running/Completed"

echo ""
echo "== pod control-plane =="
if ! kubectl -n "${NS}" get deployment "${DEPLOY}" >/dev/null 2>&1; then
  fail "deployment/${DEPLOY} no encontrado en namespace ${NS}"
fi

kubectl -n "${NS}" rollout status "deployment/${DEPLOY}" --timeout=120s >/dev/null

PHASE="$(kubectl -n "${NS}" get pods -l app=houston-control-plane \
  -o jsonpath='{.items[0].status.phase}' 2>/dev/null || true)"
READY="$(kubectl -n "${NS}" get pods -l app=houston-control-plane \
  -o jsonpath='{.items[0].status.conditions[?(@.type=="Ready")].status}' 2>/dev/null || true)"

if [[ "${PHASE}" != "Running" || "${READY}" != "True" ]]; then
  kubectl -n "${NS}" get pods -l app=houston-control-plane -o wide >&2 || true
  fail "pod control-plane no Running/Ready (phase=${PHASE:-?}, ready=${READY:-?})"
fi
echo "ok: pod control-plane Running"

echo ""
echo "== GET ${BASE}/health =="
HEALTH="$(curl -fsS "${BASE}/health")"
echo "${HEALTH}"
[[ "${HEALTH}" == "ok" ]] || fail "esperaba 'ok', obtuvo: ${HEALTH}"

echo ""
echo "== GET ${BASE}/v1/cloud/me =="
[[ -n "${HOUSTON_CLOUD_TOKEN:-}" ]] || fail "exporta HOUSTON_CLOUD_TOKEN (mismo valor que en el Secret del cluster)"

ME="$(curl -fsS -H "Authorization: Bearer ${HOUSTON_CLOUD_TOKEN}" "${BASE}/v1/cloud/me")"
echo "${ME}"

echo ""
echo "== GET ${BASE}/v1/cloud/entitlements =="
ENT="$(curl -fsS -H "Authorization: Bearer ${HOUSTON_CLOUD_TOKEN}" "${BASE}/v1/cloud/entitlements")"
echo "${ENT}"
echo "${ENT}" | grep -q '"status":"active"' || fail "entitlements.status must be active for local QA"
echo "${ENT}" | grep -q '"maxCloudAgents"' || fail "entitlements.maxCloudAgents missing"

echo ""
echo "smoke local: OK"
