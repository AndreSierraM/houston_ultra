# Informe Cloud cluster + harness

Houston Cloud conserva el mismo loop de agente que Houston local: un proceso
`houston-engine` ejecuta el agente, y los proveedores siguen pasando por
subprocesos CLI en `houston-terminal-manager`. Cloud agrega control-plane,
backend de provisionamiento, red privada y proxy. No reemplaza el harness por
llamadas HTTP directas a modelos.

Refs inspeccionadas: `main` / `origin/main` en `85977ca`, comparado contra
`upstream/main` en `523bd6c`.

## Estado de ramas

`origin/main` es la rama consolidada de Cloud/K8s en este checkout. Está 8
commits adelante de `upstream/main`:

| Commit | Cambio |
|--------|--------|
| `4bea92f` | Base de dependencias/config del control-plane cloud |
| `78c81bf` | Eliminó `.env.example` de deploy obsoleto |
| `548b689` | Corrigió puerto `DATABASE_URL` en Compose/Supabase |
| `4089947` | Agregó runtime K8s, manifests, scripts y prompts local/VPS |
| `4c96f7e` | Agregó bootstrap bundle, sync credenciales E2E y runtime `/data` |
| `5496b87` | Mejoró soporte de agentes cloud y config runtime |
| `a1a9e57` | Actualizó soporte de modelos agentic y cobertura |
| `85977ca` | Extendió RBAC K8s para ResourceQuota y LimitRange |

Ramas remotas visibles en este clone. La mayoría de heads `upstream/*` son PRs,
releases o experimentos antiguos. No aparecen ancestry-merged en `origin/main`
porque upstream usa historial squash/rebase.

| Rama | Tema del head |
|------|---------------|
| `origin/main` | Consolidación Cloud/K8s actual en este repo |
| `upstream/main` | Baseline release `v0.4.19` |
| `upstream/agents-store-update` | Agente Outbound en Store |
| `upstream/chore/analytics-cleanup` | Limpieza PostHog `email_domain` |
| `upstream/chore/website-legal-pages` | Privacy/Terms del website |
| `upstream/claude/finalize-gh-repo-fix` | Fix CI `GH_REPO` en finalize |
| `upstream/claude/fix-claude-spawn-absolute-path` | Spawn Claude por path absoluto |
| `upstream/claude/fix-sentry-stack-smoke` | Verificación Sentry smoke/symbols |
| `upstream/claude/p2-a` | Migración conversations a engine REST |
| `upstream/claude/quirky-swartz-4aca02` | Toast funcional de bug report |
| `upstream/claude/release-parallelize` | CI release mac/windows paralelo |
| `upstream/export-import-agent` | Compartir/exportar/importar agentes |
| `upstream/feat/website-windows-download` | Downloads Windows en website |
| `upstream/fix-auth-gate-after-setsession` | Cache auth tras `setSession` |
| `upstream/fix-composio-url-and-add-codex-5.5` | URL Composio + modelo Codex |
| `upstream/fix-onboarding-i18n-and-stuck-waiting` | Onboarding i18n/retry Waiting |
| `upstream/fix-provider-settings-ux` | UX de provider settings |
| `upstream/fix-skills-search-and-errors` | Search/errors de skills |
| `upstream/fix-tutorial-and-activity-flashes` | Flashes tutorial/activity |
| `upstream/fix-windows-arm64-claude-installer` | Claude installer Windows ARM64 |
| `upstream/fix-windows-file-summary-and-md` | File summaries/links en Windows |
| `upstream/fix-windows-oauth-implicit-flow` | Callback OAuth implicit Windows |
| `upstream/fix-windows-oauth-single-instance` | OAuth single-instance Windows |
| `upstream/fix-windows-stop-shows-fake-error` | Stop Windows sin error falso |
| `upstream/fix/ci-verify-arch-keys` | Verificación CI arch keys Windows |
| `upstream/fix/ci-windows-arm64-verify` | Verify CLI Windows ARM64 |
| `upstream/fix/ci-windows-cli-deps` | Desbloqueo CLI deps Windows |
| `upstream/gcp-remote-claude-sessions` | Spike abstracción provider/integrations |
| `upstream/jakarta-welcome` | Trabajo de welcome branch |
| `upstream/mission-disappears-after-settings` | Fix persistencia de misiones |
| `upstream/release-0.4.11` a `upstream/release-0.4.19` | Cortes release antiguos |
| `upstream/release/0.4.19` | Rama release upstream actual |
| `upstream/review-broomva-prs` | Docs de estándar de review |
| `upstream/setup-posthog-sentry` | Rama observability setup |
| `upstream/simplify-onboarding-flow` | Sign-in dentro de app |
| `upstream/skill-md-at-repo-root` | Fix instalar `SKILL.md` en root |
| `upstream/tahoe-white-screen-on-launch` | Startup crash gate |
| `upstream/v0.4.16-windows-persistence-skip-tutorial` | Storage/onboarding Windows |
| `upstream/windows-sentry-key-embed` | Env Linear en build Windows |
| `upstream/yc-application-answers` | Docs arquitectura cloud-design |

