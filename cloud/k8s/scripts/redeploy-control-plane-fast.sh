#!/usr/bin/env bash
# Fast control-plane rebuild: Linux cargo via Docker volume cache (~1-3 min after first run).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"
CLUSTER="${K3D_CLUSTER_NAME:-houston-local}"

echo "==> cargo build (linux, cached target/)"
docker run --rm \
  -v "${ROOT}:/src" \
  -w /src \
  rust:1-slim-bookworm \
  bash -c 'apt-get update -qq && apt-get install -y -qq pkg-config libssl-dev ca-certificates >/dev/null \
    && cargo build --release -p houston-cloud-control-plane'

echo "==> package slim image"
docker build -t houston/control-plane:dev -f cloud/control-plane/Dockerfile.k8s.fast .

echo "==> k3d import + rollout"
k3d image import houston/control-plane:dev -c "${CLUSTER}"
kubectl rollout restart deployment/houston-control-plane -n houston-system
kubectl rollout status deployment/houston-control-plane -n houston-system --timeout=120s
echo "redeploy-control-plane-fast: OK"
