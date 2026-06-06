#!/bin/bash
set -euo pipefail

# Immutable CLIs live under /opt/houston (image). HOME=/data is the PVC:
# provider credentials, workspace, and engine state.

mkdir -p /data/.houston /data/workspace \
  /data/.local/bin /data/.composio \
  /data/.codex /data/.claude /data/.gemini

link_if_missing() {
  local target="$1" link="$2"
  if [[ ! -e "$link" && -e "$target" ]]; then
    ln -sf "$target" "$link"
  fi
}

link_if_missing /opt/houston/.local/bin/claude /data/.local/bin/claude
link_if_missing /opt/houston/.local/bin/codex /data/.local/bin/codex
link_if_missing /opt/houston/.composio/composio /data/.composio/composio

chmod 700 /data 2>/dev/null || true

exec /usr/local/bin/houston-engine "$@"
