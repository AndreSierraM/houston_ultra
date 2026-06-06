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
volume: hou-cloud-agent-{agent_id}-home
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
