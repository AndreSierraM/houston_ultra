# Houston Cloud — QA local en Mac (k3d)

Runbook para validar el control plane en Kubernetes local antes de desplegar al VPS. **Sin secretos en git:** tokens y `DATABASE_URL` viven solo en `profiles/local.env` y `app/.env.local` (gitignored).

Documentación completa aquí. Resumen rápido en `arrancar.md`.

## Prerrequisitos

| Herramienta | Verificación | Notas |
|-------------|--------------|-------|
| Docker Desktop | `docker info` | **RAM:** ≥ 8 GB asignados (16 GB recomendado). **Disco:** ≥ 40 GB libres en la VM de Docker (Settings → Resources). |
| k3d | `k3d version` | `brew install k3d` |
| kubectl | `kubectl version --client` | `brew install kubectl` |
| Repo | `cd /Users/as/Documents/GitHub/houston_ultra` | |

No hace falta aplicar `001_init.sql` a mano: el control plane ejecuta la migración al arrancar (`db.migrate()` en boot).

## 1. Perfil local (Secret K8s)

```bash
cp cloud/control-plane/deploy/profiles/local.env.example \
   cloud/control-plane/deploy/profiles/local.env
```

O en un solo paso (solo si `local.env` no existe):

```bash
./cloud/k8s/scripts/bootstrap-local-env.sh
./scripts/generate-cloud-token.sh   # si el token sigue siendo placeholder
```

Editar `local.env` (fuera de git):

- `DATABASE_URL` — Postgres **in-cluster** (default del example):  
  `postgresql://houston:change-me-local-dev@postgres:5432/houston_cloud`  
  La password debe coincidir con `cloud/k8s/overlays/local/postgres.yaml`. Supabase u otro Postgres externo solo si cambias el overlay a propósito.
- `HOUSTON_CLOUD_TOKEN` — generar con el script del paso 2.

No commitear `local.env`. Las líneas `VITE_*` son solo para la app desktop; el script de Secret las excluye.

## 2. Token compartido

```bash
./scripts/generate-cloud-token.sh
```

Copiar el mismo valor a:

- `HOUSTON_CLOUD_TOKEN` en `profiles/local.env`
- `VITE_HOUSTON_CLOUD_TOKEN` en `profiles/local.env` (referencia) y en `app/.env.local`

## 3. Cluster k3d + control plane

```bash
chmod +x cloud/k8s/scripts/setup-local-k3d.sh
./cloud/k8s/scripts/setup-local-k3d.sh
```

El script:

1. Crea cluster `houston-local` con mapeo host `8788:80@loadbalancer` (Traefik de k3d).
2. Construye `houston/engine:dev` y `houston/control-plane:dev`.
3. Importa imágenes al cluster.
4. Aplica Secret desde `profiles/local.env`.
5. Despliega overlay local (`base` + Postgres in-cluster + **Ingress Traefik**).

**Red local:** el Service `houston-control-plane` es **ClusterIP** (`:8788`). El tráfico desde el Mac entra por **Ingress Traefik** → Service, no por un LoadBalancer del Service. Desde el host: `http://127.0.0.1:8788`.

**Tras cambios en `cloud/control-plane/`** (p. ej. fix del proxy Axum), volver a ejecutar `./cloud/k8s/scripts/setup-local-k3d.sh` para reconstruir e importar la imagen.

Verificación rápida:

```bash
kubectl config current-context   # k3d-houston-local
kubectl -n houston-system get pods,svc,ingress
```

## 4. Smoke automatizado

```bash
export HOUSTON_CLOUD_TOKEN=hst_<tu-token>
chmod +x cloud/k8s/scripts/smoke-local.sh
./cloud/k8s/scripts/smoke-local.sh
```

Comprueba: contexto kubectl, pod `houston-control-plane` Running, `GET /health` → `ok`, `GET /v1/cloud/me` con Bearer.

Variables opcionales: `KUBECTL_CONTEXT`, `HOUSTON_CLOUD_BASE` (default `http://127.0.0.1:8788`).

## 5. App desktop

Crear o editar `app/.env.local` (gitignored):

```env
VITE_HOUSTON_CLOUD_BASE=http://127.0.0.1:8788
VITE_HOUSTON_CLOUD_TOKEN=hst_<mismo HOUSTON_CLOUD_TOKEN del perfil>
```

Reiniciar la app para que Vite cargue las variables:

```bash
cd app && pnpm tauri dev
```

## 6. Providers (copiar/pegar API keys)

La imagen `houston/engine:dev` incluye **solo Claude Code y Codex**. No hay Gemini CLI ni Composio en el pod.

En la app Houston (antes o al crear el agente cloud):

1. **Settings → Providers** → pegar API key de **Anthropic** (Claude).
2. Pegar API key de **OpenAI** (Codex) o **OpenRouter** (rutea vía Codex).
3. Al crear agente **Nube 24/7**, dejar activada la sincronización de credenciales al pod.

No hace falta OAuth en el servidor ni instalar CLIs en el Mac para cloud.

## 7. Agente Nube 24/7 (E2E)

En la app Houston:

1. Crear agente con runtime **Nube 24/7** (con credenciales ya pegadas en Settings).
2. El control plane debe provisionar en el cluster:
   - Namespace `hou-org-{org_id}`
   - Deployment + PVC + Service del engine (`houston/engine:dev`)

