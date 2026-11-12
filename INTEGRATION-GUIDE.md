# OpenHarness: UI, Backend, and Integration Guide

## 1. Current product state

OpenHarness is a Tauri 2 desktop shell built with React 19, TypeScript, Vite, and Rust. The older `app/` folder is a separate egui prototype; the active desktop entry point is `tauri-react/` and Tauri Rust backend is in `tauri-react/src-tauri/`.

The UI includes a project drawer, composer, conversation view, settings, provider configuration, and integrated terminal. For implemented runtime behavior and explicit limitations, see the sections below. Browser-only mode is a demo fallback, not the desktop backend.


## 2. Runtime architecture and current state

The active desktop application is `tauri-react/`: React 19, TypeScript, Vite, and a Tauri 2 Rust backend. The top-level `app/` directory is a separate egui prototype. Desktop state persists providers, projects, threads/messages, compaction metadata, learning proposals, and approval mode to Tauri app-data `state.json`; this is file-backed JSON, not SQLite. Provider API keys are held in the OS credential store. Browser-only projects/providers use localStorage, threads are in-memory, and test/chat are simulated.

```text
React components → services/tauriBridge.ts → Tauri IPC/channels
  → commands and AppState in tauri-react/src-tauri/src/lib.rs
       ├─ secrets.rs: OS credential storage
       ├─ client.rs: provider HTTP requests and stream parsing
       └─ harness.rs: tools, workspace sandbox, skills, shell/Git helpers
Browser-only mode → localStorage/in-memory demo fallbacks (no Rust backend)
```

The active composer uses streamed chat with thread history and an agent/tool loop; `chat_send` is a separate one-shot command. Implemented providers are Ollama, OpenRouter, Google AI Studio's OpenAI-compatible endpoint, and generic OpenAI-compatible endpoints. Anthropic Messages, Claude Code/Codex OAuth or CLI, MCP, and model discovery are not implemented. The packaged app has a CSP in `tauri.conf.json`; review the configured Tauri capabilities when changing command/plugin access.

API keys must not be returned to React, logged, or persisted in ordinary files. Tauri commands are the frontend/backend boundary; keep inputs typed/validated and synchronize Rust signatures with `tauriBridge.ts`. Browser demo behavior is not evidence of desktop/provider behavior.

Rust diagnostics use Tauri's platform-specific application log directory (`app_log_dir`), not the process working directory. Release builds log `Info` and above to `openharness.log`; debug builds log `Debug` and above to that file and stdout. The logger rotates at 5 MiB and retains four files. The startup message identifies the log directory; if path resolution or plugin initialization fails, startup continues and reports the failure to stderr. Log messages must not contain credentials or private conversation content. The audit JSONL stream is separate, stored in app-data, and is not the diagnostic log. Typical paths are `%LOCALAPPDATA%\\<identifier>\\logs` on Windows, `~/Library/Logs/<identifier>` on macOS, and `$XDG_DATA_HOME/<identifier>/logs` (default `~/.local/share/.../logs`) on Linux; Tauri/runtime packaging may influence the identifier. Use the startup-reported path as authoritative.

## 3. Implemented command contract


Commands are registered in `src-tauri/src/lib.rs`; TypeScript calls belong in `src/services/tauriBridge.ts`.

| Purpose | Commands |
|---|---|
| Providers | `provider_save`, `provider_list`, `provider_delete`, `test_provider_connection` |
| Projects | `get_projects`, `select_project`, `create_project`, `rename_project`, `delete_project`, `relink_project` |
| Threads/messages | `list_threads`, `create_thread`, `get_thread`, `rename_thread`, `delete_thread`, `append_thread_message`, `edit_thread_message`, `delete_thread_message`, `compact_thread` |
| Chat and approvals | `chat_send`, `chat_send_stream`, `cancel_chat_stream`, `respond_to_tool_approval`, `get_tool_approval_mode`, `set_tool_approval_mode`, `update_thread_learning_candidates`, `dismiss_thread_learning_candidates` |
| Tools/workspace | `tool_list`, `skills_list`, `workspace_read_file`, `workspace_list_dir`, `workspace_grep`, `workspace_git_status`, `workspace_git_diff`, `workspace_git_log`, `workspace_git_branch` |
| Integrated terminal | `terminal_create`, `terminal_write`, `terminal_kill`, `terminal_list` |
| Window | `app_minimize`, `app_toggle_maximize`, `app_close` |

Commands are declared and registered in `tauri-react/src-tauri/src/lib.rs`; frontend wrappers live in `tauri-react/src/services/tauriBridge.ts`. Tauri maps JavaScript camelCase argument keys to Rust snake_case parameters (for example `projectId` to `project_id`). Consult the registration list for exact signatures and return types. `chat_send_stream` sends incremental `StreamChunk`s over a Tauri `Channel`; chunks may carry text, completion/error state, approval requests, step events, and token usage. `chat_send` is a separate one-shot command, not the agent loop.

## 4. Provider setup and request behavior

