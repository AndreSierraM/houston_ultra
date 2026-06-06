#!/bin/bash
set -euo pipefail

# Immutable CLIs live under /opt/houston (image). HOME=/data is the PVC:
# provider credentials, workspace, and engine state.
# Fresh Docker/K8s volume mounts hide the image /data layer and arrive
# root-owned — chown once as root, then drop to houston.

if [[ "$(id -u)" -eq 0 ]]; then
  # K8s fsGroup + OnRootMismatch fix volume root ownership. A recursive chown
  # on a populated PVC can exceed pod rollout timeout — only repair top-level paths.
  mkdir -p /data/.houston /data/workspace /data/.local /data/.codex /data/.claude
  houston_uid="$(id -u houston)"
  if [[ "$(stat -c '%u' /data 2>/dev/null || echo 0)" != "${houston_uid}" ]]; then
    chown houston:houston /data
  fi
  for dir in .houston workspace .local .codex .claude; do
    if [[ -e "/data/${dir}" ]]; then
      owner="$(stat -c '%u' "/data/${dir}" 2>/dev/null || echo 0)"
      if [[ "${owner}" != "${houston_uid}" ]]; then
        chown -R houston:houston "/data/${dir}"
      fi
    fi
  done
  exec gosu houston "$0" "$@"
fi

mkdir -p /data/.houston /data/workspace \
  /data/.local/bin /data/.local/share \
  /data/.codex /data/.claude

link_if_missing() {
  local target="$1" link="$2"
  if [[ ! -e "$link" && -e "$target" ]]; then
    ln -sf "$target" "$link"
  fi
}

link_if_missing /opt/houston/.local/bin/claude /data/.local/bin/claude
link_if_missing /opt/houston/.local/bin/codex /data/.local/bin/codex
# Claude resolves its version tree from ~/.local/share/claude; without this
# symlink it writes next to the real binary under /opt/houston (read-only).
link_if_missing /opt/houston/.local/share/claude /data/.local/share/claude

chmod 700 /data 2>/dev/null || true

if [[ -z "${HOUSTON_APP_SYSTEM_PROMPT:-}" && -s /opt/houston/product-prompt.txt ]]; then
  export HOUSTON_APP_SYSTEM_PROMPT="$(cat /opt/houston/product-prompt.txt)"
fi

exec /usr/local/bin/houston-engine "$@"
