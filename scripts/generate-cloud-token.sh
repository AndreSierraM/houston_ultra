#!/usr/bin/env bash
# Emit a shared bearer token for local Houston Cloud auth (control plane + app).
set -euo pipefail
bytes="$(openssl rand -hex 32)"
token="hst_${bytes}"
echo "HOUSTON_CLOUD_TOKEN=${token}"
echo "VITE_HOUSTON_CLOUD_TOKEN=${token}"
