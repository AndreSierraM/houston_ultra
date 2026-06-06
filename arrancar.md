export PATH="$HOME/.npm-global/bin:$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"

# Raíz (workspace)
cd /Users/as/Documents/GitHub/houston_ultra
pnpm install
cargo build -p houston-engine-server

# Cloud blyxlabs → cloudhouston.blyxlabs.dev (envs listos)
# Servidor: cloud/control-plane/deploy/.env
# App: app/.env.local
# Dokploy: cloud/control-plane/deploy/DOKPLOY.md

# Cloud local (sin Supabase)
# ./scripts/generate-cloud-token.sh   # copia las dos líneas a deploy/.env y app/.env.local
# 1. Postgres + control plane:
#    DATABASE_URL=postgres://... HOUSTON_CLOUD_AUTH=local HOUSTON_CLOUD_TOKEN=... cargo run -p houston-cloud-control-plane
#    (sin HOUSTON_CLOUD_TOKEN el servidor genera uno efímero en el log)
# 2. app/.env.local:
#    VITE_HOUSTON_CLOUD_BASE=http://127.0.0.1:8788
#    VITE_HOUSTON_CLOUD_TOKEN=...   # mismo valor que HOUSTON_CLOUD_TOKEN

# App principal
cd app && pnpm tauri dev

# En terminales aparte:
cd mobile && pnpm dev                    # :5173
cd houston-relay && pnpm dev              # :8787 (build mobile antes: pnpm build)
cd website && npm install && npm run dev  # :8080 (fuera del pnpm workspace)
