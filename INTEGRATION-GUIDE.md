# OpenHarness: UI, Backend, and Integration Guide

## 1. Current product state

OpenHarness is a Tauri 2 desktop shell built with React 19, TypeScript, Vite, and Rust. The older `app/` folder is a separate egui prototype; the active desktop entry point is `tauri-react/` and Tauri Rust backend is in `tauri-react/src-tauri/`.

The UI now has a title bar and Code/Chat mode toggle, navigation rail, project drawer, composer with model chooser, conversation view, settings overlay, provider form/list. Tauri commands now provide in-memory project state, provider configuration state, OS credential-store API keys, and chat requests through OpenAI-compatible, Anthropic-compatible, or local Ollama APIs. The browser-only development path uses in-memory providers/projects and a response stub.

## 2. Current state and important limitations

- Projects exist only in backend memory. They reset on application restart; project paths, conversations, files, and agent runs are not yet persisted.
- Provider metadata also exists only in backend memory; the API key uses the OS credential store. Restarting loses model/base-URL metadata, leaving the secret orphaned until re-entered. Persist metadata before claiming durable provider setup.
- `chat_send` performs single-request, non-streaming model inference. It sends one user message, no conversation history, system prompt, tool calls, or project context. This is not an autonomous coding agent yet.
- The current model picker defaults to Claude 3.7 Sonnet while the provider dropdown saves Anthropic-compatible/API gateway configurations under provider-option names. A real send requires matching the configured provider name, model ID, and API compatibility.
- “Claude Code” and “Codex” source cards are intentionally disabled; there is no OAuth, CLI process integration, or vendor-specific account login.
- “Developer API” settings, update checks, search, attachment, template cards, histories/archives, and the non-project rail navigation are incomplete or demonstrative.
- `tauri.conf.json` still sets `security.csp` to `null`. Replace with restrictive CSP before packaging.
- OS credential store errors must be shown; never silently fall back to in-memory/plaintext storage for a user-supplied API key.
- Rust `run()` currently uses `expect` at application startup, which is acceptable as process bootstrap failure, not a recoverable command-path error.

## 3. Architecture and data flow

```text
React components
  ├── events for local UI composition
  └── services/tauriBridge.ts — invokes typed Tauri commands
          └── Tauri IPC + capabilities/default.json
                └── src-tauri/src/lib.rs — validate requests/state
                      ├── secrets.rs — API keys in OS credential store
                      ├── client.rs — outbound HTTPS/localhost model calls
                      └── in-memory AppState — providers/projects
```

Keep all outbound provider traffic in Rust. API keys must not be returned to React, logged, stored in localStorage, or persisted in ordinary config files. Keep Tauri command inputs narrow, typed, validated, size-bounded; do not expose shell or arbitrary filesystem commands to the renderer.

## 4. Implemented command contract

Commands are registered in `src-tauri/src/lib.rs`; TypeScript calls belong in `src/services/tauriBridge.ts`.

| Command | Input | Output | Purpose |
|---|---|---|---|
| `provider_save` | `provider`, `modelId`, `baseUrl`, `customEndpoint`, `apiKey` | Provider metadata, never key | Save metadata in process state, key via OS credential manager |
| `provider_list` | none | `ProviderInfo[]` | Return safe provider metadata |
| `provider_delete` | `provider` | void | Delete metadata and key |
| `get_projects` | none | `ProjectInfo[]` | Return in-memory projects |
| `select_project` | `id` | `ProjectInfo[]` | Change active project |
| `create_project` | `name` | `ProjectInfo` | Add active project |
| `chat_send` | `prompt`, `modelId`, `provider`, optional `projectId` | `{text, model, provider}` | Call provider or give explicit setup guidance |

Rust command parameters use camelCase as received from JavaScript and snake_case internally. Keep the interface synchronized with `tauriBridge.ts` and test IPC serialization whenever fields change.

## 5. Provider setup and expected API shapes

Supported request strategies in `client.rs`:

- **OpenRouter**: `POST https://openrouter.ai/api/v1/chat/completions`, bearer key, OpenAI-compatible JSON.
- **OpenAI-compatible**: `POST https://api.openai.com/v1/chat/completions`, or configured custom URL, bearer key.
- **Google AI Studio compatibility**: `POST https://generativelanguage.googleapis.com/v1beta/openai/chat/completions`, bearer key.
- **Anthropic-compatible**: `POST https://api.anthropic.com/v1/messages`, `x-api-key`, `anthropic-version`, Messages JSON. A custom base URL currently replaces the whole request URL; if implementing server-compatible local Anthropic endpoints, append `/v1/messages` carefully and normalize user-provided base URLs.
- **Ollama**: `POST http://localhost:11434/api/chat`, `stream:false`; local requests need no key. Optional key is for cloud endpoints.

Validate model IDs and endpoint syntax. Restrict custom URLs to `https:` by default; permit `http://localhost` only for explicit local Ollama use. Block loopback/private/link-local/metadata targets for remote custom endpoints where practical; DNS rebinding considerations make URL string checks alone insufficient. Enforce request timeouts, response size limits, and safe error messages. Do not echo arbitrary provider error bodies into logs or UI without redaction and size caps.

