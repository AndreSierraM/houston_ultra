# Houston Cloud Agents Ops

## VPS Deploy

Servicios en VPS:

- Caddy o Traefik con TLS.
- `cloud-control-plane`.
- Docker daemon local.
- Postgres.
- Redis opcional para WS/routing.
- backups de volumes.

Puertos:

- publico: 443.
- privado: engine containers.

## Docker Runtime

Cada agente cloud corre con:

```text
container: hou-cloud-agent-{agent_id}
volume: hou-cloud-agent-{agent_id}-home mounted at /data
network: hou-org-{org_id}
restart: unless-stopped
```

Variables:

```text
HOUSTON_HOME=/data/.houston
HOUSTON_DOCS=/data/workspace
HOUSTON_ENGINE_TOKEN=<random>
HOUSTON_BIND=0.0.0.0:7777
HOUSTON_BIND_ALL=1
HOUSTON_NO_PARENT_WATCHDOG=1
```

El volumen debe cubrir `/data` completo, no solo `/data/.houston`. Provider homes
como `.codex`, `.claude` y el workspace bajo `/data/workspace` deben persistir
despues de restart/recreate.

La imagen `houston/engine:dev` incluye solo **Claude Code** y **Codex** (sin Gemini
CLI ni Composio). Conectar providers via API key en la app desktop; se sincronizan
al pod al crear agente Nube 24/7.

## VPS Provision

Pasos:

1. Instalar Docker y Compose plugin.
2. Crear usuario `houston-cloud`.
3. Crear directorio `/opt/houston-cloud`.
4. Configurar Caddy/Traefik con TLS.
5. Levantar Postgres.
6. Levantar control plane.
7. Probar `GET /v1/cloud/me`.
8. Crear agente cloud desde UI.
9. Verificar container privado.
10. Verificar chat desde Houston App.

## Seguridad VPS

- Solo publicar 443.
- No publicar puertos de engines.
- Token interno por engine.
- Volumen por agente.
- Network por organizacion.
- Audit log para cambios de share, create, delete, restart y 403.
- Backups diarios de Postgres.
- Backups diarios de volumes.

## K8s / K3s (implementado)

Runtime seleccionable con `HOUSTON_CLOUD_RUNTIME=k8s|docker`.

1. Manifests: `cloud/k8s/base` + overlay `cloud/k8s/overlays/blyxlabs`.
2. Backend: `k8s_runtime.rs` + `k8s_specs.rs` (namespace por org, Deployment/PVC/Secret/Service por agente).
3. Imagen control-plane: `cloud/control-plane/Dockerfile.k8s` (incluye kubectl).
4. Operacion VPS: copiar prompt de `cloud/k8s/PROMPT-SERVER.md` en agente IA del servidor.

Pendiente endurecimiento: NetworkPolicy default deny, ResourceQuota/LimitRange por namespace org.

La UI no cambia. El contrato cloud agent sigue igual.

## Gates K3s

- Namespace creado por organizacion.
- Engine no recibe trafico externo directo.
- Solo control plane puede hablar con engine.
- PVC persiste despues de recrear pod.
- ResourceQuota bloquea exceso.
- Usuario sin share recibe 403.

## Scaling model

Houston cloud escala en **tres ejes**. No mezclar.

### Eje A: multi-agente horizontal (modelo principal)

**Regla:** 1 agente = 1 identidad = 1 pod/container = 1 volumen = 1 `.houston/`.

- Escala = **mas agentes**, no mas replicas K8s del mismo Deployment.
- Cada org vive en `hou-org-{org_id}`. Cada agente cloud es un Deployment
  `hou-cloud-agent-{agent_id}` con PVC RWO dedicado.
- Tareas pesadas: un **coordinador** despacha a **N workers** (N agent_ids
  distintos, N pods). Mission Control, skills Store (dispatch paralelo) y
  multiples sesiones board ya apuntan a este patron.
- Limite de negocio: `cloud_entitlements.max_cloud_agents` (Postgres).
- Limite de cluster: ResourceQuota por namespace org (pendiente).

Ejemplo (50 PDFs): 1 agente bookkeeping coordinador + 10 workers bookkeeping
(10 creates cloud), no `replicas: 10` en un solo Deployment.

