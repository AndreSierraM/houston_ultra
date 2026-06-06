# Cloud + API key harness parity

Houston's agent loop is a **CLI subprocess harness** (`houston-terminal-manager`), not a direct HTTP chat client. Cloud and API-key paths must preserve that stack.

## Three axes (do not conflate)

| Axis | What it controls |
|------|------------------|
| Agent runtime | `local` sidecar vs `cloud_24_7` container |
| Control-plane auth | `VITE_HOUSTON_CLOUD_TOKEN` vs Supabase JWT |
| Provider auth | CLI OAuth vs Houston-managed API key in `~/.houston/providers/<id>/.env` |

## Provider API key does NOT drop the harness

| Provider | API key path | Harness |
|----------|--------------|---------|
| Anthropic | `ANTHROPIC_API_KEY` injected at spawn | `claude -p` subprocess |
| OpenAI | `OPENAI_API_KEY` when no OAuth | `codex exec` subprocess |
| OpenRouter | `OPENROUTER_API_KEY` required | `codex exec` + process-local `-c` overrides |
| Gemini | `GEMINI_API_KEY` optional | `gemini` subprocess |

OpenRouter explicitly excludes a direct HTTP agent loop (`cloud/openrouter-provider-feature.json`).

## Cloud runtime contract

Each `cloud_24_7` agent gets:

- Private `houston-engine` container + volume
- Same REST/WS surface as local (`runtime-router.ts` → control-plane proxy)
- CLIs inside the image (`always-on/Dockerfile`: Claude, Codex, Composio)
- Provider credentials on the **container volume**, not copied from desktop

Desktop must activate the same harness services against the proxied engine:

- `ensureAgentEngineWs` for event firehose
- `startAgentWatcher` for `.houston/` reactivity
- `startRoutineScheduler` for cron/routines

Implemented in `app/src/lib/activate-agent-runtime.ts`, called from `stores/agents.ts` on `setCurrent` and `create`.

## Known gaps

| Gap | Mitigation |
|-----|------------|
| Gemini CLI missing in Linux engine image | API key only in cloud until CLI bundled for linux |
| `cloud_provider_connections` table not implemented | Credentials live in engine volume via existing provider routes |
| WS revoke mid-session | Checked at connect only |
| Portable export | Local only; cloud uses live share wizard |

## Routing checklist for new app code

1. Domain calls → `resolveEngine(agent)` not `getEngine()` when agent-scoped
2. Provider prefs/status → `resolveEngine(currentAgent())`
3. Events → `subscribeHoustonEvents(handler, currentAgent)`
4. Never assume API key = HTTP-only loop
