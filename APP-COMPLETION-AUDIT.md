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