### Eje B: paralelismo intra-pod (ya existe)

Un solo `houston-engine` admite hasta **15** procesos CLI concurrentes
(`houston-terminal-manager` session semaphore, default 15).

- Workdirs **distintos** corren en paralelo (varias misiones board a la vez).
- Mismo workdir se serializa (`acquire_workdir`) para no corromper `.houston/`.
- Util para carga media en un solo agente. No sustituye workers aislados para
  jobs masivos con aislamiento fuerte.

### Eje C: replicas K8s del mismo agent_id (rechazado)

`replicas > 1` en el Deployment actual **no es viable** sin rediseño:

| Bloqueador | Motivo |
|------------|--------|
| PVC ReadWriteOnce | Solo un pod monta el home del agente |
| Estado en memoria | Sesiones WS, semaforo CLI, locks de workdir |
| Proxy 1:1 | Control plane enruta a una `internal_url` por agent_id |

Si alguna vez se necesita "varias copias del mismo agente", crear **N agent_ids**
(clon de `config_id`), no subir replicas del Deployment.

### Runtime modes (actual + planeado)

| Mode | Compute | Datos | Uso |
|------|---------|-------|-----|
| `local` | Sidecar desktop | `~/.houston/` local | Default |
| `cloud_24_7` | Pod siempre encendido (replicas 1) | PVC 10Gi | Implementado |
| `cloud_on_demand` | Scale to zero (replicas 0 idle, 1 al abrir) | PVC conservado | API stop/start implementada |
| `cloud_worker` | Pod efimero, TTL post-job | PVC opcional | Pendiente |

`cloud_on_demand` ahorra coste idle. Control plane expone:

- `POST /v1/cloud/agents/:id/stop` (admin): scale deployment a 0 / `docker stop`; PVC/volume intacto; DB `status=stopped`.
- `POST /v1/cloud/agents/:id/start` (admin): scale a 1 + rollout + endpoints / `docker start`; DB `status=running`.

**Follow-up:** wake-on-proxy (auto-`start` en REST/WS proxy cuando `status=stopped`) no esta cableado aun; hoy hay que llamar `start` antes de chatear.

### Recursos por agente (K8s, hoy)

```text
requests: cpu 250m, memory 512Mi   (reserva scheduler)
limits:   cpu 2,    memory 2Gi
storage:  PVC 10Gi RWO per agent
```

Capacidad aproximada por nodo: `floor(nodo_cpu / 0.25)` agentes antes de Pending
(solo requests; limits permiten burst). k3d local = 1 nodo. Prod necesita node
autoscaling ademas de quota por org.

### ResourceQuota por org (pendiente, prioridad 1)

Al crear `hou-org-{org_id}`, aplicar junto al Namespace:

```text
requests.cpu:           max_cloud_agents × 250m
requests.memory:        max_cloud_agents × 512Mi
limits.cpu:             max_cloud_agents × 2
limits.memory:          max_cloud_agents × 2Gi
persistentvolumeclaims: max_cloud_agents
requests.storage:       max_storage_gb Gi
```

Mas LimitRange default por pod. Enforcement en K8s, no solo en app.

### Worker burst (pendiente, prioridad 2)

API y lifecycle para jobs pesados:

1. Coordinador (humano o agente) pide N workers via control plane.
2. Control plane crea N agentes `runtime: cloud_worker` (mismo `config_id`,
   distinto `agent_id`, opcional `parent_agent_id`).
3. Workers ejecutan y reportan; al terminar, teardown pod (conservar o borrar PVC
   segun politica).
4. ResourceQuota + `max_cloud_agents` acotan el burst.

### Orden de implementacion

1. ResourceQuota + LimitRange en `ensure_namespace` (`k8s_specs.rs`).
2. Stop/wake (`cloud_on_demand`): scale deploy 0/1, status `stopped`, wake en
   primer proxy/WS.
3. Worker burst API + runtime mode `cloud_worker`.
4. Node autoscaling en prod (EKS/GKE). Fuera de scope k3d local.

### Verificacion rapida en Lens / kubectl

```bash
kubectl get pods -A | grep hou-cloud-agent
kubectl get ns | grep hou-org
```

Filtrar **all namespaces**. Los agentes no viven en `default` ni `houston-system`.
