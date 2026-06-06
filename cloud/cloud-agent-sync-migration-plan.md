# Cloud Agent Sync Migration Plan

## Objetivo

Crear agentes cloud 24/7 desde la UI actual, sin reautenticar manualmente en el pod, copiando de forma segura la configuracion real que hace funcionar a un agente Houston local: instrucciones, schemas, skills, seeds, provider/model, credenciales de provider exportables, Composio cuando aplique, y volumen persistente completo.

## Principios

- No crear otra app ni otro wizard. Reusar `CreateAgentDialog`, `NamingStep`, `AiReviewStep`, provider dialogs y sharing actual.
- El usuario no ve palabras como pod, token, PVC, JSON, Secret o filesystem.
- Cloud agent usa un `houston-engine` real, no un backend paralelo.
- Control plane nunca guarda credenciales de provider en texto plano.
- Cada agente cloud tiene su propio runtime, volumen, token interno y audit log.
- Skills, `CLAUDE.md`, `AGENTS.md`, `GEMINI.md`, `.houston/` y provider homes deben sobrevivir restart/recreate.

## Problemas Encontrados

1. `DockerRuntime` monta solo `/data/.houston`; eso pierde `~/.codex`, `~/.claude`, `~/.gemini`, `~/.composio` y `HOUSTON_DOCS=/data/workspace`.
2. `always-on/Dockerfile` instala CLIs bajo `/data/.local` y `/data/.composio`; montar `/data` completo taparia esos binarios.
3. Cloud bootstrap no pasa `installedPath` ni copia packaged skills de Store, entonces un Store agent cloud puede nacer sin sus Skills.
4. Provider auth local vive en archivos/env propios del CLI, no en Houston account.
5. UI ya enruta provider/status/login por `resolveEngine(currentAgent())`; conviene extender ese camino, no crear UI nueva.

## Arquitectura Target

Houston App usa el local engine para exportar `AgentBootstrapBundle` y `ProviderCredentialBundle` cifrado. El cloud client envia eso al control plane, que valida auth/entitlement/RBAC, audita, proxy-pasa secretos sin decrypt, y habla con un engine privado por agente. El pod corre con `HOME=/data`, `HOUSTON_HOME=/data/.houston`, `HOUSTON_DOCS=/data/workspace`.

## Contrato Del Pod

Montar un volumen persistente por agente en `/data`.

Contenido esperado: `/data/.houston/` para engine DB/prefs/API-key env; `/data/workspace/Cloud/<Agent>/` para agent root; `.houston`, `.agents/skills`, `.claude/skills`, `CLAUDE.md`, `AGENTS.md`, `GEMINI.md` dentro del agent root; provider homes en `/data/.codex`, `/data/.claude`, `/data/.gemini`, `/data/.composio`.

Cambios Docker/K8s:

- Mover CLIs de `/data` a `/opt/houston/bin` o `/usr/local/bin`.
- `PATH=/opt/houston/bin:/usr/local/bin:...`
- Mantener `HOME=/data`.
- PVC/volume unico en `/data`, no solo subPath `/data/.houston`.
- Permisos: `/data` `0700`, secretos `0600`, owner `houston`.
- K8s: PVC por agente, Secret solo para `HOUSTON_ENGINE_TOKEN`, NetworkPolicy default deny, ResourceQuota/LimitRange por namespace.

## AgentBootstrapBundle

Nuevo bundle generado por el local engine antes de crear cloud agent: `configId`, `name`, `color`, `claudeMd`, `seeds`, `skills[{slug, skillMd}]`, `configPatch{provider, model, effort}` y `source{kind, id, version}`.

Reglas:

- No incluir activity seeds por defecto.
- Copiar packaged skills de Store igual que local create.
- Escribir `.agents/skills`, luego llamar `listSkills` o helper para crear `.claude/skills`.
- Llamar `seedAgentSchemas` y `migrateAgentFiles` despues del import.
- Para migrar un agente local existente a cloud, reutilizar inventario portable sin anonimizar: `CLAUDE.md`, skills, routines y learnings.

