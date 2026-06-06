# Provider UI regression checklist

Operator-run visual QA for provider-related screens after the provider UX + API keys
feature (`cloud/provider-ux-api-keys-feature.json`). Focus: overflow, truncation, spacing,
and dual-path connect affordances. No layout redesign expected.

**Prerequisites:** Local Houston desktop build (`pnpm tauri dev` or release build). Test at
least one viewport width ≤1280px and one narrow height (~720px) to catch clipped CTAs.

**Evidence:** For each step, record pass/fail, date, operator initials, and a one-line note.
Attach screenshots when overflow or clipping is suspected.

| # | Screen / flow | fe owner | Status | Operator | Date | Notes |
|---|---------------|----------|--------|----------|------|-------|
| 1 | **Settings → Provider** (`provider-settings.tsx`, `provider-account-row.tsx`, `provider-cards.tsx`): list scrolls vertically; no horizontal scrollbar; long account emails/subtitles truncate with ellipsis. | fe-01 | pending | | | |
| 2 | **Provider picker** (`provider-picker.tsx`): card grid fits container; connected/not-connected subtitles truncate; card density matches pre-OpenRouter baseline (no oversized padding). | fe-01 | pending | | | |
| 3 | **Coming soon cards** in picker/onboarding: labels do not overflow card bounds. | fe-01 | pending | | | |
| 4 | **Generic provider connect** (`provider-connect-dialog.tsx`): dialog fits `max-w-md`; primary/secondary buttons fully visible; body scrolls if content exceeds viewport. | fe-02 | pending | | | |
| 5 | **API key connect** (`api-key-connect-dialog.tsx`, `api-key-form.tsx`): key input + submit row not clipped; helper links wrap or truncate cleanly. | fe-02 | pending | | | |
| 6 | **Gemini connect** (`gemini-connect-dialog.tsx`): OAuth and API key paths both visible; neither path pushes buttons off-screen. | fe-02 | pending | | | |
| 7 | **Anthropic / OpenAI connect**: CLI sign-in default visible; API key path reachable as advanced option without horizontal overflow. | fe-02 | pending | | | |
| 8 | **Chat model selector** (`chat-model-selector.tsx`, `chat-model-selector-parts.tsx`): trigger truncates long model id; dropdown labels/descriptions truncate; provider icon does not overflow composer. | fe-03 | pending | | | |
| 9 | **Provider logos** (`provider-logos.tsx`): icons stay `shrink-0` inside badges; no badge overflow in selector or picker. | fe-03 | pending | | | |
| 10 | **Create workspace** (`create-workspace-dialog.tsx`, `naming-step.tsx`): provider step scrolls; footer CTAs visible on short viewports. | fe-04 | pending | | | |
| 11 | **Onboarding brain mission** (`brain.tsx`): provider cards scroll; connect dialogs open without clipping; continue CTA always reachable. | fe-04 | pending | | | |
| 12 | **Import wizard** (`import-wizard.tsx`): provider selection step same overflow rules as create flow. | fe-04 | pending | | | |
| 13 | **Provider error card** (`provider-error-card.tsx`, `provider-error-cards/*`): message body wraps; action buttons not clipped; long provider names truncate. | fe-05 | pending | | | |
| 14 | **Reconnect card** (`provider-reconnect-card.tsx`, `auth-reconnect-banner.tsx`): reconnect offers API key when provider supports it; layout readable at narrow width. | fe-05 | pending | | | |
| 15 | **i18n spot-check** (en / es / pt): open provider settings + one connect dialog in each language; no missing keys, no em dashes in copy, no literal English in es/pt. | qa-01 | pending | | | Switch language in app settings before steps 1–7. |

## Automated gates (qa-01)

| Check | Status | Date | Notes |
|-------|--------|------|-------|
| `cd app && pnpm tsc --noEmit` | pass | 2026-06-06 | No TypeScript errors in app. |
| `cd app && pnpm check-locales` | pass | 2026-06-06 | en/es/pt namespaces in sync (17 each). |
| `pnpm typecheck` (workspace) | pass | 2026-06-06 | app + all ui packages clean via `tsc --noEmit`. |

## Blocked / cannot run

If a step cannot be executed (provider backend not wired, missing CLI, no API key test
account), mark the row **blocked**, state why, and what would unblock it. Do not mark the
feature UI-done without explicit blocked proof.

## Related docs

- Feature tracker: `cloud/provider-ux-api-keys-feature.json`
- OpenRouter manual QA (credential E2E): `cloud/openrouter-qa-checklist.md`
- KB: `knowledge-base/agent-manifest.md`, `knowledge-base/provider-errors.md`, `knowledge-base/i18n.md`
