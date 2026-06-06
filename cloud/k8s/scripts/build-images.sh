#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"

docker build -t houston/engine:dev -f always-on/Dockerfile .
docker build -t houston/control-plane:dev -f cloud/control-plane/Dockerfile.k8s .

if command -v k3s >/dev/null 2>&1; then
  docker save houston/engine:dev houston/control-plane:dev | sudo k3s ctr images import -
  echo "images imported into k3s"
else
  echo "k3s not found — load images into your cluster registry or use 'ctr images import'"
fi