Verificar en terminal:

```bash
kubectl get deploy -A | grep -E 'houston-control-plane|hou-cloud-agent'
kubectl get ns | grep hou-org
```

Si el provision falla:

```bash
kubectl -n houston-system logs deployment/houston-control-plane --tail=100
```

## 8. Limpieza

```bash
./cloud/k8s/scripts/teardown-local-k3d.sh
```

Equivalente manual: `k3d cluster delete houston-local`.

## Troubleshooting

| Síntoma | Acción |
|---------|--------|
| `ImagePullBackOff` | Re-ejecutar `setup-local-k3d.sh` (reimporta imágenes). |
| `curl :8788` falla | `kubectl -n houston-system get ingress,pods,svc`. Pods Running; Ingress presente; Traefik expone `:8788` vía k3d (`8788:80@loadbalancer`). El Service es ClusterIP, no LoadBalancer. |
| App apunta a `:8789` | `app/.env.local` debe usar `VITE_HOUSTON_CLOUD_BASE=http://127.0.0.1:8788` (**8788**, no 8789). Reiniciar `pnpm tauri dev`. |
| Proxy agente 404 | Imagen antigua sin fix Axum del proxy; re-ejecutar `setup-local-k3d.sh`. |
| `/v1/cloud/me` 401 | Token distinto entre Secret y `HOUSTON_CLOUD_TOKEN` exportado. |
| `/v1/cloud/entitlements` 404 o create 403/404 "Entitlement not found" | Org sin fila en `cloud_entitlements` (DB parcial o borrada). Tras fix en control plane: `GET /v1/cloud/me` backfilla fila `active`. Imagen antigua: `INSERT` manual o redeploy. Status `past_due`/`canceled`: `UPDATE cloud_entitlements SET status = 'active' WHERE org_id = '<orgId de /me>';` |
| Error DB en logs | Revisar `DATABASE_URL` (host `postgres:5432`, password alineada con `postgres.yaml`). El control plane migra solo; no hace falta `001_init.sql` manual salvo depuración. |
| App sin cloud | Confirmar `app/.env.local` (puerto **8788**) y reiniciar `pnpm tauri dev`. |
| Flannel / `subnet.env`: pod en CrashLoop, log tipo `failed to read subnet.env` o CNI no levanta | **Race CNI** al crear el cluster. `./cloud/k8s/scripts/teardown-local-k3d.sh` y volver a `./cloud/k8s/scripts/setup-local-k3d.sh`. Si se repite, fijar imagen k3s al crear: `k3d cluster create houston-local --image rancher/k3s:v1.31.5-k3s1 --port "8788:80@loadbalancer" --agents 1` (ajusta versión según `k3d version`). |
| `invalid capacity 0` / `no space left on device` al pull o build de imagen | **Transitorio:** reintentar build/import. **Persistente:** `docker system df`; `docker system prune -a` (libera imágenes no usadas); subir disco en Docker Desktop → Settings → Resources → Disk image size; reiniciar Docker Desktop. |

## Checklist QA local

Marca cada ítem antes de considerar el entorno válido.

### Infra

- [ ] Docker Desktop en ejecución (`docker info` OK), RAM/disco suficientes
- [ ] k3d y kubectl instalados
- [ ] `profiles/local.env` creado desde example (no commiteado)
- [ ] `DATABASE_URL` apunta a `postgres:5432` con password del overlay local
- [ ] `./cloud/k8s/scripts/setup-local-k3d.sh` termina sin error
- [ ] Contexto kubectl = `k3d-houston-local`
- [ ] `kubectl -n houston-system get pods` → control-plane y postgres `Running`

### API

- [ ] `curl -sf http://127.0.0.1:8788/health` → `ok`
- [ ] `export HOUSTON_CLOUD_TOKEN=...` (mismo que Secret)
- [ ] `curl -sf -H "Authorization: Bearer $HOUSTON_CLOUD_TOKEN" http://127.0.0.1:8788/v1/cloud/me` → JSON usuario
- [ ] `./cloud/k8s/scripts/smoke-local.sh` → `smoke local: OK`

### App desktop

- [ ] `app/.env.local` con `VITE_HOUSTON_CLOUD_BASE=http://127.0.0.1:8788`
- [ ] `VITE_HOUSTON_CLOUD_TOKEN` = mismo token que servidor
- [ ] App reiniciada (`pnpm tauri dev`)
- [ ] UI permite crear agente **Nube 24/7** sin error de token

### Providers

- [ ] API keys pegadas en Settings (Anthropic, OpenAI u OpenRouter)
- [ ] Credenciales sincronizadas al crear agente cloud

### Provision agente

- [ ] Agente Nube 24/7 creado desde la app
- [ ] `kubectl get deploy -A` lista deployment del agente (`hou-cloud-agent` o similar)
- [ ] Namespace `hou-org-*` presente
- [ ] Chat con el agente cloud responde (smoke funcional E2E)

### Seguridad

- [ ] Ningún token ni `DATABASE_URL` en commits, issues ni PRs
- [ ] Solo `*.env.example` en el repo; perfiles reales gitignored
