#!/usr/bin/env bash
# Token compartido control-plane + app desktop (local).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
bytes="$(openssl rand -hex 32)"
token="hst_${bytes}"

upsert_env() {
  local file="$1"
  [[ -f "$file" ]] || return 0
  local tmp
  tmp="$(mktemp)"
  if grep -q '^HOUSTON_CLOUD_TOKEN=' "$file" 2>/dev/null; then
    sed "s/^HOUSTON_CLOUD_TOKEN=.*/HOUSTON_CLOUD_TOKEN=${token}/" "$file" >"$tmp"
  else
    cat "$file" >"$tmp"
    echo "HOUSTON_CLOUD_TOKEN=${token}" >>"$tmp"
  fi
  if grep -q '^VITE_HOUSTON_CLOUD_TOKEN=' "$tmp" 2>/dev/null; then
    sed -i '' "s/^VITE_HOUSTON_CLOUD_TOKEN=.*/VITE_HOUSTON_CLOUD_TOKEN=${token}/" "$tmp"
  fi
  mv "$tmp" "$file"
  echo "actualizado: $file"
}

upsert_env "$ROOT/cloud/control-plane/deploy/.env"
upsert_env "$ROOT/cloud/control-plane/deploy/profiles/local.env"

cat <<EOF

# Pegar en app/.env.local:
VITE_HOUSTON_CLOUD_BASE=http://127.0.0.1:8788
VITE_HOUSTON_CLOUD_TOKEN=${token}

# Exportar para smoke:
export HOUSTON_CLOUD_TOKEN=${token}
EOF
