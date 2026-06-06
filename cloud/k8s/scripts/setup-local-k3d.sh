#!/usr/bin/env bash
# Cluster k3d local para QA Houston Cloud en Mac.
#
# Uso:
#   cp cloud/control-plane/deploy/profiles/local.env.example \
#      cloud/control-plane/deploy/profiles/local.env
#   # Editar local.env: DATABASE_URL y HOUSTON_CLOUD_TOKEN (fuera de git)
#   ./cloud/k8s/scripts/setup-local-k3d.sh
#
# Argumento opcional: ruta al perfil env (default profiles/local.env)

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "${ROOT}"

CLUSTER="${K3D_CLUSTER_NAME:-houston-local}"
CONTEXT="k3d-${CLUSTER}"
NS="houston-system"
ENV_FILE="${1:-cloud/control-plane/deploy/profiles/local.env}"
K3S_IMAGE="${K3S_IMAGE:-rancher/k3s:v1.31.14-k3s1}"

need() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "error: falta '$1' en PATH" >&2
    exit 1
  }
}

wait_cluster_healthy() {
  echo ""
  echo "== esperando nodos Ready =="
  kubectl wait --for=condition=Ready node --all --timeout=300s

  echo ""
  echo "== esperando CNI (flannel subnet.env) =="
  local max=60 i=0
  local containers pending c
  while [[ $i -lt $max ]]; do
    containers="$(docker ps --filter "label=k3d.cluster=${CLUSTER}" --format '{{.Names}}' | sed '/^$/d')"
    if [[ -z "${containers}" ]]; then
      echo "error: no hay contenedores k3d para cluster '${CLUSTER}'" >&2
      exit 1
    fi
    pending=""
    while IFS= read -r c; do
      [[ -z "${c}" ]] && continue
      if ! docker exec "${c}" test -f /run/flannel/subnet.env 2>/dev/null; then
        pending="${pending}${c} "
      fi
    done <<< "${containers}"
    if [[ -z "${pending// /}" ]]; then
      local node_count
      node_count="$(printf '%s\n' "${containers}" | sed '/^$/d' | wc -l | tr -d ' ')"
      echo "flannel subnet.env: OK (${node_count} nodo(s))"
      break
    fi
    sleep 2
    i=$((i + 1))
  done
  if [[ -n "${pending// /}" ]]; then
    echo "error: timeout (${max} x 2s) esperando /run/flannel/subnet.env en: ${pending}" >&2
    exit 1
  fi

  echo ""
  echo "== esperando kube-system =="
  kubectl -n kube-system rollout status deployment/coredns --timeout=120s
  kubectl -n kube-system rollout status deployment/local-path-provisioner --timeout=120s
}

need docker
need k3d
need kubectl

if [[ ! -f "${ENV_FILE}" ]]; then
  echo "error: perfil no encontrado: ${ENV_FILE}" >&2
  echo "copia local.env.example -> local.env y rellena DATABASE_URL + token" >&2
  exit 1
fi

if [[ "${ENV_FILE}" == *.example ]]; then
  echo "error: no uses .env.example; copia a local.env y rellena credenciales" >&2
  exit 1
fi

echo "== k3d cluster '${CLUSTER}' =="
if k3d cluster list 2>/dev/null | awk '{print $1}' | grep -qx "${CLUSTER}"; then
  echo "cluster ya existe"
else
  k3d cluster create "${CLUSTER}" \
    --port "8788:80@loadbalancer" \
    --image "${K3S_IMAGE}" \
    --wait --timeout 120s
fi

kubectl config use-context "${CONTEXT}" >/dev/null
echo "contexto: ${CONTEXT}"

wait_cluster_healthy

echo ""
echo "== build imágenes =="
docker build -t houston/engine:dev -f always-on/Dockerfile .
docker build -t houston/control-plane:dev -f cloud/control-plane/Dockerfile.k8s .

echo ""
echo "== import imágenes a k3d =="
k3d image import houston/engine:dev houston/control-plane:dev -c "${CLUSTER}"

echo ""
echo "== manifests + secret =="
kubectl apply -f cloud/k8s/base/namespace.yaml

chmod +x cloud/k8s/scripts/create-control-plane-secret.sh
./cloud/k8s/scripts/create-control-plane-secret.sh "${ENV_FILE}" "${NS}"

kubectl apply -k cloud/k8s/overlays/local

echo ""
echo "== esperando rollout =="
kubectl -n "${NS}" rollout status deployment/postgres --timeout=180s
kubectl -n "${NS}" wait --for=condition=Bound pvc/houston-postgres-data --timeout=120s
kubectl -n "${NS}" rollout status deployment/houston-control-plane --timeout=300s

echo ""
echo "setup local k3d: OK"
echo "siguiente:"
echo "  export HOUSTON_CLOUD_TOKEN=<valor de ${ENV_FILE}>"
echo "  ./cloud/k8s/scripts/smoke-local.sh"