Provider metadata stores provider name, model ID, optional custom endpoint, context/output token limits, and vision support. API keys are passed to Rust during save/test and retained in the OS credential store; `provider_list` exposes safe metadata and `has_key`, never the key. Ollama and generic OpenAI-compatible configurations may be keyless; other configured providers require a key.

`client.rs` supports:

- **Ollama**: local default `http://localhost:11434/api/chat`; streaming requests use Ollama's streaming response format. A custom endpoint can support an explicitly configured server.
- **OpenRouter**: OpenAI-compatible chat completions.
- **Google AI Studio**: its OpenAI-compatible chat completions endpoint.
- **OpenAI Compatible**: default OpenAI chat completions or a validated custom endpoint; custom base paths are normalized to the chat-completions path.

Requests disable redirects and apply timeouts and response/error-body size bounds. Custom URL validation permits HTTPS and constrains HTTP to local/private endpoints for Ollama or OpenAI Compatible use; this is not general network isolation. Generic OpenAI-compatible services must match implemented request/stream response shapes. Model IDs are configured manually; model discovery is not implemented.

Anthropic's native Messages API, Claude Code/Codex sign-in, and vendor CLI integration are not implemented. Do not infer support from old UI labels or examples. Image attachments supported by the backend are PNG, JPEG, WebP, and GIF, validated for base64, 4 MB maximum size, MIME, and content signature; SVG is rejected. Images require model vision support. Persisted thread messages retain their own image attachments and are reconstructed in message order on later streamed requests; exact duplicate current-message references are merged once. History images are serialized as OpenAI-compatible `image_url` content parts, or Ollama `images` arrays. The implementation supports Ollama and the OpenAI-compatible request shape (including OpenRouter, Google AI Studio's compatible endpoint, and generic OpenAI-compatible providers); provider-specific departures from that shape may reject image requests. Missing/corrupt or unsupported stored image data fails request construction with an error rather than being silently omitted. Historical-image context is not automatically trimmed independently, so long threads with many images can exceed provider context limits. Arbitrary document upload/provider-side parsing is not provided.

## 6. Persistence and conversation history

Desktop startup loads Tauri app-data `state.json`. It persists provider metadata, projects, threads/messages, compaction metadata, learning proposals, and tool-approval mode. Writes serialize to a temporary file and replace the state file. This is JSON persistence, not SQLite. Provider secrets remain in the OS credential store; store failures are surfaced, not intentionally downgraded to plaintext.

Project IDs are independent of mutable names. Rename preserves IDs and thread association; existing persisted IDs remain unchanged. Loading rejects empty or duplicate IDs rather than choosing an ambiguous project. Duplicate display names are supported; duplicate project paths are rejected on creation. Missing project folders require relinking.

Threads are associated with a project ID and retain messages. `chat_send_stream` accepts a thread ID, incorporates prior messages, respects stored compaction summary/count, and appends the assistant result when complete. The UI may persist the current user message before invoking the stream, and passes its ID so the backend does not duplicate it in model context. Editing/deleting messages clears compaction state. `compact_thread` asks the configured model for a summary and stores it only if source messages have not changed.

At agent startup, project `AGENTS.md` files are discovered only along the canonical project-root-to-agent-working-directory chain, in root-to-leaf order; the nearest file has precedence over earlier conflicting project instructions. Sibling directories are excluded. The current agent working directory is the selected project root, so only root instructions apply in the current chat flow; nested instruction discovery is available to callers that supply a nested task directory, but tool execution does not yet change that scope automatically. Files are loaded once per agent run, validated as regular non-symlink files within the project boundary, and logged by canonical path/count (never content). Repository instructions are untrusted task guidance and cannot override system, developer, or user instructions, sandbox boundaries, or approval requirements.

Browser-only mode is intentionally different: project/provider metadata uses localStorage, threads are page-memory only, and chat/provider tests are simulated.

## 7. Streaming, cancellation, and agent loop

The active composer uses `chat_send_stream`. Provider parsers emit text deltas and token usage when supplied. The Rust agent loop can make multiple model requests, dispatch tool calls, add tool results to context, and continue until a final response or request limit/error. Stream chunks can contain step events and approval requests as well as text.

`cancel_chat_stream` accepts an optional stream ID; omitting it cancels all active streams. The bridge supports passing an ID, but callers that omit it use cancel-all semantics. Backend registry entries are cleaned up on task exit. Registry tests do not prove end-to-end cancellation during every provider or tool operation. `chat_send` remains a separate one-shot provider command. Browser demo chat produces a simulated local callback, not provider streaming.

## 8. Tools, approval, and execution limits

The Rust tool registry exposes file read/write/patch, directory listing, text search, shell command, Git status/diff/log/branch, skill activation, and project-learning proposals. Model tool calls are parsed and validated before dispatch; `tool_list` exposes definitions to the frontend.

