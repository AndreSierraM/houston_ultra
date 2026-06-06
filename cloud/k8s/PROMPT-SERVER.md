# Prompt para IA en el VPS (K3s / Kubernetes)

**Repo público.** Nunca commitear ni pegar en issues/PRs: `DATABASE_URL`, `SUPABASE_DB_PASSWORD`, `HOUSTON_CLOUD_TOKEN`. Esos valores viven solo en el VPS y en `app/.env.local` (gitignored).

Plantilla segura en repo: `cloud/control-plane/deploy/profiles/cloudhouston.blyxlabs.dev.env.example`  
Archivo real (local): `profiles/cloudhouston.blyxlabs.dev.env` (gitignored).

Copia el bloque siguiente en tu agente de IA en el servidor. El agente debe ejecutar los pasos en orden y reportar fallos con logs.

---

## Prompt (copiar desde aquí)

```
Eres el operador de despliegue de Houston Cloud en este VPS.

Objetivo: dejar funcionando el control plane en Kubernetes con runtime k8s, conectado a Supabase Postgres externo, accesible en https://cloudhouston.blyxlabs.dev

Repositorio: https://github.com/AndreSierraM/houston_ultra
Rama: main (o la que indique el operador)

Requisitos previos en el servidor:
- kubectl funciona contra el cluster (K3s u otro)
- docker disponible para build de imágenes locales
- DNS cloudhouston.blyxlabs.dev apunta a este servidor
- Ingress controller activo (Traefik en K3s por defecto)

NO uses Docker Compose para el control plane.
NO despliegues Postgres en el cluster.
NO crees pods de agentes a mano; el control plane los crea al provisionar desde la app desktop.

### Paso 1 — Clonar o actualizar repo

cd ~
git clone https://github.com/AndreSierraM/houston_ultra.git || true
cd houston_ultra
git pull

### Paso 2 — Configurar secret del control plane

El operador DEBE entregarte credenciales por canal privado (no en git).

En el servidor, crear el env local (no commitear):

cp cloud/control-plane/deploy/profiles/cloudhouston.blyxlabs.dev.env.example \
   cloud/control-plane/deploy/profiles/cloudhouston.blyxlabs.dev.env
# Editar .env con valores reales que el operador pegue en el chat privado del VPS

Generar token si hace falta: echo "hst_$(openssl rand -hex 32)"

Aplicar secret (excluye variables VITE_*):

chmod +x cloud/k8s/scripts/create-control-plane-secret.sh
./cloud/k8s/scripts/create-control-plane-secret.sh cloud/control-plane/deploy/profiles/cloudhouston.blyxlabs.dev.env

### Paso 3 — Build e import de imágenes

chmod +x cloud/k8s/scripts/build-images.sh
./cloud/k8s/scripts/build-images.sh

Esto construye:
- houston/engine:dev (runtime de cada agente cloud)
- houston/control-plane:dev (API + kubectl embebido)

En K3s las imágenes se importan con ctr. Si el cluster usa registry remoto, push e actualiza HOUSTON_ENGINE_IMAGE en el secret.

### Paso 4 — Aplicar manifests Kubernetes

kubectl apply -k cloud/k8s/overlays/blyxlabs

Verificar:
kubectl -n houston-system get pods,svc,ingress
kubectl -n houston-system rollout status deployment/houston-control-plane --timeout=180s

### Paso 5 — Smoke tests

curl -sf https://cloudhouston.blyxlabs.dev/health
# debe responder: ok

# El operador exporta el token en la sesión (no imprimirlo en logs públicos)
curl -sf -H "Authorization: Bearer $HOUSTON_CLOUD_TOKEN" https://cloudhouston.blyxlabs.dev/v1/cloud/me

### Paso 6 — Conectar app desktop (Mac del operador)

En app/.env.local del Mac (no en el servidor):

VITE_HOUSTON_CLOUD_BASE=https://cloudhouston.blyxlabs.dev
VITE_HOUSTON_CLOUD_TOKEN=<mismo HOUSTON_CLOUD_TOKEN del perfil>

Reiniciar app: pnpm tauri dev

En la app: crear agente "Nube 24/7". El control plane debe:
1. Crear namespace hou-org-{org_id}
2. Crear Deployment + PVC + Service del engine
3. Bootstrap workspace Cloud en el engine

Verificar agente:
kubectl get deployments -A | grep hou-cloud-agent

### Troubleshooting

- Pod ImagePullBackOff: re-ejecutar build-images.sh o configurar imagePullPolicy IfNotPresent
- /health 404: ingress o DNS incorrecto; revisar kubectl -n houston-system describe ingress
- DB error: DATABASE_URL debe usar pooler Supabase puerto 5432 con sslmode=require
- Provision falla: kubectl logs -n houston-system deployment/houston-control-plane

Entrega final: URLs smoke OK, pod control-plane Running, instrucciones para el operador si falta TLS/certificado.
```

---

## Arquitectura desplegada

```text
Ingress (cloudhouston.blyxlabs.dev)
  → Service houston-control-plane:8788 (namespace houston-system)
      → Pod control-plane (kubectl + RBAC)
          → Postgres Supabase (externo)
          → Por cada agente cloud (dinámico):
              Namespace hou-org-{org_id}
              Deployment + PVC + Secret + Service :7777
              Imagen houston/engine:dev
```

## Archivos clave

| Ruta | Uso |
|------|-----|
| `cloud/k8s/overlays/blyxlabs/` | Kustomize del despliegue |
| `cloud/k8s/scripts/build-images.sh` | Build engine + control-plane |
| `cloud/k8s/scripts/create-control-plane-secret.sh` | Secret desde perfil env |
| `cloud/control-plane/Dockerfile.k8s` | Imagen control-plane con kubectl |
| `cloud/control-plane/deploy/profiles/cloudhouston.blyxlabs.dev.env.example` | Plantilla (repo) |
| `cloud/control-plane/deploy/profiles/cloudhouston.blyxlabs.dev.env` | Secretos reales (gitignored, solo VPS/Mac) |
