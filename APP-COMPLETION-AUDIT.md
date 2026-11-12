# OpenHarness — Completion and Accuracy Audit

**Status:** Current source review; not a release certification. This document describes the active Tauri application, not the separate `app/` egui prototype. Feature labels distinguish source inspection from checks actually run during this review.

## 1. Executive summary

OpenHarness is a Tauri 2 desktop app with a React 19/TypeScript/Vite frontend and Rust backend. Desktop functionality includes provider-backed agent chat, streaming, persisted conversations, tools, skill activation, approval for selected write/command tools, workspace file sandboxing, an integrated terminal, diagnostic logging, and a JSONL audit trail.

Implementation is substantial but not fully verified end-to-end. Startup readiness includes inferred/static statuses rather than checks for every component; audit event declarations exceed emitted coverage; shell commands and the integrated terminal are not OS-sandboxed; packaged cross-platform behavior is not established by the current checks.

## 2. Current implementation overview

- **Frontend:** `tauri-react/src`; React and TypeScript. `services/tauriBridge.ts` wraps Tauri IPC and has a browser-only demo fallback.
- **Backend:** `tauri-react/src-tauri/src`; Rust crate `app` / library `app_lib` (edition 2024, minimum Rust 1.90).
- **Boundary:** frontend bridge → Tauri commands/channels in `src-tauri/src/lib.rs` → provider client (`client.rs`), credential storage (`secrets.rs`), and harness (`harness.rs`).
- **Persistence:** Tauri app-data `state.json`; credentials in OS credential storage. No SQLite. Browser fallback is not equivalent to desktop persistence.
- Top-level `app/` is a separate egui prototype, not the active Tauri UI.

## 3. Verified architecture

### Initialization and readiness

Tauri setup resolves app-data, loads `AppState`, initializes the `tauri-plugin-log` file target in Tauri's app log directory, initializes separate audit logging, records startup outcomes, and calculates readiness. A persisted-state load failure aborts setup; logging or audit initialization failures are reported to stderr and do not themselves abort startup. `get_startup_readiness` exposes the report.

**Readiness is partial:** `readiness_report()` returns a fixed component list. Some statuses derive from startup flags, provider metadata, active project, shell resolution, and a skill-discovery call. Directory writability, independent persistence components, project skill discovery, and provider connectivity are not comprehensively validated. Application configuration and tool registry are reported ready statically. No network probe runs. Messages do not include provider secrets. Treat this as preliminary diagnostics, not a complete health guarantee.

### Providers and agent execution

`client.rs` implements Ollama and OpenAI-compatible request/stream formats used for OpenRouter, Google AI Studio's compatible endpoint, and generic OpenAI-compatible endpoints. Provider metadata is persisted; API keys use the OS credential store and the UI receives availability metadata, not key values. Model discovery and native Anthropic Messages support are absent.

The active chat path is `chat_send_stream`: it uses thread context and loops through provider responses and tool calls until a final response or error/limit. `chat_send` is a separate one-shot path, not the agent loop. Stream chunks carry text and lifecycle/approval/usage information. Cancellation accepts an optional stream ID; omitting it cancels all active streams. Seam-level tests do not establish exhaustive provider/tool end-to-end cancellation behavior.

### Persistence, memory, token accounting

`AppState::load` loads `state.json`, defaults missing state to empty, and rejects malformed JSON and empty/duplicate project IDs. Persistence serializes to a temporary file then replaces the state file. Providers, projects, threads/messages, approval mode, and conversation token usage are persisted; credentials are separate. Memory/learning proposals are conversation/project-associated data requiring user review before becoming project guidance or skills; this is not a separate vector-memory database.

Provider-reported usage is stored per thread/request when available and restored/aggregated by the UI. Missing values remain unknown; estimates are not provider-reported usage. Providers may omit fields.

### Skills, tools, approval, sandbox

`harness.rs` discovers validated `SKILL.md` catalogs from project/global roots. Metadata is available before full instructions; the agent may request activation and validated resource loading. Skills and project `AGENTS.md` are untrusted task guidance. Discovery errors may be logged while returning an empty or partial catalog, so readiness success does not prove all intended roots were scanned.

Manual approval is the default. `write_file`, `patch_file`, and `run_command` require approval unless automatic mode is enabled. Read/search/list/Git and skill activation are not in that approval set. Learning proposals are staged for user review.

Workspace file operations use `harness::Sandbox` rooted at the canonical project directory, with path validation, symlink restrictions, and bounded I/O. Canonicalization does not eliminate hostile concurrent filesystem TOCTOU races. `run_command` executes with user OS privileges, outside the file sandbox. The interactive terminal is separate and has similarly broad OS access.

### Logging and audit

Diagnostic logging uses Tauri's `app_log_dir`: release `Info+`; debug `Debug+` plus stdout; 5 MiB rotation; four retained files. Initialization failure is non-fatal and reported to stderr. See `INTEGRATION-GUIDE.md` for paths.

`audit.rs` writes JSONL with IDs, timestamps, a SHA-256 hash chain, rotation, and Unix directory/file permissions. Emission before initialization is a no-op; append failures are warnings in diagnostics. Declared event variants do not prove call-site coverage. Selected startup, agent, provider, stream, skill, approval, and shutdown events are emitted, but declared tool/sandbox categories and per-operation outcomes are incomplete. Hash chaining does not provide an external immutable sink or guarantee complete coverage.

## 4. Implemented functionality

Labels rely on source inspection; checks run during this review are shown separately below.

