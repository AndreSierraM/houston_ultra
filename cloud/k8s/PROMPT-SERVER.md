# Prompt para IA en el VPS (K3s / Kubernetes)

**Repo público.** Nunca commitear ni pegar en issues/PRs: `DATABASE_URL`, `SUPABASE_DB_PASSWORD`, `HOUSTON_CLOUD_TOKEN`.

Plantilla: `cloud/control-plane/deploy/profiles/cloudhouston.blyxlabs.dev.env.example`  
Real (gitignored): `profiles/cloudhouston.blyxlabs.dev.env`

**Imagen del pod:** solo `claude` + `codex` (sin Gemini CLI, sin Composio).

---

## Prompt (copiar desde aquí)

```
Eres el operador de despliegue de Houston Cloud en este VPS.

Objetivo: control plane en K8s, Postgres Supabase externo, agentes cloud con Claude Code + Codex en cada pod.

Repositorio: https://github.com/AndreSierraM/houston_ultra
Rama: main

Requisitos: kubectl, docker, DNS cloudhouston.blyxlabs.dev → este servidor, Ingress (Traefik en K3s).

NO Docker Compose para control plane. NO Postgres en cluster. NO pods de agentes a mano.

### Credenciales (el operador las pega por canal privado)

**Servidor** — editar `cloud/control-plane/deploy/profiles/cloudhouston.blyxlabs.dev.env`:
- `DATABASE_URL` — pooler Supabase :5432 sslmode=require
- `SUPABASE_DB_PASSWORD`
- `HOUSTON_CLOUD_TOKEN` — generar: echo "hst_$(openssl rand -hex 32)"
- `HOUSTON_CLOUD_DOMAIN`, `HOUSTON_ENGINE_IMAGE`, etc. (ver .env.example)

**App desktop del operador** — `app/.env.local`:
- `VITE_HOUSTON_CLOUD_BASE=https://cloudhouston.blyxlabs.dev`
- `VITE_HOUSTON_CLOUD_TOKEN=<mismo HOUSTON_CLOUD_TOKEN>`

**Providers** — en la app Houston (no en el VPS):
- Settings → pegar API keys (Anthropic, OpenAI/Codex, OpenRouter)
- Crear agente Nube 24/7 con sync de credenciales activado

### Paso 1 — Clonar

cd ~
git clone https://github.com/AndreSierraM/houston_ultra.git || true
cd houston_ultra && git pull

### Paso 2 — Secret K8s

cp cloud/control-plane/deploy/profiles/cloudhouston.blyxlabs.dev.env.example \
   cloud/control-plane/deploy/profiles/cloudhouston.blyxlabs.dev.env
# Pegar valores reales en .env (canal privado)

chmod +x cloud/k8s/scripts/create-control-plane-secret.sh
./cloud/k8s/scripts/create-control-plane-secret.sh cloud/control-plane/deploy/profiles/cloudhouston.blyxlabs.dev.env

### Paso 3 — Build imágenes

chmod +x cloud/k8s/scripts/build-images.sh
./cloud/k8s/scripts/build-images.sh
# houston/engine:dev (claude + codex) + houston/control-plane:dev

### Paso 4 — Deploy

kubectl apply -k cloud/k8s/overlays/blyxlabs
kubectl -n houston-system rollout status deployment/houston-control-plane --timeout=180s

### Paso 5 — Smoke

curl -sf https://cloudhouston.blyxlabs.dev/health   # ok
curl -sf -H "Authorization: Bearer $HOUSTON_CLOUD_TOKEN" https://cloudhouston.blyxlabs.dev/v1/cloud/me

### Paso 6 — App desktop

Reiniciar app con app/.env.local actualizado. Crear agente Nube 24/7.
kubectl get deployments -A | grep hou-cloud-agent

### Troubleshooting

- ImagePullBackOff → build-images.sh o imagePullPolicy IfNotPresent
- DB error → DATABASE_URL pooler 5432 sslmode=require
- Provision falla → kubectl logs -n houston-system deployment/houston-control-plane

Entrega: /health OK, control-plane Running, operador tiene token + instrucciones app.
```

---

## Arquitectura

```text
Ingress (cloudhouston.blyxlabs.dev)
  → houston-control-plane:8788 (houston-system)
      → Postgres Supabase (externo)
      → Por agente: hou-org-{org} / Deployment + PVC + houston/engine:dev (claude + codex)
```

## Archivos clave

| Ruta | Uso |
|------|-----|
| `cloud/k8s/overlays/blyxlabs/` | Kustomize |
| `cloud/k8s/scripts/build-images.sh` | Build engine + control-plane |
| `cloud/k8s/scripts/create-control-plane-secret.sh` | Secret desde perfil |
| `always-on/Dockerfile` | Imagen engine (Claude Code + Codex) |
| `cloud/control-plane/deploy/profiles/*.env.example` | Plantillas |
