#!/usr/bin/env bash
# Crea profiles/local.env desde example + token nuevo si no existe.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
EXAMPLE="$ROOT/cloud/control-plane/deploy/profiles/local.env.example"
TARGET="$ROOT/cloud/control-plane/deploy/profiles/local.env"

if [[ -f "$TARGET" ]]; then
  echo "ya existe: $TARGET"
  exit 0
fi

cp "$EXAMPLE" "$TARGET"
"$ROOT/scripts/generate-cloud-token.sh"
echo "creado $TARGET — revisa DATABASE_URL si cambiaste password postgres"
