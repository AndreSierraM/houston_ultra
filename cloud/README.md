# Houston Cloud

Managed deployment service for Houston Engine. We run it for you.

## What it is
Dev builds a product on Houston Engine. Doesn't want to run infra. Pushes to Houston Cloud. Cloud provisions + monitors + bills. Dev focuses on agents.

## Status
**MVP en desarrollo.** Ver `cloud-agents-mvp.md`, `cloud-agent-features.json`, `cloud-agents-ops.md`.

- `control-plane/` — Rust/Axum: auth Supabase, entitlements, agentes cloud, Docker runtime, proxy REST/WS.
- Fases 1-4 implementadas en repo; VPS deploy en `control-plane/deploy/docker-compose.yml`.
- K3s (fase 8) pendiente.

## Relation to other products
- Hosts **Houston Engine** instances for third-party devs
- Always On + Teams could run on Cloud (dogfooding) OR on separate infra
- This is the **revenue engine** for Houston as a company

## Unknowns to solve
- Multi-tenant isolation strategy (VM per customer? container per customer?)
- Pricing model (per request, per agent, per compute-hour?)
- Custom branding — customer apps need own domain + branding (whitelabeling)
- SLA + support tiers
- Self-service signup vs sales-led onboarding
- Engine plugin/extension model — how custom code ships alongside managed Engine