## Qué agregó la rama cloud

- `cloud/control-plane/`: control-plane Rust/Axum con auth, entitlements,
  metadata de agentes, ciclo de vida runtime, shares, audit events, proxy REST
  y proxy WS.
- `cloud/control-plane/src/runtime.rs`: trait común `RuntimeBackend` para Docker
  y K8s.
- `cloud/control-plane/src/docker_runtime.rs`: un contenedor Docker privado +
  volumen + red por organización.
- `cloud/control-plane/src/k8s_runtime.rs` y `k8s_specs.rs`: un namespace K8s
  por organización, un Deployment/PVC/Secret/Service por agente, quota y limits.
- `cloud/k8s/`: scripts k3d local y K3s/K8s server, overlays, RBAC, smoke,
  teardown y E2E de cuatro agentes.
- `always-on/Dockerfile` y `docker-entrypoint.sh`: imagen reusable del engine
  con Claude Code + Codex en `/opt/houston`, HOME persistente en `/data`.
- `app/src/lib/cloud-client.ts`, `runtime-router.ts`, `engine-for-agent.ts`,
  `activate-agent-runtime.ts`: routing desktop a sidecar local o proxy cloud.
- `app/src/lib/cloud-agent-create.ts` y `cloud-create-plan.ts`: flujo create
  cloud, bootstrap bundle y credential sync post-create.
- `engine/houston-engine-core/src/bootstrap/*` y
  `engine/houston-engine-server/src/routes/bootstrap.rs`: ruta del engine para
  exportar bootstrap bundle desde template Store o agente existente.
- Credenciales provider: infraestructura export/import para
  Anthropic/OpenAI/OpenRouter/Composio, manteniendo API keys dentro del harness
  CLI.
- Debug/QA: UI de cloud orchestration debug, burst runner, status chips y tests
  de proxy, routing runtime, create cloud, entitlements, shares, bootstrap,
  providers y scripts E2E.

## Forma del cluster

```text
Houston App desktop
  -> HTTPS/WS al control-plane (/v1/cloud/*)

Control-plane (houston-system)
  -> Postgres (k3d local in-cluster, VPS usa Supabase/Postgres externo)
  -> RuntimeBackend
      Docker: contenedor + volumen + red Docker por org
      K8s: namespace hou-org-{org_id}
           ResourceQuota + LimitRange
           Deployment hou-cloud-agent-{agent_id}
           PVC hou-cloud-agent-{agent_id}-home
           Secret hou-cloud-agent-{agent_id}-token
           Service hou-cloud-agent-{agent_id}:7777

Engine privado pod/contenedor
  HOME=/data
  HOUSTON_HOME=/data/.houston
  HOUSTON_DOCS=/data/workspace
  HOUSTON_BIND=0.0.0.0:7777
  CLIs: claude, codex
```

URL interna K8s:
`http://hou-cloud-agent-{agent_id}.hou-org-{org_id}.svc.cluster.local:7777`.
El engine no es público. Todo tráfico de app pasa por el proxy del control-plane.

## Flujo de provisionamiento

1. App elige runtime `cloud_24_7` y crea bootstrap bundle llamando al engine local:
   `POST /v1/agents/bootstrap-bundle`.
2. App llama `POST /v1/cloud/agents` con name, config, provider/model, optional
   bootstrap bundle y decisión de sync credenciales.
