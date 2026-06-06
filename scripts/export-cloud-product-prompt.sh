#!/usr/bin/env bash
# Export Houston product system prompt for cloud engine images.
#
# Writes always-on/houston-product-prompt.txt. The always-on Dockerfile copies
# it to /opt/houston/product-prompt.txt when present; docker-entrypoint.sh
# sets HOUSTON_APP_SYSTEM_PROMPT from that file when the env var is unset.
#
# Docker build succeeds without this file — cloud pods just omit the product
# voice until you run this script and rebuild the image.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${ROOT}/always-on/houston-product-prompt.txt"

mkdir -p "$(dirname "$OUT")"
cd "${ROOT}/app/src-tauri"

EXPORT_CLOUD_PROMPT=1 cargo test -p houston-app --lib \
  houston_prompt::tests::export_product_prompt_for_cloud \
  -- --exact --nocapture > "${OUT}"

bytes="$(wc -c < "${OUT}" | tr -d ' ')"
echo "Wrote ${OUT} (${bytes} bytes)"
echo "Rebuild: docker build -t houston/engine:dev -f always-on/Dockerfile ."
