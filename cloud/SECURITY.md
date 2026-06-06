# Houston Cloud — secretos (repo público)

## Qué nunca va en git

- `DATABASE_URL` / `SUPABASE_DB_PASSWORD`
- `HOUSTON_CLOUD_TOKEN` / `VITE_HOUSTON_CLOUD_TOKEN`
- `app/.env.local`
- `cloud/control-plane/deploy/.env`
- `cloud/control-plane/deploy/profiles/*.env` (excepto `*.env.example`)

## Dónde van los valores reales

| Entorno | Archivo / destino |
|---------|-------------------|
| VPS K8s | `profiles/cloudhouston.blyxlabs.dev.env` → `kubectl` Secret |
| VPS Dokploy | Variables en panel Dokploy |
| Mac desktop | `app/.env.local` |

Plantilla: `cloud/control-plane/deploy/profiles/cloudhouston.blyxlabs.dev.env.example`

## Si un secreto estuvo en git

1. **Rotar de inmediato** (nuevo valor en Supabase / nuevo `HOUSTON_CLOUD_TOKEN`).
2. Actualizar VPS Secret, Dokploy y `app/.env.local`.
3. El historial de git sigue visible en GitHub aunque borres el archivo; la rotación es obligatoria.

Generar token nuevo:

```bash
echo "hst_$(openssl rand -hex 32)"
```

## Referencias en docs

Usar placeholders (`<tu token>`, `hst_<openssl rand -hex 32>`). No copiar tokens reales en markdown commiteado.
