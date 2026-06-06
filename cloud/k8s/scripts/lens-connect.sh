#!/usr/bin/env bash
# Deja el cluster k3d en ~/.kube/config y abre Lens (Mac).
set -euo pipefail

CLUSTER="${K3D_CLUSTER_NAME:-houston-local}"
CONTEXT="k3d-${CLUSTER}"
KUBECONFIG="${KUBECONFIG:-$HOME/.kube/config}"

if ! k3d cluster list 2>/dev/null | awk '{print $1}' | grep -qx "${CLUSTER}"; then
  echo "error: cluster k3d '${CLUSTER}' no existe. Ejecuta setup-local-k3d.sh primero." >&2
  exit 1
fi

mkdir -p "$(dirname "$KUBECONFIG")"
k3d kubeconfig merge "${CLUSTER}" --kubeconfig-merge-default --kubeconfig-switch-context

if kubectl config get-contexts -o name | grep -qx "${CONTEXT}"; then
  kubectl config use-context "${CONTEXT}" >/dev/null
  echo "contexto activo: ${CONTEXT}"
  echo "kubeconfig: ${KUBECONFIG}"
else
  echo "error: contexto ${CONTEXT} no encontrado tras merge" >&2
  exit 1
fi

if [[ "$(uname)" == "Darwin" ]] && [[ -d "/Applications/Lens.app" ]]; then
  open -a Lens
  echo "Lens abierto. Catalog → busca '${CONTEXT}' o Sync kubeconfig."
else
  echo "Lens no encontrado en /Applications/Lens.app"
  echo "Importa manualmente: kubeconfig ${KUBECONFIG}, contexto ${CONTEXT}"
fi
