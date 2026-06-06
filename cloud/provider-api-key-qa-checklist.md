# Provider API key manual QA checklist

Operator-run acceptance for multi-provider API key connect (Anthropic, OpenAI,
Gemini, OpenRouter) alongside existing CLI OAuth paths. Derived from
`cloud/provider-ux-api-keys-feature.json`.

**Prerequisites:** Houston desktop build with provider API key routes wired
(be-01 through be-05). Real API keys from each provider console. Do not commit
keys or paste them into tickets.

**Evidence:** For each step, record pass/fail, date, operator initials, and a
one-line note (redact any key material). Attach screenshots or log excerpts when
useful.

## API key connect matrix

| # | Provider | Step | Status | Operator | Date | Notes |
|---|----------|------|--------|----------|------|-------|
| 1 | Anthropic | Connect via API key paste in settings or provider picker (advanced section). | pending | | | `POST /v1/providers/anthropic/credentials`; status shows connected without restart. |
| 2 | Anthropic | Send one chat message with a Claude model. | pending | | | Reply streams; no auth error. |
| 3 | Anthropic | Disconnect. Confirm status unauthenticated and reconnect UI appears. | pending | | | `POST /v1/providers/anthropic/logout`; `~/.houston/providers/anthropic/.env` cleared. |
| 4 | OpenAI | Connect via API key paste (advanced section). | pending | | | `POST /v1/providers/openai/credentials`; status shows connected. |
| 5 | OpenAI | Send one chat message with a Codex model. | pending | | | Reply streams when no OAuth session exists. |
| 6 | OpenAI | Disconnect API key only. Confirm stored key cleared. | pending | | | If OAuth subscription login exists, subscription session may remain; API key file must clear from `~/.houston/providers/openai/.env`. |
| 7 | Gemini | Connect via API key paste. | pending | | | `POST /v1/providers/gemini/credentials`; canonical `~/.houston/providers/gemini/.env`. |
| 8 | Gemini | Send one chat message. | pending | | | Reply streams. |
| 9 | Gemini | Disconnect. Confirm reconnect UI. | pending | | | OAuth creds cleared per gemini disconnect path. |
| 10 | OpenRouter | Connect via API key paste. | pending | | | `POST /v1/providers/openrouter/credentials`. |
| 11 | OpenRouter | Select OpenRouter model and send one chat message. | pending | | | Errors label provider=openrouter, not openai. |
| 12 | OpenRouter | Disconnect. Confirm reconnect UI. | pending | | | `~/.houston/providers/openrouter/.env` removed. |

## CLI OAuth regression (subscription login unchanged)

| # | Provider | Step | Status | Operator | Date | Notes |
|---|----------|------|--------|----------|------|-------|
| 13 | Anthropic | Connect via CLI OAuth (Connect with Claude, not API key). | pending | | | `POST /v1/providers/anthropic/login`; browser or paste-code flow completes. |
| 14 | OpenAI | Connect via CLI OAuth (Codex login, not API key). | pending | | | Subscription OAuth still preferred when present over API key. |
| 15 | Gemini | Connect via Sign in with Google (CLI OAuth). | pending | | | Gemini OAuth path must still work after API key feature ships. |
| 16 | Cross | After OAuth connect on one provider, API key connect on another still works. | pending | | | No cross-provider credential bleed. |

## Chat smoke (post-connect)

| # | Step | Status | Operator | Date | Notes |
|---|------|--------|----------|------|-------|
| 17 | Pick agent using each connected provider; send "Reply with OK only." | pending | | | Minimal smoke across providers exercised in rows 2, 5, 8, 11. |
| 18 | Invalidate one stored key (revoke or replace with bogus value). Confirm Unauthenticated error card with correct provider id. | pending | | | User sees toast + Report bug; no silent fallback. |
| 19 | Reconnect via API key after invalidation. | pending | | | Status returns authenticated; chat works again. |

## Automated gate (qa-02)

Run before marking manual rows complete:

```bash
cargo test --workspace -- --test-threads=1
```

Integration coverage lives in `engine/houston-engine-server/tests/providers.rs` for
`anthropic`, `openai`, `gemini`, and `openrouter` credential routes (24 tests in that
file; full workspace gate: 944 passed, 5 ignored).

## Blocked / cannot run

If a step cannot be executed (missing credits, shell `OPENAI_API_KEY` blocking
disconnect, platform without bundled CLI, etc.), mark the row **blocked**, state
why, and what evidence would unblock it.

## Related docs

- Feature tracker: `cloud/provider-ux-api-keys-feature.json`
- OpenRouter-only checklist: `cloud/openrouter-qa-checklist.md`
- KB: `knowledge-base/agent-manifest.md`, `auth.md`, `provider-errors.md`, `engine-protocol.md`