## 6. Next integration milestones

### P0 — make currently exposed UI behavior correct

- Persist provider metadata using Tauri plugin-store or a small SQLite table; preserve secrets in the OS credential store only.
- Replace the model-picker static list with models derived from saved provider entries; keep provider/model pair selection consistent.
- Wire project selection into the chat request (currently the composer event does not carry the selected project ID). Display API errors clearly; do not replace later responses with stale async results.
- Implement duplicate-name/ID handling in project creation, including Unicode/slug collisions.
- Add unit tests for blank input, URL validation, provider save/list/delete, project selection, and UTF-8-safe chat subtitle truncation.

### P1 — complete actual chat experience

- Introduce bounded conversation messages, persist them per project, and pass context to provider adapters.
- Add streaming: Rust emits events/channels; React incrementally appends deltas. Support cancellation and app/window teardown.
- Preserve structured provider errors; distinguish authentication, rate limits, offline Ollama, timeout, unsupported model, malformed response.
- Add provider-specific request adapters instead of provider-name switches when the first substantial behavior differences arrive. Support custom OpenAI-compatible endpoints and Anthropic-compatible endpoints explicitly.
- Add model discovery only through provider APIs that support it; manual model IDs remain valid.

### P2 — coding-agent capabilities

- Define explicit opt-in project root selection and canonicalize every path inside the chosen root.
- Add workspace context readers only after content and size constraints are clear. Put untrusted repo content in an untrusted data channel; never merge file instructions into system policy.
- Add tools one at a time: read/search first; write/edit only after visible diff approval; terminal execution only with explicit per-command confirmation and timeout/output limits.
- Define cancellation, task ownership, logs, bounded concurrency, retry ceilings, and recovery before adding agent loops.
- Add Git integration, diff display, apply/revert, checkpoints, and a clear audit trail before automatic edits.

### P3 — ship quality

- Restrict CSP and capabilities; audit every Tauri command exposed to the webview.
- Set a restrictive minimum window size compatible with layout; test keyboard navigation, high contrast, screen readers, and responsive settings.
- Add release signing/notarization, update signing/key rotation, reproducible release builds, privacy policy, license notices, and dependency/CVE checks.
- Add SQLite schema migrations for projects, sessions, messages, provider metadata, model selection, and user settings. Migrations must preserve compatibility and have backup/restore tests.
- Add integration tests with a local mock HTTP server; never depend on paid providers in CI.

## 7. Security and reliability checklist

- API keys: keyring only; separate account per provider; never expose a “read secret” command.
- Errors: propagate credential-store failures; avoid `let _ = result` in production paths; redact secrets and response bodies.
- HTTP: HTTPS by default, local HTTP only for Ollama, request/response bounds, timeouts, no redirects carrying credentials to another origin.
- IPC: validate empty/oversized prompt, provider ID, model ID, URL scheme/host, and project ID; handle poisoned locks with surfaced errors.
- Agent tools: filesystem sandbox to one canonical root; shell denylist is insufficient, require explicit approval and allowlisted operations.
- UI: settings dialog focus management and Escape handling, keyboard-operable controls, accessible names, disabled controls must not imply support.
- Product copy: never label a placeholder or non-persisted state “saved” or “connected.”

## 8. Verification commands

Run from `tauri-react/`:

```powershell
npm run build
npx tauri dev
```

Run from `tauri-react/src-tauri/`:

```powershell
cargo fmt --check
cargo check
cargo clippy -- -D warnings
cargo test
```

The original `app/` is separate; run its Rust checks from `app/` when changing that prototype. Do not use its successful build as evidence that the Tauri backend is complete.

## 9. Current research references

Primary sources, consulted 2026-11-07:

- [Tauri v2 Security](https://v2.tauri.app/security/) — IPC trust boundary, capabilities, principle of least privilege.
- [Tauri v2 Capabilities](https://v2.tauri.app/security/capabilities/) — command/plugin access scope.
- [Tauri v2 CSP](https://v2.tauri.app/security/csp/) — production content security policy.
- [Tauri v2 State Management](https://v2.tauri.app/develop/state-management/) — managed Rust state.
- [Tauri v2 Calling Rust](https://v2.tauri.app/develop/calling-rust/) — command registration and IPC.
- [Tauri v2 Plugin Store](https://v2.tauri.app/plugin/store/) — non-secret settings persistence.
- [Anthropic Messages API](https://platform.claude.com/docs/en/api/messages) — request/response schema.
- [Anthropic Streaming](https://platform.claude.com/docs/en/build-with-claude/streaming) — SSE event flow.
- [Ollama Chat API](https://docs.ollama.com/api/chat) — local chat API.
- [Ollama OpenAI compatibility](https://docs.ollama.com/api/openai-compatibility) — supported compatibility surface.
- [OpenRouter Quickstart](https://openrouter.ai/docs/quickstart) — OpenAI-compatible API configuration.

Treat the APIs/model names as changing; use official docs and verify compatibility when implementing each provider adapter. These sources support API shapes and platform patterns, not a claim of completed streaming, tool execution, project persistence, or account integration.
