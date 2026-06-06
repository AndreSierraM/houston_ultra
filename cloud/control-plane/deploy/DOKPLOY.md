# Houston Cloud en Dokploy — `cloudhouston.blyxlabs.dev`

Repo: [AndreSierraM/houston_ultra](https://github.com/AndreSierraM/houston_ultra)

**Usa Dokploy**, no el compose con Caddy manual. Dokploy ya tiene Traefik en 80/443; el compose con Caddy chocaría.

## 1. DNS

Registro **A** (o CNAME):

```text
cloudhouston.blyxlabs.dev → IP del VPS
```

## 2. Proyecto Compose en Dokploy

| Campo | Valor |
|-------|--------|
| Source | GitHub `AndreSierraM/houston_ultra` |
| Branch | la que tenga cloud (p. ej. `main` o tu worktree) |
| Compose file | `cloud/control-plane/deploy/docker-compose.dokploy.yml` |
| Env file | variables desde `profiles/cloudhouston.blyxlabs.dev.env.example` (rellenar en Dokploy, **no** commitear) |

Variables mínimas en Dokploy (valores reales solo en el panel del VPS, nunca en git):

```env
SUPABASE_DB_PASSWORD=<desde dashboard Supabase>
DATABASE_URL=postgresql://postgres.<project-ref>:<password>@<pool-host>:5432/postgres?sslmode=require
HOUSTON_CLOUD_AUTH=local
HOUSTON_CLOUD_TOKEN=hst_<openssl rand -hex 32>
HOUSTON_CLOUD_LOCAL_USER_ID=00000000-0000-0000-0000-000000000001
HOUSTON_CLOUD_LOCAL_EMAIL=you@example.com
HOUSTON_CLOUD_DOMAIN=cloudhouston.blyxlabs.dev
HOUSTON_ENGINE_IMAGE=houston/engine:dev
HOUSTON_CLOUD_CORS_ORIGINS=*
```

## 3. Dominio en Dokploy

En el servicio **control-plane**:

- Host: `cloudhouston.blyxlabs.dev`
- Puerto contenedor: **8788**
- HTTPS: activado (Let's Encrypt vía Dokploy)

No publiques Postgres ni el puerto 8788 al mundo si Dokploy enruta por red interna; con `8788:8788` en el compose basta para que Traefik alcance el servicio.

## 4. Docker socket (obligatorio)

El control-plane **debe** montar `/var/run/docker.sock` para crear contenedores `hou-cloud-agent-*`.

En Dokploy, en opciones avanzadas del servicio **control-plane**, confirma que el volumen del socket está permitido (el compose ya lo declara). Si Dokploy bloquea el socket, el deploy fallará al crear el primer agente cloud.

## 5. Build imagen engine (una vez en el VPS)

SSH al servidor (o terminal Dokploy):

```bash
git clone https://github.com/AndreSierraM/houston_ultra.git
cd houston_ultra
docker build -t houston/engine:dev -f always-on/Dockerfile .
docker image inspect houston/engine:dev --format '{{.Id}}'
```

Sin esta imagen, `POST /v1/cloud/agents` falla al provisionar.

## 6. Cuota dev (más de 1 agente)

Tras el primer `GET /v1/cloud/me`:

```bash
docker exec -it <postgres-container> psql -U houston -d houston_cloud -c \
  "UPDATE cloud_entitlements SET max_cloud_agents = 10 WHERE status = 'active';"
```

## 7. Smoke

```bash
curl -fsS https://cloudhouston.blyxlabs.dev/health

export HOUSTON_CLOUD_TOKEN="<tu token del panel Dokploy>"
curl -fsS -H "Authorization: Bearer $HOUSTON_CLOUD_TOKEN" \
  https://cloudhouston.blyxlabs.dev/v1/cloud/me
```

O desde el repo:

```bash
cd cloud/control-plane/deploy
export HOUSTON_CLOUD_BASE=https://cloudhouston.blyxlabs.dev
export HOUSTON_CLOUD_TOKEN="<tu token>"
./smoke.sh
```

## 8. App en tu Mac (`app/.env.local`, gitignored)

```env
VITE_HOUSTON_CLOUD_BASE=https://cloudhouston.blyxlabs.dev
VITE_HOUSTON_CLOUD_TOKEN=<mismo HOUSTON_CLOUD_TOKEN del servidor>
```

```bash
cd app && pnpm tauri dev
```

Crear agente → **Nube 24/7** → chatear.

## Terminal vs Dokploy

| | Dokploy (recomendado) | SSH + `docker compose` |
|--|----------------------|---------------------------|
| TLS | Traefik de Dokploy | Caddy en `docker-compose.yml` |
| Updates | UI + redeploy | `git pull` + compose |
| Conflicto 80/443 | No | Sí si Dokploy ya corre |

## Red Docker (si create agente falla)

Si el log dice `engine did not become healthy`, el control-plane no alcanza el contenedor del agente en `hou-org-*`. Diagnóstico:

```bash
docker ps | grep hou-cloud
docker network ls | grep hou-org
docker compose logs control-plane
```

Workaround temporal: conectar el contenedor del control-plane a la red del org tras crear agente. Fix permanente: IP del contenedor en `internal_url` (pendiente en código).
