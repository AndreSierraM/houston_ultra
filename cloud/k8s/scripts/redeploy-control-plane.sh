#!/usr/bin/env bash
# Rebuild + import control-plane only (~2 min with Docker cache). Full cluster: setup-local-k3d.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"
CLUSTER="${K3D_CLUSTER_NAME:-houston-local}"

echo "==> docker build houston/control-plane:dev"
docker build -t houston/control-plane:dev -f cloud/control-plane/Dockerfile.k8s .

echo "==> k3d import"
k3d image import houston/control-plane:dev -c "${CLUSTER}"

echo "==> rollout restart"
kubectl rollout restart deployment/houston-control-plane -n houston-system
kubectl rollout status deployment/houston-control-plane -n houston-system --timeout=120s

echo "redeploy control-plane: OK"
