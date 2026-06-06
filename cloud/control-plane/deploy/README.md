# Houston Cloud — VPS deploy

Production stack for the Houston Cloud control plane on a single VPS:

- **Postgres** — internal only, schema migrated on control-plane boot
- **control-plane** — internal only (`8788` on the compose network, not published to the host)
- **Caddy** — public `80`/`443`, automatic TLS, reverse-proxies to control-plane

Only port **443** (and **80** for ACME) should be reachable from the internet. Engine containers spawned by the control plane stay on private Docker networks.

## Prerequisites

- Linux VPS with Docker Engine and the Compose plugin (`docker compose version`)
- DNS **A record** for your domain pointing at the VPS public IP (required before Caddy can issue TLS)
- Supabase project (same one the Houston desktop app uses for auth)
- Firewall allowing inbound **80** and **443**

## 1. Clone the repo on the VPS

```bash
git clone https://github.com/gethouston/houston.git
cd houston
```

Use your fork or deploy branch if you are not on `main`.

## 2. Create `.env`

From the repo root:

```bash
cd cloud/control-plane/deploy
$EDITOR .env
chmod 600 .env
```

Edit `.env`:

```bash
# Public hostname (Caddy TLS + app config)
HOUSTON_CLOUD_DOMAIN=cloud.example.com

# Postgres (generate a strong password)
POSTGRES_PASSWORD=

# Auth (default: local static token — no Supabase)
HOUSTON_CLOUD_AUTH=local
HOUSTON_CLOUD_TOKEN=generate-a-long-random-secret

# Optional JWT mode (Supabase or self-minted tokens):
# HOUSTON_CLOUD_AUTH=jwt
# HOUSTON_CLOUD_JWT_SECRET=your-signing-secret

# Engine image the control plane spawns for cloud agents (build in step 3)
HOUSTON_ENGINE_IMAGE=houston/engine:dev
```

Set the same `HOUSTON_CLOUD_TOKEN` in `app/.env.local` as `VITE_HOUSTON_CLOUD_TOKEN`. For JWT mode, mint with `cargo run -p houston-cloud-control-plane --bin houston-cloud-mint-token`.

## 3. Build the engine image

Cloud agents run the Houston Engine inside Docker. Build the image once on the VPS (from repo root):

```bash
cd /path/to/houston
docker build -t houston/engine:dev -f always-on/Dockerfile .
```

Confirm:

```bash
docker image inspect houston/engine:dev --format '{{.Id}}'
```

Set `HOUSTON_ENGINE_IMAGE=houston/engine:dev` in `.env` (or retag to match your chosen name).

## 4. Build and start the stack

```bash
cd cloud/control-plane/deploy
docker compose build
docker compose up -d
```

Watch startup:

```bash
docker compose ps
docker compose logs -f control-plane
```

Postgres must become healthy before control-plane starts. Control-plane must become healthy before Caddy starts.

## 5. Verify health

### Compose service status

```bash
docker compose ps
```

All three services should show `healthy` (Caddy may show `healthy` after config validation).

### HTTP smoke script

```bash
chmod +x smoke.sh

# Option A: domain from .env
./smoke.sh

# Option B: explicit base URL
export HOUSTON_CLOUD_BASE="https://cloud.example.com"
./smoke.sh
```

`/health` must return `ok` with no auth.

To test authenticated routes, obtain a Supabase **user** access token (not the anon key) from a signed-in Houston session or Supabase dashboard, then:

```bash
export HOUSTON_CLOUD_JWT="eyJ..."
./smoke.sh
```

Expect `200` JSON from `/v1/cloud/me` with `userId`, `orgId`, and `orgRole`.

### Manual curls

```bash
curl -fsS "https://cloud.example.com/health"

curl -fsS -H "Authorization: Bearer $JWT" \
  "https://cloud.example.com/v1/cloud/me"
```

## 6. Configure the Houston desktop app

Point the app at your control plane URL at **build time** (or in local dev via `.env.local`):

```bash
# app/.env.local
VITE_HOUSTON_CLOUD_BASE=https://cloud.example.com
```

Rebuild or restart `pnpm tauri dev` so Vite picks up the variable. Without it, cloud agent features show “Set VITE_HOUSTON_CLOUD_BASE…” in the shell UI.

Use the same Supabase project (`SUPABASE_URL` / `SUPABASE_ANON_KEY`) in the app as on the server so user JWTs validate on `/v1/cloud/*`.

## Operations

| Task | Command |
|------|---------|
| Logs | `docker compose logs -f` |
| Restart | `docker compose restart control-plane` |
| Upgrade | `git pull`, rebuild engine + `docker compose build && docker compose up -d` |
| Stop | `docker compose down` |

Back up the `houston-cloud-postgres` volume and agent Docker volumes regularly. See `cloud/cloud-agents-ops.md` for runtime and security notes.

## Troubleshooting

- **Caddy TLS fails** — DNS must resolve to this VPS before first start; ports 80/443 must be open.
- **`control-plane` unhealthy** — check `docker compose logs control-plane`; usually bad `DATABASE_URL` or missing Supabase env.
- **`401` on `/v1/cloud/me`** — token expired, anon key used instead of user JWT, or `SUPABASE_JWT_SECRET` mismatch with the Supabase project.
- **Cloud agents fail to start** — confirm `HOUSTON_ENGINE_IMAGE` exists locally (`docker images`) and `/var/run/docker.sock` is mounted.
