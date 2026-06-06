# Houston Engine — standalone deployment image.
#
# Produces a self-contained image that runs `houston-engine` and ships
# the provider CLIs cloud agents need (Claude Code, Codex).
# Typical deployment: VPS (Houston Always On) or Kubernetes,
# behind a reverse proxy with TLS.
#
# Provider CLIs install under /opt/houston (immutable). Persistent agent
# state mounts at /data (HOME): .houston, workspace, provider homes.
#
#   docker build -t houston/engine:dev -f always-on/Dockerfile .
#   docker run -p 7777:7777 \
#     -e HOUSTON_BIND_ALL=1 \
#     -e HOUSTON_BIND=0.0.0.0:7777 \
#     -e HOUSTON_ENGINE_TOKEN=<your-token> \
#     -v houston-data:/data \
#     houston/engine:dev

# ---------- Build stage (Rust engine) ----------
FROM rust:1-slim-bookworm AS build
WORKDIR /src

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates \
 && rm -rf /var/lib/apt/lists/*

# Cache the dependency compile by copying the manifests first.
COPY Cargo.toml Cargo.lock ./
COPY engine/ engine/
# Workspace lists `app/houston-tauri` and `app/src-tauri` as members, so their
# manifests must exist for cargo to load the workspace — even though `-p
# houston-engine-server` will not compile them (no path in its dep tree).
COPY app/houston-tauri/ app/houston-tauri/
COPY app/src-tauri/ app/src-tauri/
COPY cloud/control-plane/ cloud/control-plane/
# `houston-agent-files` embeds the `@houston-ai/agent-schemas` JSON files
# (activity / routines / routine_runs / config / learnings) at compile time via
# `include_str!("../../../ui/agent-schemas/src/*.schema.json")` — that subtree
# is the source of truth for the `.houston/*` data layout, see
# `knowledge-base/architecture.md`.
COPY ui/agent-schemas/ ui/agent-schemas/

# Only build the engine server binary — excludes the desktop app crates.
RUN cargo build --release -p houston-engine-server --bin houston-engine

# ---------- Provider-CLI install stage ----------
# CLIs install under /opt/houston (not /data) so a PVC mount on /data does
# not hide the binaries. The entrypoint symlinks them into $HOME paths the
# engine resolves at runtime (~/.local/bin/claude, ~/.local/bin/codex).
FROM debian:bookworm-slim AS cli-tools
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl bash tar unzip \
 && rm -rf /var/lib/apt/lists/*

ENV HOME=/opt/houston
WORKDIR /opt/houston
RUN mkdir -p /opt/houston/.local/bin

# Claude Code (proprietary). Upstream installer is non-interactive,
# writes to `$HOME/.local/bin/claude` with a sibling `~/.local/share/
# claude/versions/<v>/` tree. Pinned to whatever's stable at build
# time — Anthropic does not publish a linux-x64 URL in cli-deps.json,
# so there's no SHA-256 anchor available. Rebuild the image to upgrade.
RUN curl -fsSL https://claude.ai/install.sh | bash \
 && test -L /opt/houston/.local/bin/claude \
 && test -e "$(readlink -f /opt/houston/.local/bin/claude)"

# Codex (Apache-2.0). Static-musl binary from the openai/codex GitHub
# release. Version pinned to match `cli-deps.json#codex.version`.
ARG CODEX_VERSION=0.130.0
RUN curl -fsSL "https://github.com/openai/codex/releases/download/rust-v${CODEX_VERSION}/codex-x86_64-unknown-linux-musl.tar.gz" -o /tmp/codex.tgz \
 && tar -xzf /tmp/codex.tgz -C /tmp \
 && install -m 755 /tmp/codex-x86_64-unknown-linux-musl /opt/houston/.local/bin/codex \
 && /opt/houston/.local/bin/codex --version \
 && rm -rf /tmp/codex*

# ---------- Runtime stage ----------
# Stays slim. Runtime needs: ca-certs for HTTPS, bash + git + python3
# because agents spawn these through their Bash tool, and the
# pre-installed provider binaries from `cli-tools`.
FROM debian:bookworm-slim
# Fixed UID/GID so K8s fsGroup matches the container user on fresh PVC mounts.
ARG HOUSTON_UID=10000
ARG HOUSTON_GID=10000
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates bash curl git python3 ripgrep zsh gosu \
 && rm -rf /var/lib/apt/lists/* \
 && groupadd -r -g "${HOUSTON_GID}" houston \
 && useradd -r -u "${HOUSTON_UID}" -g houston -d /data -s /usr/sbin/nologin houston \
 && mkdir -p /data \
 && chown houston:houston /data

COPY --from=build /src/target/release/houston-engine /usr/local/bin/houston-engine
COPY --from=cli-tools --chown=root:root /opt/houston/.local /opt/houston/.local
COPY always-on/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
# Product prompt: optional. Run scripts/export-cloud-product-prompt.sh before
# build to bake Houston voice into cloud pods at /opt/houston/product-prompt.txt.
COPY always-on/ /tmp/always-on/
RUN chmod 755 /usr/local/bin/docker-entrypoint.sh \
 && if [ -f /tmp/always-on/houston-product-prompt.txt ]; then \
      mkdir -p /opt/houston \
      && install -m 644 /tmp/always-on/houston-product-prompt.txt /opt/houston/product-prompt.txt; \
    fi \
 && rm -rf /tmp/always-on

USER root
WORKDIR /data
# HOUSTON_NO_PARENT_WATCHDOG=1: no supervisor owns our stdin in a container,
# so disable the parent-watchdog that would otherwise see /dev/null EOF and
# exit the engine at boot (gethouston/houston#306).
ENV HOME=/data \
    HOUSTON_HOME=/data/.houston \
    HOUSTON_DOCS=/data/workspace \
    HOUSTON_BIND=0.0.0.0:7777 \
    HOUSTON_BIND_ALL=1 \
    HOUSTON_NO_PARENT_WATCHDOG=1 \
    PATH=/opt/houston/.local/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

EXPOSE 7777
ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