3. Control-plane valida auth y `cloud_entitlements`, inserta `cloud_agents`.
4. Runtime backend provisiona infra:
   - Docker: asegura red org, crea volumen, corre contenedor engine.
   - K8s: asegura namespace org + quota/limits, crea PVC, aplica Deployment,
     espera Deployment ready y endpoints de Service.
5. Control-plane espera `/v1/health`, asegura workspace `Cloud`, crea agente en
   engine, escribe `CLAUDE.md` y `AGENTS.md`, siembra `.houston/*`, instala
   skills, aplica provider/model config, siembra schemas y migra data.
6. Runtime row guarda `container_name`, `internal_url`, token hash, engine token,
   status y `folder_path` real del engine.
7. Sync credenciales corre después de existir el engine. App abre import session
   en cloud engine, exporta credenciales locales cifradas desde engine local, e
   importa ciphertext al volumen del cloud engine.

## Flujo de requests

- REST app call para cloud agent:
  `HoustonClient -> /v1/cloud/agents/{id}/proxy/v1/... -> private engine /v1/...`
- WS app call:
  `/v1/cloud/agents/{id}/ws -> private engine /v1/ws`
- Control-plane agrega bearer token privado del engine, elimina headers
  incoming de auth/host, conserva query string, audita proxy usage y repara
  segmentos `%2F` decodificados en rutas de sesiones.
- Antes de proxyear, `runtime_wake::ensure_agent_awake` levanta runtime stopped
  o provisioning y espera health.

## Invariantes del harness

- Provider API keys no cambian a HTTP directo contra providers.
- Anthropic API key termina como `ANTHROPIC_API_KEY`; luego `claude -p` corre
  dentro del engine objetivo.
- OpenAI API key termina como `OPENAI_API_KEY`; luego `codex exec` corre dentro
  del engine objetivo.
- OpenRouter API key termina como `OPENROUTER_API_KEY`; luego `codex exec`
  corre con overrides OpenRouter.
- Imagen cloud actual trae Claude Code y Codex. No Gemini CLI. No Composio CLI
  en el `always-on/Dockerfile` actual.
- Activación de app debe correr el harness completo contra el engine resuelto:
  cloud WS, file watcher y routine scheduler.

## Configuración

Env del control-plane:

| Var | Qué controla |
|-----|--------------|
| `DATABASE_URL` | URL Postgres. k3d local usa servicio `postgres:5432`; VPS usa pooler Supabase con SSL. |
| `HOUSTON_CLOUD_BIND` | Bind address, default `0.0.0.0:8788`. |
| `HOUSTON_CLOUD_AUTH` | `local` static token o `jwt`. |
| `HOUSTON_CLOUD_TOKEN` | Bearer estático para local/dev/Dokploy auth local. |
| `HOUSTON_CLOUD_JWT_SECRET` / `SUPABASE_JWT_SECRET` | Requerido para JWT auth. |
| `HOUSTON_CLOUD_LOCAL_USER_ID` | Principal local-mode, default UUID dev all-zero ending `1`. |
| `HOUSTON_ENGINE_IMAGE` | Imagen runtime, default `houston/engine:dev`. |
| `HOUSTON_CLOUD_RUNTIME` | `docker`, `k8s`, `kubernetes` o `k3s`. |
| `DOCKER_HOST` | Socket Docker para backend Docker. |
| `KUBECTL_BIN` | Path de `kubectl` para backend K8s. |

Env desktop:

| Var | Qué controla |
|-----|--------------|
| `VITE_HOUSTON_CLOUD_BASE` | URL pública del control-plane, sin slash final. |
| `VITE_HOUSTON_CLOUD_TOKEN` | Mismo token que servidor cuando usa auth local. |

Secretos viven solo en env files gitignored o UI de deploy:
`cloud/control-plane/deploy/profiles/*.env`, `app/.env.local`, env de Dokploy.

## Flujo local k3d

1. `./cloud/k8s/scripts/bootstrap-local-env.sh`
2. `./scripts/generate-cloud-token.sh`
3. Copiar mismo token a `profiles/local.env` y `app/.env.local`.
4. `./cloud/k8s/scripts/setup-local-k3d.sh`
5. `export HOUSTON_CLOUD_TOKEN=...`
6. `./cloud/k8s/scripts/smoke-local.sh`
7. `cd app && pnpm tauri dev`
8. En app: setear provider API keys, crear agente `Nube 24/7`, mantener sync de
   credenciales activado.

