#!/usr/bin/env bash
# Escribe SUPABASE_DB_PASSWORD + DATABASE_URL en los env de Houston Cloud y prueba conexión.
# Uso: ./scripts/sync-supabase-db-env.sh 'tu-database-password'
set -euo pipefail

if [[ $# -lt 1 || -z "${1:-}" ]]; then
  echo "Uso: $0 <supabase-database-password>" >&2
  echo "Obtenerla: https://supabase.com/dashboard/project/vfdydumriboswopwrakb/settings/database" >&2
  exit 1
fi

PW="$1"
PROJECT_REF="vfdydumriboswopwrakb"
POOL_HOST="aws-1-us-east-2.pooler.supabase.com"
DB_URL="postgresql://postgres.${PROJECT_REF}:${PW}@${POOL_HOST}:6543/postgres?sslmode=require"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEPLOY_ENV="$ROOT/cloud/control-plane/deploy/.env"
PROFILE_ENV="$ROOT/cloud/control-plane/deploy/profiles/cloudhouston.blyxlabs.dev.env"

write_env_block() {
  local file="$1"
  local tmp
  tmp="$(mktemp)"
  {
    echo "SUPABASE_DB_PASSWORD=${PW}"
    echo "DATABASE_URL=${DB_URL}"
    echo ""
    grep -v -E '^(SUPABASE_DB_PASSWORD|DATABASE_URL|POSTGRES_PASSWORD)=' "$file" 2>/dev/null || true
  } >"$tmp"
  mv "$tmp" "$file"
}

for f in "$DEPLOY_ENV" "$PROFILE_ENV"; do
  [[ -f "$f" ]] || touch "$f"
  write_env_block "$f"
  echo "actualizado: $f"
done

echo "probando pooler..."
docker run --rm postgres:16-alpine psql "$DB_URL" -c "SELECT org_id, max_cloud_agents FROM cloud_entitlements LIMIT 1;"
echo "OK — redeploy control-plane en Dokploy con SUPABASE_DB_PASSWORD"
