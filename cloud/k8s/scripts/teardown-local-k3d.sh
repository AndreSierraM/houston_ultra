#!/usr/bin/env bash
set -euo pipefail

CLUSTER="${K3D_CLUSTER_NAME:-houston-local}"

if k3d cluster list 2>/dev/null | awk '{print $1}' | grep -qx "${CLUSTER}"; then
  k3d cluster delete "${CLUSTER}"
  echo "cluster ${CLUSTER} eliminado"
else
  echo "cluster ${CLUSTER} no existe"
fi

echo ""
echo "== post-teardown check =="
WARN_DOCKER=false

while IFS= read -r container; do
  [[ -z "${container}" ]] && continue
  WARN_DOCKER=true
  if ! docker exec "${container}" test -f /run/flannel/subnet.env 2>/dev/null; then
    echo "aviso: ${container} sin /run/flannel/subnet.env (CNI incompleto o contenedor huérfano)"
  fi
done < <(docker ps -a --format '{{.Names}}' 2>/dev/null | grep "^k3d-${CLUSTER}-" || true)

CONTEXT="k3d-${CLUSTER}"
if kubectl config get-contexts -o name 2>/dev/null | grep -qx "${CONTEXT}"; then
  while IFS= read -r line; do
    [[ -z "${line}" ]] && continue
    name="${line%%$'\t'*}"
    cap="${line#*$'\t'}"
    if [[ -z "${cap}" || "${cap}" == "0" ]]; then
      echo "aviso: nodo ${name} reporta ephemeral-storage capacity ${cap:-0}"
      WARN_DOCKER=true
    fi
  done < <(kubectl --context "${CONTEXT}" get nodes \
    -o jsonpath='{range .items[*]}{.metadata.name}{"\t"}{.status.capacity.ephemeral-storage}{"\n"}{end}' \
    2>/dev/null || true)
fi

if [[ "${WARN_DOCKER}" == true ]]; then
  echo ""
  echo "antes de volver a ejecutar setup-local-k3d.sh:"
  echo "  - revisar disco en Docker Desktop (Settings → Resources)"
  echo "  - docker system df"
  echo "  - docker system prune   # libera capas/containers huérfanos"
fi
