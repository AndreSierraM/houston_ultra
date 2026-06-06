# Houston Cloud Control Plane

Auth, entitlements, cloud agent lifecycle, and engine proxy for Houston Cloud.

## Unit tests

```bash
cargo test -p houston-cloud-control-plane
```

Covers JWT validation (anon rejection, bearer extraction), entitlement quota gates, agent role ranking, proxy path joining (`v1/health` without double `/v1/v1`), and GET vs POST RBAC on the engine proxy (DB test when `DATABASE_URL` is set).

## Connect Houston App

Point the desktop app at your control plane and reuse the same Supabase project as the server.

1. **App env** — in `app/.env.local`:

   ```bash
   VITE_HOUSTON_CLOUD_BASE=https://your-server
   ```

   Use your public control plane URL (no trailing slash). For local dev: `http://127.0.0.1:8788`.

2. **Supabase** — app and control plane must share the same Supabase project (`SUPABASE_URL`, `SUPABASE_JWT_SECRET` on the server; app uses the same anon URL/key).

3. **Flow** — sign in → create agent → choose **Nube 24/7** (Cloud 24/7) → chat. The app routes REST/WS through `/v1/cloud/agents/:id/proxy/v1/...` to the private engine container.

## Manual smoke checklist

Prerequisites: Postgres migrated (`migrations/001_init.sql`), env vars from `src/config.rs`, server running on `HOUSTON_CLOUD_BIND` (default `0.0.0.0:8788`).

1. **Health** — `curl -s http://localhost:8788/health` returns `ok` (no auth).
2. **GET /v1/cloud/me** — Supabase user JWT:
   ```bash
   curl -s -H "Authorization: Bearer $JWT" http://localhost:8788/v1/cloud/me
   ```
   Expect `200` with `userId`, `orgId`, `orgRole`. Missing or anon token → `401`.
3. **Create agent** — `POST /v1/cloud/agents` with JSON body (`name`, `workspace`, etc. per `CreateCloudAgent`):
   ```bash
   curl -s -X POST -H "Authorization: Bearer $JWT" -H "Content-Type: application/json" \
     -d '{"name":"smoke-agent","workspace":"default"}' \
     http://localhost:8788/v1/cloud/agents
   ```
   Expect `200` when entitlement is active and under quota; at limit → `403` with limit message.
4. **Proxy health** — After agent is running, hit engine health through the proxy:
   ```bash
   curl -s -H "Authorization: Bearer $JWT" \
     http://localhost:8788/v1/cloud/agents/$AGENT_ID/proxy/health
   ```
   Expect engine health response (typically `ok`). Invalid agent or token → `401`/`403`/`404`.
