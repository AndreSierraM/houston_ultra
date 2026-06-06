export PATH="$HOME/.npm-global/bin:$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"

# Raíz (workspace)
cd /Users/as/Documents/GitHub/houston_ultra
pnpm install
cargo build -p houston-engine-server

# Cloud blyxlabs → cloudhouston.blyxlabs.dev (envs listos)
# Servidor: cloud/control-plane/deploy/.env
# App: app/.env.local
# Dokploy: cloud/control-plane/deploy/DOKPLOY.md

# Cloud local K8s (k3d en Mac, Postgres in-cluster)
# Runbook: cloud/k8s/LOCAL-RUNBOOK.md
# ./cloud/k8s/scripts/bootstrap-local-env.sh
# ./cloud/k8s/scripts/setup-local-k3d.sh
# export HOUSTON_CLOUD_TOKEN=... && ./cloud/k8s/scripts/smoke-local.sh
# Tras cambios en control-plane: ./cloud/k8s/scripts/redeploy-control-plane-fast.sh (~2 min)
# app/.env.local: VITE_HOUSTON_CLOUD_BASE=http://127.0.0.1:8788 + mismo token
# Teardown: ./cloud/k8s/scripts/teardown-local-k3d.sh
# Prompt IA: cloud/k8s/PROMPT-LOCAL.md

# Cloud local bare metal (sin K8s)
# cargo run -p houston-cloud-control-plane + DATABASE_URL local

# App principal
cd app && pnpm tauri dev

# En terminales aparte:
cd mobile && pnpm dev                    # :5173
cd houston-relay && pnpm dev              # :8787 (build mobile antes: pnpm build)
cd website && npm install && npm run dev  # :8080 (fuera del pnpm workspace)
