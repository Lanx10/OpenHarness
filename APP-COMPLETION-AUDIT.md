# OpenHarness App Completion Guide

## Summary

OpenHarness has a functional shell, projects, provider storage, and a tauri updater. The chat core remains stateless, non-streaming, and disconnected from configured models. The list below prioritizes required capabilities to reach parity with mature desktop AI harnesses.

---

## P0 — Chat Core

1. **Persist chat history per project** (`Canvas.tsx`, `tauriBridge.ts`, `lib.rs`)
   - Messages live in volatile React `useState`. Implement per-project conversation storage in `state.json` or SQLite.
   - Add session list, thread switching, rename, and search. Keep project folders as containers, not substitutes for chat history.

2. **Add token streaming** (`client.rs`, `Canvas.tsx`)
   - Current calls use blocking HTTP. Stream via Tauri channel events or server-sent events from Rust into incremental text state.
   - Include a stop/cancel control bound to a Rust `tokio::task::AbortHandle`.

3. **Send context to the model** (`client.rs`)
   - Request body has single user message only. Include the current thread’s recent messages so multi-turn context is real.
   - Expose system prompt, temperature, max tokens, and top-p controls.

4. **Wire Composer models to real providers** (`Composer.tsx`, `SettingsPage.tsx`)
   - Remove hardcoded `MODELS` array. Load from `apiListProviders()` and local Ollama tags.
   - Add a persisted default model and per-project model override. Disable reflection of a saved model as a new fake fallback.

---

## P1 — Workspace & Project Management

1. **Add project editing** (`Drawer.tsx`, `lib.rs`)
   - Implement rename, delete, and folder-relink commands. Validate project path existence on load and open.

2. **Add attachments pipeline** (`Composer.tsx`, `client.rs`)
   - Use Tauri file dialog to read selected files and transmit their contents or path metadata.
   - Support drag-and-drop and pasted images/code. Do not display filenames without sending content.

3. **Add message actions** (`Canvas.tsx`)
   - Implement copy, regenerate, edit, delete, and message retry.

4. **Add Markdown and code rendering** (`Canvas.tsx`)
   - Use a Markdown + syntax-highlighting renderer. Include a copy button for code blocks and fenced language detection.

---

## P2 — UI Completeness

1. **Choose tabs or remove the tab metaphor** (`Chrome.tsx`)
   - Replace fake tab/titlebar chrome with a persistent breadcrumb containing project/model/title, or implement true multi-tab state.

2. **Fix the Hero "Local" shortcut** (`Canvas.tsx`)
   - Target an actionable local state or the relevant Ollama provider configuration. Do not open a misleading generic settings page.

3. **Add global notices** (`Canvas.tsx`, `Drawer.tsx`)
   - Replace the dead `#notice` element with a shared status/toast surface for loading, fetch errors, and updater progress.

4. **Add connection testing** (`SettingsPage.tsx`)
   - Verify API endpoint and credential before persist. Show endpoint errors inline, not raw exceptions.

5. **Add empty states** (`Drawer.tsx`, `Canvas.tsx`, `SettingsPage.tsx`)
   - Show usable guidance when projects, chats, and providers are absent.

---

## P3 — Reliability & Cleanup

1. **Centralize event state** (`OpenHarness.tsx`, components)
   - Replace `window.dispatchEvent` string events with React context or a small store. Reduce component coupling and unsynchronized state drift.

2. **Remove dead code and redundant paths** (`Chrome.tsx`, `Icons.tsx`, boilerplate assets)
   - Remove unused icon exports, legacy root experiments, and duplicate window-control fallbacks.

3. **Harden web fallback storage** (`tauriBridge.ts`)
   - Do not store API keys as plaintext in browser `localStorage` outside the Tauri runtime. Use session-only storage for demos.

4. **Add root `.gitignore`**
   - Ignore build logs, transcripts, generated artifacts, and local environment files.

5. **Additional mature-app surfaces** (optional)
   - Keyboard shortcuts, theme controls, export, search, data/retention preferences, telemetry opt-out, and per-message feedback/reporting.

### Verification Targets

- Chat history survives project transitions and app restarts.
- Selected provider/model in Composer is the model that handles the request.
- Responses stream token-by-token and can be canceled.
- Attachments change the request payload.
- Code blocks, syntax colors, and copy controls render correctly.
- Project deletion, renaming, search, and provider-connection errors behave without unhandled failures.

---

## Agent Runtime Audit Update — 2026-11-11

### Confirmed runtime findings