## ProviderCredentialBundle

Nuevo flujo sin reauth manual:

1. Cloud engine crea una sesion de importacion con public key de un solo uso.
2. Local engine lee solo credenciales permitidas del provider actual.
3. Local engine cifra el bundle para esa public key.
4. App envia ciphertext al control plane.
5. Control plane proxy passthrough al cloud engine, no decrypt.
6. Cloud engine descifra, valida, escribe paths esperados, hace `checkStatus`.

Payload conceptual: `provider`, `authKind`, `files[{relPath, mode, contents}]`, `checksum`, `createdAt`, `expiresAt`. Se cifra end-to-end para el cloud engine, con import session de un solo uso.

Paths permitidos:

- `openai`: `.codex/auth.json`, `.houston/providers/openai/.env`
- `anthropic`: `.claude/.credentials.json`, `.houston/providers/anthropic/.env`
- `gemini`: `.gemini/oauth_creds.json`, `.gemini/google_accounts.json`, `.gemini/settings.json`, `.houston/providers/gemini/.env`
- `openrouter`: `.houston/providers/openrouter/.env`
- `composio`: `.composio/user_data.json` solo si formato validado y usuario eligio sincronizar integraciones.

Si una credencial vive solo en Keychain/Credential Manager, implementar adapter nativo por plataforma. Si el provider no permite export, mostrar error claro y no fingir exito.

## Cambios Backend

### Engine

- Agregar `provider_credentials::{export, import, import_session}` en `engine/houston-engine-core/src/provider/`.
- Agregar rutas:
  - `POST /v1/providers/:name/credential-import/session`
  - `POST /v1/providers/:name/credential-export`
  - `POST /v1/providers/:name/credential-import`
- Agregar tipos wire en `houston-engine-protocol` y `ui/engine-client`.
- Reusar validadores existentes de API keys.
- Agregar allowlist de paths y parser JSON por provider.
- Emitir evento `ProviderCredentialsSynced`.

### Control Plane

- Extender `CreateCloudAgent` con `bootstrapBundle` y `credentialSync`.
- En `engine_provision.rs`, crear agente con bundle completo, no solo `claudeMd`.
- Agregar `cloud_provider_syncs` o audit event estructurado para estado, sin valores secretos.
- Proteger sync por rol `admin` del agente.
- Proxy no debe loguear body en rutas de credential import.

### Runtime

- Cambiar `docker_runtime.rs`: `-v volume:/data`, `HOME=/data`, `HOUSTON_DOCS=/data/workspace`.
- Cambiar `k8s_specs.rs`: mount PVC en `/data`, no `subPath: houston`.
- Cambiar `always-on/Dockerfile`: CLIs fuera de `/data`; Composio bin resoluble desde PATH.
- Ajustar tests para validar mountPath y env.

## Cambios Frontend

- `cloud-client.ts`: `createCloudAgent` recibe `bootstrapBundle` y `syncProviderCredentials`.
- `stores/agents.ts`: antes de `createCloudAgent`, pedir al local engine `buildBootstrapBundle`.
- `CreateAgentDialog`: default para cloud = sincronizar conexion actual, sin texto tecnico.
- `RuntimeModeSelector`: copy simple: "Usar en la nube 24/7" y "Usar mi conexion actual".
- `ProviderPicker`, `ProviderSettings`, `CliConnectDialog`: no decidir remote por `osIsTauri`; decidir por `isCloudAgent(currentAgent())`.
- `ExportAgentWizard`: para cloud agent mostrar compartir; para local agent agregar "Mover a nube" fase 2.
- i18n en `en/es/pt`, sin strings literales.

## Reparto De Agentes

### Arquitectura, 2 agentes

