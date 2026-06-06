# Prompt — Houston Cloud local (Mac + k3d)

Repo público. No pegar secretos en git ni en issues.

Runbook: `cloud/k8s/LOCAL-RUNBOOK.md`.

**Imagen del pod:** solo `claude` + `codex` (sin Gemini CLI, sin Composio).

---

## Prompt (copiar en Cursor / agente en esta Mac)

```
Montar Houston Cloud local en esta Mac: clonar, pegar credenciales, listo.

Repo: /Users/as/Documents/GitHub/houston_ultra (o clone AndreSierraM/houston_ultra)

### Credenciales (copiar/pegar, dos archivos)

1. Servidor — `cloud/control-plane/deploy/profiles/local.env` (gitignored):
   - `HOUSTON_CLOUD_TOKEN` — generar: `./scripts/generate-cloud-token.sh`
   - `DATABASE_URL` — default del example (Postgres in-cluster)

2. App Mac — `app/.env.local` (gitignored):
   - `VITE_HOUSTON_CLOUD_BASE=http://127.0.0.1:8788`
   - `VITE_HOUSTON_CLOUD_TOKEN=<mismo token que local.env>`

3. Providers (en la app Houston, no en el servidor):
   - Settings → pegar API key de Anthropic (Claude)
   - Settings → pegar API key de OpenAI (Codex) u OpenRouter
   - Al crear agente Nube 24/7, activar sincronización de credenciales

No hace falta instalar CLIs en el Mac para cloud: el pod trae Claude Code y Codex.

### Pasos

cd houston_ultra
./cloud/k8s/scripts/bootstrap-local-env.sh   # si local.env no existe
./cloud/k8s/scripts/setup-local-k3d.sh       # cluster + build imágenes

export HOUSTON_CLOUD_TOKEN=$(grep '^HOUSTON_CLOUD_TOKEN=' cloud/control-plane/deploy/profiles/local.env | cut -d= -f2-)
./cloud/k8s/scripts/smoke-local.sh

cd app && pnpm tauri dev
# Crear agente → Nube 24/7 → verificar: kubectl get deploy -A | grep hou-cloud-agent

### Teardown
./cloud/k8s/scripts/teardown-local-k3d.sh

### Si falla
- docker info (RAM ≥ 8 GB, disco ≥ 40 GB)
- kubectl config use-context k3d-houston-local
- kubectl -n houston-system logs deployment/houston-control-plane
- curl -v http://127.0.0.1:8788/health
```

---

Ver también `cloud/k8s/LOCAL-RUNBOOK.md`.