| Capability | Classification | Evidence / boundary |
|---|---|---|
| Tauri 2 + React/TypeScript frontend | IMPLEMENTED AND VERIFIED (build) | `tauri-react/package.json`; current build passed. |
| Desktop JSON state and thread persistence | IMPLEMENTED BUT NOT FULLY VERIFIED | `AppState::load` / `persist`; no packaged restart test in this review. |
| Provider configuration and credential storage | IMPLEMENTED BUT NOT FULLY VERIFIED | `save_provider`, `client.rs`, `secrets.rs`; no live provider test in this review. |
| Agent loop, tool dispatch, streaming | IMPLEMENTED BUT NOT FULLY VERIFIED | `run_agent_loop`, `chat_send_stream`; browser fallback is simulated. |
| File sandbox and approval for writes/commands | IMPLEMENTED BUT NOT FULLY VERIFIED | Constrains file tools, not shell execution. |
| Skill discovery and activation | IMPLEMENTED BUT NOT FULLY VERIFIED | Discovery/activation exist; scope and error limitations above. |
| Provider-reported usage persistence/UI aggregation | IMPLEMENTED BUT NOT FULLY VERIFIED | `TokenUsageRecord`, `tokenUsage.ts`; provider fields vary. |
| Diagnostic logging | IMPLEMENTED BUT NOT FULLY VERIFIED | Setup exists; packaged path/rotation not tested here. |
| Audit JSONL/hash chaining | IMPLEMENTED BUT NOT FULLY VERIFIED | `audit.rs`; coverage and append-failure visibility incomplete. |
| Startup readiness endpoint and Settings view | PARTIALLY IMPLEMENTED | Several statuses inferred/static; refresh does not reinitialize components. |
| Integrated terminal | IMPLEMENTED BUT NOT FULLY VERIFIED | Commands exist; cross-OS runtime not verified here. |

## 5. Partially implemented functionality

- **Readiness:** statuses exist, but component-specific checks, directory/persistence validation, and active project skill roots are incomplete. Refresh rereads the report; it does not initialize/recover components.
- **Audit:** selected event call sites exist; there is no complete event matrix or coverage guarantee for declared tool/sandbox/provider/stream events. Append failures do not fail operations and are visible only through diagnostics.
- **Skills/instructions:** discovery/loading exist; nested task scope is not automatically wired into tool execution. Scan errors can yield partial results.
- **Token usage:** provider-reported fields are authoritative; responses may omit them. No model-matched tokenizer is promised.
- **Cross-platform:** source has platform-specific behavior, but this audit does not certify packaged runtime on Windows/macOS/Linux.

## 6. Known limitations

- Browser-only mode is a demo: threads are in memory; chat/provider behavior is simulated.
- Shell commands and integrated terminal run with current-user privileges; file sandboxing does not restrict their filesystem, network, or process access.
- Path checks leave a TOCTOU window.
- No native Anthropic Messages API, vendor OAuth/CLI integrations, model discovery, SQLite, arbitrary document upload, command allowlist, or OS-level shell containment.
- No startup provider connectivity checks; this avoids network delays.
- Audit declarations exceed verified emission coverage; no external immutable audit sink.
- Project `AGENTS.md` scope is root-only in current chat flow; nested scope requires callers to supply a nested task directory.

## 7. Outstanding issues

1. Make readiness component-specific and test success/failure cases, including config corruption, directory access, and persistence failures.
2. Complete the audit coverage matrix; test JSONL lifecycle, write failures, tool/sandbox outcomes, and cancellations.
3. Verify packaged app-data/log/audit paths, rotation, credentials, terminal, and sandbox per supported OS.
4. Add provider-mocked tests for streaming failures, cancellation, retries, request context, and usage parsing.
5. Decide whether nested project instruction/skill scope is required; wire and document accordingly.

## 8. Security and sandbox assessment

Credentials are intended to remain in OS credential storage and readiness messages do not return them. Audit fields filter identifiers/outcomes; emitter contract prohibits prompts, file contents, credentials, and arbitrary provider errors. Review every new diagnostic/audit call site for sensitive-data leakage.

The file sandbox constrains workspace file tools only. Approved shell commands retain user privileges, environment, and network access. Approval is user consent, not OS isolation. Path canonicalization leaves a TOCTOU window; no adversarial process containment is claimed.

## 9. Testing and verification results

### Run during this documentation review

- `npm test`: **31 passed, 0 failed**.
- `npm run build`: **passed** (TypeScript compilation and Vite production build).
- `cargo check --lib`: **passed**, with existing warnings (unused `mut`; audit variants not constructed).
- `cargo fmt --check`: **passed**.
- `cargo test --lib readiness_component_serializes_safe_status_contract`: **1 passed**.
- `git diff --check`: **passed**; Git emitted LF-to-CRLF working-copy warnings.

These checks verify compilation and selected tests only. No release build, packaged launch, log rotation inspection, or cross-platform runtime test was done in this review.

### Historical verification records

The previous version recorded dated results for earlier work, including Rust suites of 50 and 58 tests, a 26-test frontend run, and a release build. These were not rerun and are not current verification. Older P0–P3 task lists and contradictory completion claims are removed from current-status sections.

## 10. Release readiness assessment

**Not release-certified by this audit.** Current frontend build, Rust library check, formatting, and focused tests pass. No `cargo build --release`, packaged install/launch, upgrade validation, audit/log rotation inspection, or per-OS test was performed here. Close or explicitly accept readiness/audit coverage gaps before making stronger release claims.

## 11. Remaining work

Prioritize truthful readiness checks and failure tests, close or scope audit coverage gaps, and add packaged OS-specific verification. Update this assessment with dated results only after executing those checks.

## Historical notes

The former P0–P3 completion checklist and dated runtime/sandbox audit entries have been consolidated into the current architecture, limitations, and security sections. Historical test counts are retained only as records, not current results.
