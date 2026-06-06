# Prompt — Houston Cloud local (Mac + k3d)

Repo público. No pegar secretos en git ni en issues.

Runbook detallado: `cloud/k8s/LOCAL-RUNBOOK.md`.

---

## Prompt (copiar en Cursor / agente en esta Mac)

```
Montar Houston Cloud 100% local en esta Mac con k3d + Docker.

Repo: /Users/as/Documents/GitHub/houston_ultra (o clone público AndreSierraM/houston_ultra)

Objetivo: control plane en K8s local, Postgres in-cluster, runtime k8s para agentes cloud, app desktop en http://127.0.0.1:8788 (Ingress Traefik → Service ClusterIP, no LoadBalancer del Service).

### Prerrequisitos
- Docker Desktop corriendo (≥ 8 GB RAM, ≥ 40 GB disco en Resources)
- k3d, kubectl instalados (brew install k3d kubectl)
- No aplicar 001_init.sql a mano: el control plane migra al arrancar

### Paso 1 — Env local
cd houston_ultra
./cloud/k8s/scripts/bootstrap-local-env.sh
# Si profiles/local.env ya existe: ./scripts/generate-cloud-token.sh
# DATABASE_URL default: postgresql://houston:change-me-local-dev@postgres:5432/houston_cloud

### Paso 2 — Cluster + deploy
./cloud/k8s/scripts/setup-local-k3d.sh
# Primera vez: build Rust en Docker (~10-15 min)

### Paso 3 — Smoke
export HOUSTON_CLOUD_TOKEN=$(grep '^HOUSTON_CLOUD_TOKEN=' cloud/control-plane/deploy/profiles/local.env | cut -d= -f2-)
./cloud/k8s/scripts/smoke-local.sh

### Paso 4 — App Mac
En app/.env.local (gitignored):
VITE_HOUSTON_CLOUD_BASE=http://127.0.0.1:8788
VITE_HOUSTON_CLOUD_TOKEN=<mismo token>

cd app && pnpm tauri dev

Crear agente → Nube 24/7. Verificar:
kubectl get deploy -A | grep hou-cloud-agent

### Teardown
./cloud/k8s/scripts/teardown-local-k3d.sh

### Si falla conexión
- docker info (RAM/disco Docker Desktop)
- kubectl config use-context k3d-houston-local
- kubectl -n houston-system get ingress,pods,svc
- kubectl -n houston-system logs deployment/houston-control-plane
- curl -v http://127.0.0.1:8788/health

### Si falla CNI (flannel subnet.env)
./cloud/k8s/scripts/teardown-local-k3d.sh && ./cloud/k8s/scripts/setup-local-k3d.sh
# Recurrente: k3d cluster create con --image rancher/k3s:v1.31.5-k3s1 (ver LOCAL-RUNBOOK.md)

### Si falla build/pull (invalid capacity 0)
docker system df && docker system prune -a
# Subir disco en Docker Desktop → Settings → Resources
```

---

Ver también `cloud/k8s/LOCAL-RUNBOOK.md`.