- A1 Runtime Contract: define `/data`, CLI relocation, K8s/Docker parity, rollback.
- A2 Secret Contract: define encryption, path allowlist, platform adapters, audit redaction.

### Backend, 10 agentes

- B1 Docker runtime migration: `always-on/Dockerfile`, `docker_runtime.rs`, compose docs.
- B2 K8s runtime migration: `k8s_specs.rs`, `k8s_runtime.rs`, NetworkPolicy/Quota.
- B3 Engine credential import session: one-time key, expiry, tests.
- B4 Engine provider export: OpenAI, Anthropic, Gemini, OpenRouter bundle readers.
- B5 Engine provider import: path validation, permissions, status probe.
- B6 AgentBootstrapBundle builder: Store skills, seeds, CLAUDE.md, config patch.
- B7 Cloud provision bootstrap: control plane creates cloud agent with full bundle.
- B8 Composio sync spike: validate if `user_data.json` portable enough.
- B9 Audit/RBAC: admin-only sync, redacted audit events, no body logs.
- B10 Protocol/types/tests: Rust protocol, TS client, route tests.

### Frontend, 10 agentes

- F1 UX copy: nontechnical labels for cloud sync and failures.
- F2 CreateAgentDialog state: cloud sync default-on, errors inline.
- F3 RuntimeModeSelector UI: compact cloud section, no extra wizard.
- F4 Bootstrap orchestration: local export -> cloud create -> credential sync.
- F5 Provider status UX: post-create status and success/failure toast.
- F6 Shared engine routing: provider calls use runtime, not `osIsTauri`.
- F7 Store skills parity UI check: cloud Store agents show same skills.
- F8 Share/move surfaces: cloud share and later local-to-cloud entry.
- F9 i18n: `shell.json`, `providers.json`, `portable.json` en/es/pt.
- F10 Visual regression: creation modal desktop/mobile sizes.

### QA, 4 agentes

- QA1 Runtime persistence: restart container/pod, credentials and skills remain.
- QA2 Security/RBAC: viewer/operator/admin, no cross-agent access, no secret logs.
- QA3 Provider matrix: OpenAI, Claude, Gemini, OpenRouter API key/OAuth paths.
- QA4 UI E2E: local create unaffected, cloud create, Store skills, sharing.

## Fases

1. Runtime home migration. Gate: pod restart no pierde `.codex`, `.claude`, `.gemini`, skills ni workspace.
2. Bootstrap bundle. Gate: Store agent cloud nace con mismas skills que local.
3. Credential sync engine. Gate: provider status cloud queda authenticated sin login manual.
4. UI orchestration. Gate: crear cloud agent desde NamingStep hace todo en una sola accion.
5. Sharing and RBAC. Gate: compartido puede operar sin ver secretos ni otros agentes.
6. K3s hardening. Gate: NetworkPolicy, quotas, PVC, restart y audit validados.

## Gates De Aceptacion

- Local create sigue igual.
- Cloud create con Store agent conserva `CLAUDE.md`, schemas, skills y config.
- Cloud create con provider conectado localmente queda autenticado en cloud sin reauth manual.
- Restart/recreate conserva credenciales y agent files.
- Control plane nunca guarda plaintext secrets.
- Usuario sin admin no puede sync credentials.
- Usuario compartido no puede exportar ni ver credenciales.
- Logs no contienen API keys, refresh tokens, auth JSON ni bundle bodies.
- `cargo test --workspace` y `cd app && pnpm tsc --noEmit` pasan.

## Riesgos

- Keychain/Credential Manager puede no exponer ciertos tokens. Resolver con adapter por plataforma o error claro.
- Copiar OAuth refresh tokens puede violar supuestos de algun provider. Mantener allowlist, auditoria y kill switch por provider.
- Montar `/data` completo tapa CLIs si no se mueven antes. Esa migracion va primero.
- Composio puede atar sesion a device. Tratar como spike separado, no bloquear provider LLM.
