#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"

docker build -t houston/engine:dev -f always-on/Dockerfile .
docker build -t houston/control-plane:dev -f cloud/control-plane/Dockerfile.k8s .

CLUSTER="${K3D_CLUSTER_NAME:-houston-local}"
if command -v k3d >/dev/null 2>&1 \
  && k3d cluster list 2>/dev/null | awk '{print $1}' | grep -qx "${CLUSTER}"; then
  k3d image import houston/engine:dev houston/control-plane:dev -c "${CLUSTER}"
  echo "images imported into k3d cluster ${CLUSTER}"
elif command -v k3s >/dev/null 2>&1; then
  docker save houston/engine:dev houston/control-plane:dev | sudo k3s ctr images import -
  echo "images imported into k3s"
else
  echo "no k3d cluster '${CLUSTER}' (K3D_CLUSTER_NAME) or k3s — load images manually"
fi
