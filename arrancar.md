export PATH="$HOME/.npm-global/bin:$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"

# Raíz (workspace)
cd /Users/as/Documents/GitHub/houston_ultra
pnpm install
cargo build -p houston-engine-server

# ─── Cloud local (k3d, todo en Mac) ─────────────────────────────────────
# Runbook: cloud/k8s/LOCAL-RUNBOOK.md
# Prompt IA: cloud/k8s/PROMPT-LOCAL.md

# 1. Env (una vez): copiar example → local.env, pegar token generado
./cloud/k8s/scripts/bootstrap-local-env.sh
./scripts/generate-cloud-token.sh

# 2. Cluster + imágenes (Claude Code + Codex en el pod, sin Gemini ni Composio)
./cloud/k8s/scripts/setup-local-k3d.sh

# 3. App: pegar mismo token en app/.env.local
#    VITE_HOUSTON_CLOUD_BASE=http://127.0.0.1:8788
#    VITE_HOUSTON_CLOUD_TOKEN=<mismo HOUSTON_CLOUD_TOKEN>
cd app && pnpm tauri dev

# 4. En la app: Settings → pegar API keys (Anthropic, OpenAI, OpenRouter)
#    Crear agente → Nube 24/7 (sincroniza credenciales al pod)

# Smoke: export HOUSTON_CLOUD_TOKEN=... && ./cloud/k8s/scripts/smoke-local.sh
# Redeploy control-plane: ./cloud/k8s/scripts/redeploy-control-plane-fast.sh
# Teardown: ./cloud/k8s/scripts/teardown-local-k3d.sh

# ─── Cloud VPS (blyxlabs) ───────────────────────────────────────────────
# Servidor: cloud/control-plane/deploy/profiles/cloudhouston.blyxlabs.dev.env
# App: app/.env.local
# Prompt IA: cloud/k8s/PROMPT-SERVER.md
# Dokploy: cloud/control-plane/deploy/DOKPLOY.md

# ─── Otros ──────────────────────────────────────────────────────────────
cd mobile && pnpm dev                    # :5173
cd houston-relay && pnpm dev              # :8787
cd website && npm install && npm run dev  # :8080