Approval mode defaults to `manual`. `write_file`, `patch_file`, and `run_command` require explicit approval in manual mode; automatic mode permits them without per-operation approval. Read/search/list/Git and skill activation are not in the approval-required set. Approval decisions use `respond_to_tool_approval`. Learning proposals are staged for user review; the model tool itself writes no files. Approved proposals can add notes to project `AGENTS.md` or create a project skill.

`run_command` runs a shell command in the project folder, defaults to a 30-second timeout (clamped to 1–120 seconds), and caps output. It is not a command allowlist: shell commands can access paths outside the project and cause other OS-visible effects. Approval is not an OS sandbox. The integrated terminal is a separate user-controlled shell.

## 9. Workspace sandbox and limits

Workspace file tools use `harness::Sandbox`, rooted at the selected project's canonical directory. Absolute paths, parent traversal, non-portable path components, and symlink reads/edits are rejected; existing paths and destination parents are checked to remain under the root. Reads/patches and command outputs are size-bounded; directory listing/search skip symlinks and common generated directories. Writes use a temporary file and replacement after path checks. These restrictions apply to file tools, not shell commands or the interactive terminal.

Project-root lookup validates project IDs, rejects missing/ambiguous IDs, canonicalizes the selected folder, and requires a directory. Projects without an available folder cannot provide project context or run project-root tools until relinked. Path checks reduce traversal risk but do not eliminate races from hostile concurrent filesystem changes.

## 10. Skill discovery and activation

Skill discovery scans `.agents/skills`, `.claude/skills`, and `.kilo/skills` under the selected project root and user home. A valid skill directory contains `SKILL.md` whose frontmatter name matches the directory and whose description is non-empty; names use lowercase ASCII letters, digits, and hyphens. Project skills take precedence over same-named global skills. Symlinks, unsafe names, invalid metadata, and oversized files/resources are skipped or rejected; scan and selection counts are bounded.

The agent receives metadata first, not all skill instructions. It can invoke `activate_skill` when relevant to load `SKILL.md` and explicitly requested validated resources. The UI can also pass selected skills. Skill text and project `AGENTS.md` are untrusted guidance; they cannot override system/user instructions, sandbox boundaries, or approval rules. Learning-created skills require user approval.

## 11. Integrated terminal

The integrated terminal has separate Tauri process commands. Rust validates requested shell names against platform-supported choices, resolves a platform-appropriate configured/default shell, retains process and stdin for writes, and supports listing/killing sessions. Terminal output is streamed to the UI. The agent's `run_command` tool is a bounded, non-interactive subprocess and is not this terminal. Verify runtime behavior on each target OS before claiming cross-platform reliability.

## 12. Security and reliability checklist

- Keep API keys in the OS credential store; never expose a secret-read command or log credentials.
- Preserve credential-store errors; avoid silently downgrading to plaintext or memory.
- Keep HTTP redirects disabled, validate endpoints, and preserve timeouts/response bounds.
- Validate Tauri command inputs and surface lock/IPC errors.
- Treat workspace file sandboxing and shell execution as separate trust boundaries; shell approval does not contain the process.
- Review capabilities and CSP when commands/plugins change. Do not imply a placeholder is saved/connected.
- Keep accessible, keyboard-operable UI behavior when adding approval/settings flows.

## 13. Planned and unsupported work

Do not present these as implemented: native Anthropic Messages support, vendor OAuth/CLI integrations, model discovery, SQLite migrations, arbitrary file/document upload, command allowlisting or OS-level shell containment, complete end-to-end cancellation coverage, and cross-platform terminal runtime verification. The top-level `app/` prototype is not evidence of Tauri desktop behavior.

## 14. Verification commands

Run from `tauri-react/`:

```sh
npm run build
npx tauri dev
```

Run from `tauri-react/src-tauri/`:

```sh
cargo fmt --check
cargo check
cargo test
```

`cargo clippy -- -D warnings` is useful when the repository warning baseline permits it. Run relevant Rust tests and the frontend build after changing IPC contracts, persistence, provider parsing, tools, or UI bridge behavior. The top-level `app/` checks apply only to the separate egui prototype.

## 15. Implementation source map

- Tauri commands, state, persistence, stream/approval coordination, chat orchestration, terminal commands: `tauri-react/src-tauri/src/lib.rs`.
- Provider configuration, endpoint validation, image validation, HTTP requests and streaming parsers: `tauri-react/src-tauri/src/client.rs`.
- Workspace sandbox, execution engine, Git helpers, tool registry/dispatch, skill discovery/activation: `tauri-react/src-tauri/src/harness.rs`.
- OS credential storage: `tauri-react/src-tauri/src/secrets.rs`.
- Frontend IPC wrappers and browser fallbacks: `tauri-react/src/services/tauriBridge.ts`.
- Composer/streaming/terminal UI and project integration: `tauri-react/src/components/Composer.tsx`, `Canvas.tsx`, `Drawer.tsx`.
- Tauri security policy and plugins: `tauri-react/src-tauri/tauri.conf.json`, `tauri-react/src-tauri/capabilities/`.