- **Duplicate current user message (fixed):** `chat_send_stream` loaded persisted history and appended the active prompt again. Frontend now sends the persisted user message ID; Rust validates it belongs to the selected thread and excludes that entry from history.
- **Attachment persistence (partially fixed):** user images now persist with the message and are reattached to the corresponding current turn. File contents are already included in the composed prompt; only image attachments are stored to avoid duplicate file text. Prior-turn images can be re-used, but binary attachment lifecycle and files referenced by path are not retained.
- **Unbounded tool loop (fixed):** the agent loop now caps model requests at 12 per run and returns an explicit error after the cap. The request ceiling should be revisited if legitimate workflows need more iterations.
- **Release logging (fixed):** Tauri logging now initializes in release builds at INFO level; debug builds use DEBUG. Startup/runtime log persistence destination and retention still need verification on packaged builds.
- **Project instruction loading (improved):** `AGENTS.md` is loaded into system context when present. Its contents are explicitly labelled project-controlled, untrusted guidance; sandbox and approval requirements remain authoritative. Load failures other than absent/non-file are surfaced as errors.
- **Token accounting remains cache-only:** current provider parsing and UI expose cached input tokens, not a per-request full input/output breakdown. There is no configured model-matched tokenizer. Do not interpret the displayed value as total tokens or compare it directly with another framework's total context usage. OpenAI-compatible cache details are parsed; Ollama usage is not currently propagated.
- **Audit trail remains incomplete:** lifecycle logs cover run start, context component counts, tool registration, and request-cap events. There is no durable per-request audit record for every loaded component, tool outcome, memory reference, usage discrepancy, or final provider response.
- **Startup validation remains partial:** app state loads at startup and runtime-selected skills are loaded before a run; required component readiness is not represented in a comprehensive startup validation report. Selected-skill names that are unavailable are silently omitted by the loader.
- **Other context gaps:** historical image attachments were not included in reconstructed conversation context; now included for the active persisted message. Project-wide AGENTS.md discovery is root-only (no ancestor instruction merge). Context budgeting is compaction-based and does not use exact model tokenization.

### Changes made

- `tauri-react/src-tauri/src/lib.rs`: request cap and regression test; duplicate-message exclusion with message-ID validation; current-message image restoration; root AGENTS.md loading with untrusted-content framing; startup/run/context/tool lifecycle logs; release log plugin enabled.
- `tauri-react/src/components/Canvas.tsx`: persist the active message for all selected-thread turns and retain image data attachments.
- `tauri-react/src/services/tauriBridge.ts`: persist message attachment values in Tauri and web fallback.

### Verification evidence

- `cargo test --lib -- --test-threads=1`: **50 passed, 0 failed**.
- `cargo check --lib`: passed.
- `npx tsc --noEmit`: passed.
- `npm run build`: passed (TypeScript compile and Vite production build).
- `git diff --check`: passed; Git reported only LF-to-CRLF working-copy warnings.
- `cargo fmt --check`: fails because Rust formatting differences exist throughout pre-existing `lib.rs` and `harness.rs`; formatting the entire file would create unrelated churn. Changed Rust expressions were reviewed; rerun after a scoped formatting policy is chosen.
- `cargo clippy --lib -- -D warnings`: fails on existing warnings in `client.rs`, `harness.rs`, and `lib.rs` (including too-many-arguments and collapsible-if findings); no broad cleanup attempted.
- `npm run lint` and `npm run typecheck`: unavailable scripts in `tauri-react/package.json`.
- Existing GitHub issue update could not be posted: GitHub CLI is unauthenticated in this environment. This section is the prepared update for review; no issue status was changed.

### Remaining limitations / next steps

1. Add typed provider usage fields and aggregate per-request/run totals; keep estimated counts explicitly separate from provider-reported usage.
2. Add model-appropriate tokenizers only for supported configured model families; otherwise report counts as unavailable rather than inventing estimates.
3. Persist structured per-run audit records with redacted safe identifiers, component states, request outcomes, cache fields, and tool statuses.
4. Validate critical configuration and component readiness at startup; report unavailable and disabled components distinctly.
5. Test release log output destination and rotation on a packaged Windows build.
6. Add mock/provider tests for prompt completeness, duplicate exclusion, prior-turn image reconstruction, cancellation, and request-cap termination.
7. Reconcile the older completion-guide summary above with current implementation: chat persistence, streaming, multi-turn context, attachments, and project editing now exist; this audit update records the remaining runtime accounting and observability gaps.
