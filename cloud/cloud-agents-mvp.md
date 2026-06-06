# Houston Cloud Agents MVP

## Objetivo

Construir un MVP funcional, no una demo falsa: el usuario crea un agente desde el mismo flujo actual de Houston y puede elegir si vive en este computador o en la nube 24/7. Si elige nube, Houston provisiona un `houston-engine` real en un contenedor privado del VPS, conserva su volumen, enruta REST/WS desde la UI actual, valida entitlement de suscripcion, y permite compartir acceso sin crear otra interfaz.

## Reglas Del MVP

- Reusar la UI actual de creacion de agente.
- Agregar la eleccion `Este computador` / `Nube 24/7` en el paso de nombre y color.
- No exponer engines cloud al internet. Solo el control plane es publico.
- No reautenticar manualmente al usuario en el pod.
- Sincronizar credenciales de proveedor con el flujo seguro definido en `cloud/cloud-agent-sync-migration-plan.md`.
- Un cloud agent es 24/7: container con restart policy, volumen propio y token interno.
- Sharing cloud da acceso al agente vivo. Portable sigue siendo para plantillas.
- Docker en VPS primero. K3s despues, sin rehacer UI ni contrato.

## Arquitectura Inicial

```text
Houston App
  local houston-engine actual
  cloud client
    -> cloud control-plane
      -> Supabase auth
      -> cloud_entitlements
      -> sharing/RBAC
      -> Docker runtime worker
        -> houston-engine container per cloud agent
          -> private network
          -> private volume
          -> internal engine token
```

## Aprovechamiento Del Repo

- `always-on/Dockerfile`: imagen base del engine cloud.
- `app/src/lib/engine.ts`: ya soporta cliente HTTP/WS por `baseUrl` y `token`.
- `ui/engine-client`: contrato TypeScript del engine.
- `app/src/components/shell/create-workspace-dialog.tsx`: flujo actual de crear agente.
- `app/src/components/shell/naming-step.tsx`: paso correcto para elegir local/cloud.
- `app/src/components/shell/ai-review-step.tsx`: mismo selector para flujo "Crear con IA".
- `app/src/components/portable/export-wizard.tsx`: base para compartir o publicar plantilla.
- `knowledge-base/auth.md`: provider login headless ya documentado.

## Fase 1 - Control Plane

Crear `cloud/control-plane/` con Rust/Axum.

Modulos: `auth`, `db`, `entitlements`, `agents`, `docker_runtime`, `proxy`, `ws_proxy`, `shares`, `audit`.

Rutas: `cloud/me`, `cloud/entitlements`, `cloud/agents`, `cloud/agents/shared`, `cloud/agents/:id/status`, `cloud/agents/:id/restart`, `cloud/agents/:id/shares`, `cloud/agents/:id/proxy/*`, `cloud/agents/:id/ws`.

## Fase 2 - Modelo De Datos

Tablas:

- `organizations`: empresa o workspace comercial.
- `organization_members`: usuario, organizacion, rol.
- `cloud_entitlements`: plan, estado, limites.
- `cloud_agents`: metadata visible en UI.
- `cloud_agent_runtimes`: container, target URL interno, token hash, estado.
- `cloud_agent_shares`: usuario invitado, rol, agente.
- `cloud_provider_connections`: estado por provider dentro del container.
- `audit_events`: acciones permitidas, denegadas y operativas.

Entitlement minimo:

```text
status: active | past_due | canceled
max_cloud_agents: number
max_storage_gb: number
max_members: number
```

Primer corte: seed/admin manual. Segundo corte: Stripe webhooks.

## Fase 3 - Runtime Docker En VPS

Cada agente cloud crea container, volume, network, token interno, limits y restart policy. Detalle operativo en `cloud/cloud-agents-ops.md`.

El control plane guarda target interno:

```text
http://hou-cloud-agent-{agent_id}:7777
```

Solo el control plane entra a esa red. El engine no publica puerto al host.

## Fase 4 - Proxy REST/WS

El control plane:

1. valida Supabase JWT
2. valida entitlement
3. valida permiso sobre agente
4. lee token interno del runtime
5. reenvia request al engine privado
6. registra audit event
7. devuelve respuesta tal como engine

Para WS, el control plane abre un WS interno contra el engine y puentea frames. Si el usuario pierde permiso, corta conexion.

## Fase 5 - Frontend

Cambios de tipos:

- `ui/engine-client/src/types.ts`: `Agent.runtime?: "local" | "cloud_24_7"`
- `app/src/lib/types.ts`: mismo campo.
- `CreateAgent`: agregar `runtime?: "local" | "cloud_24_7"`.

Cambios de UI:

- `app/src/components/shell/naming-step.tsx`: selector local/cloud bajo el input de nombre.
- `app/src/components/shell/ai-review-step.tsx`: mismo selector para flujo con IA.
- `app/src/components/shell/create-workspace-dialog.tsx`: estado `runtimeMode`.
- `app/src/stores/agents.ts`: combinar agentes locales y cloud.
- `app/src/lib/cloud-client.ts`: cliente del control plane.
- `app/src/lib/runtime-router.ts`: decide local engine o cloud proxy por agente.

Texto UI:

```text
Donde vivira este agente
Este computador
Nube 24/7
```

Agregar llaves i18n en `shell.json` para `en`, `es`, `pt`.

## Fase 6 - Provider Credentials En Cloud

No pedir reauth manual cuando la credencial local sea exportable.

Flujo:

1. usuario crea agente cloud
2. container arranca
3. UI selecciona provider/model como hoy
4. app genera bootstrap bundle con instrucciones, seeds y skills
5. local engine exporta credenciales permitidas en bundle cifrado
6. cloud engine importa credenciales en su volumen persistente
7. UI pregunta status al engine cloud
8. agent queda operativo 24/7

## Fase 7 - Sharing

Roles:

- `viewer`: puede ver actividad/conversaciones permitidas.
- `operator`: puede hablar con el agente.
- `admin`: puede compartir, reiniciar, borrar.

Reusar `ExportAgentWizard`:

- agente local: exportar `.houstonagent`
- agente cloud: compartir acceso
- publicar plantilla: generar `.houstonagent` y subir como package

## Orden De Desarrollo

1. Crear control plane con auth, entitlement y DB.
2. Provisionar container real desde control plane.
3. Proxy `/v1/health` y `/v1/version` del engine cloud.
4. Proxy sesiones REST y WS.
5. Agregar selector Local/Nube en `NamingStep`.
6. Crear cloud agent desde el wizard.
7. Listar local + cloud juntos.
8. Hacer provider login contra engine cloud.
9. Compartir cloud agent.
10. Subir a VPS con TLS.

## Gates De Verificacion

- Crear agente local sigue funcionando.
- Crear agente cloud crea container real.
- Engine cloud responde `/v1/health` solo por control plane.
- Houston UI puede chatear con agente cloud.
- Provider login persiste en volumen cloud.
- Reiniciar container no pierde agente ni credenciales.
- Usuario sin entitlement no puede crear cloud agent.
- Usuario sin share no puede ver ni hablar con agente ajeno.
- Agente local no se rompe por campos cloud.

## Apéndices

- Operacion VPS y migracion K3s: `cloud/cloud-agents-ops.md`.
- Backlog estructurado: `cloud/cloud-agent-features.json`.