Cluster local expone host `http://127.0.0.1:8788` vía k3d Traefik Ingress. El
Service del control-plane queda ClusterIP.

## Flujo VPS / server

K8s/K3s:

1. Rellenar `cloud/control-plane/deploy/profiles/cloudhouston.blyxlabs.dev.env`
   fuera de git.
2. `./cloud/k8s/scripts/create-control-plane-secret.sh <env-file>`
3. `./cloud/k8s/scripts/build-images.sh`
4. `kubectl apply -k cloud/k8s/overlays/blyxlabs`
5. `kubectl -n houston-system rollout status deployment/houston-control-plane`
6. Smoke `/health` y `/v1/cloud/me`.

Dokploy:

- Usar `cloud/control-plane/deploy/docker-compose.dokploy.yml`.
- Dejar que Dokploy/Traefik maneje puertos 80/443.
- Construir `houston/engine:dev` en VPS antes de crear agentes.
- Montar Docker socket si se usa runtime Docker.
- Setear `VITE_HOUSTON_CLOUD_BASE=https://cloudhouston.blyxlabs.dev` en app.

## Modelo DB

- `organizations`, `organization_members`: org ownership y roles.
- `cloud_entitlements`: estado subscription, max agents, storage, members.
- `cloud_agents`: metadata, owner, parent/worker link, runtime mode.
- `cloud_agent_runtimes`: URL privada, token hash, engine token, status.
- `cloud_agent_shares`: permisos viewer/operator/admin.
- `audit_events`: create, bootstrap, proxy, credential sync, lifecycle.

Nota worktree actual: hay cambios no commit en archivos locales que ajustan cap
de entitlements a `max_cloud_agents=5`, `max_storage_gb=50`, y agregan
`003_entitlements_default_cap.sql`. No tratarlos como estado committed hasta que
se stage/commit.

## Checks

Checks agregados o usados por esta rama:

- `cargo test -p houston-cloud-control-plane`
- Tests control-plane: proxy path, bootstrap bundle, sharing access,
  entitlements, worker agents.
- Tests engine: bootstrap route/provider credential routes.
- Tests app: cloud create, runtime routing, cloud WS, debug burst,
  provider reconnect state, engine-agent path.
- Smoke local script: `cloud/k8s/scripts/smoke-local.sh`.
- E2E cuatro agentes: `cloud/k8s/scripts/e2e-four-agents.sh` crea agentes Store,
  verifica pods, bootstrap files, routines, skills, activity seed y persistencia
  de pods tras burst proxy calls.

## Gaps conocidos

| Gap | Mitigación actual |
|-----|-------------------|
| Gemini ausente en imagen cloud | Usar Anthropic, OpenAI/Codex u OpenRouter. |
| Composio CLI ausente en imagen actual | Storage de credenciales puede sincronizar, pero ejecución Composio desde agente necesita trabajo de imagen. |
| WS revoke mid-session | Acceso se valida al conectar. |
| Cleanup TTL de workers | Schema existe; cleanup programado sigue TODO en código. |
| Portable export cloud | Export local sigue primario; cloud usa share wizard live. |
| Docker networking en Dokploy | Si control-plane no alcanza red de agente, usar K8s o corregir internal URL/network join. |

## Checklist para código cloud nuevo

1. Calls agent-scoped usan `resolveEngine(agent)` o `resolveEngineForPath`, no
   `getEngine()` directo.
2. Calls cloud usan `cloudEngineBaseUrl(agent.id)` y cloud bearer auth.
3. Eventos usan cloud WS vía `ensureAgentEngineWs`.
4. Activación inicia watcher y routine scheduler en engine resuelto.
5. Provider auth sigue como credential import/export hacia volumen del engine.
6. Cambios K8s cubren manifests, RBAC, overlay local, overlay server y smoke.
7. Todo error cloud user-visible pasa por `showErrorToast`.
8. Tests cubren selección local/cloud, no solo happy path local sidecar.
