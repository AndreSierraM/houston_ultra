#!/usr/bin/env bash
set -euo pipefail

ENV_FILE="${1:-cloud/control-plane/deploy/profiles/cloudhouston.blyxlabs.dev.env}"
NS="${2:-houston-system}"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "missing env file: $ENV_FILE" >&2
  exit 1
fi

if [[ "$ENV_FILE" == *.example ]]; then
  echo "refusing .env.example — copy to profiles/cloudhouston.blyxlabs.dev.env and fill secrets first" >&2
  exit 1
fi

TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT

grep -v '^VITE_' "$ENV_FILE" | grep -v '^#' | grep -v '^$' >"$TMP"

kubectl create secret generic houston-control-plane-env \
  --namespace="$NS" \
  --from-env-file="$TMP" \
  --dry-run=client -o yaml | kubectl apply -f -

echo "secret houston-control-plane-env applied in $NS"
