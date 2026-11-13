# Issue 6 Technical Documentation & Verification Report
## Correct Context Construction, Compaction, and Token Accounting

### 1. Executive Summary

This report documents the audit, architectural design, implementation, and automated test verification for **Issue 6: Correct Context Construction, Compaction, and Token Accounting** in OpenHarness.

The context pipeline has been hardened to ensure:
- Deterministic, hierarchical context construction without duplicate message or attachment injection.
- Strict pairing of tool calls and tool results with validation to prevent dangling calls or orphan results.
- Robust thread compaction that preserves conversational context, keeps tool-call/result pairs intact, and retains immutable message history.
- Resilient recovery of persisted tool executions and malformed transcript progress without silent loss.
- Precise token accounting distinguishing between pre-request heuristic estimates and provider-reported usage, with client-side deduplication that prevents missing metadata from blocking subsequent usage records.
- Enforcement of provider context limits with mandatory reservation of configured output tokens.

---

### 2. Context Pipeline Analysis & Instruction Hierarchy

#### 2.1 Context Construction Order
When constructing model request payloads in `tauri-react/src-tauri/src/lib.rs` (`run_agent_loop` and `chat_send_stream`), context components are assembled deterministically in the following strict hierarchy:

1. **System Prompt (`system`)**: Root assistant identity, core operating principles, and safety boundaries.
2. **Project Instructions (`system`)**: Discovered from `AGENTS.md` along the canonical project directory chain. Untrusted task guidance that cannot override safety or sandbox rules.
3. **Skill Catalog Context (`system`)**: Discovered skill metadata and descriptions available for dynamic activation via `activate_skill`.
4. **Selected Skills (`system`)**: Explicitly activated skill contents, demarcated as untrusted guidance.
5. **Workspace State Notice (`system`)**: When no project root is active, explicitly notifies the model that filesystem, terminal, and git tools are unavailable.
6. **Compacted Summary (`system`)**: When earlier messages have been compacted, the stored summary is injected as prior context (`Previously compacted conversation summary: ...`).
7. **Conversation History (`user` / `assistant` / `tool`)**: Uncompacted historical messages starting from `compacted_message_count`.
8. **Current User Turn (`user`)**: The latest user prompt.
9. **Non-Image Text Attachments (`user`)**: Text attachments appended with deduplication to ensure content already present in the prompt or restored messages is not duplicated.
10. **Image Attachments**: Retained directly on their source messages as multimodal data URLs or structured image payloads.

#### 2.2 Provider Payload Normalization (`client.rs`)
In `client::messages_json_checked`:
- Contiguous leading system messages are merged into a single system block.
- Any system messages appearing after the conversation starts are demoted to `user` role to adhere to strict provider requirements (e.g., Anthropic / OpenRouter constraints).
- Consecutive messages of identical roles (without tool calls) are concatenated with clean line breaks.
- Multimodal data is formatted appropriately: Base64 image payloads for standard providers (`image_url`), or raw image arrays for Ollama (`images`).

#### 2.3 Tool Call & Tool Result Pairing Contract
The context pipeline enforces strict validation for all tool interactions:
- **No Orphan Results**: Every `tool` message must reference a preceding `assistant` message that declared a matching tool call ID.
- **No Unresolved Calls**: An assistant message with `tool_calls` must be immediately followed by matching `tool` results before any new `user` or `assistant` message is processed.
- **No Dangling Calls at Context Tail**: If a model request ends with an unresolved tool call, `messages_json_checked` rejects the payload rather than transmitting broken state to the provider.
- **Unique Call IDs**: Duplicate tool call IDs within an assistant turn or across unresolved sets are rejected.

---

### 3. Token Accounting & Budgeting Architecture

#### 3.1 Token Estimation
Function `estimate_context_tokens` in `tauri-react/src-tauri/src/lib.rs` calculates heuristic input tokens across all context elements before calling the provider:
- **Text Tokens**: Estimated across all message contents, system instructions, and supporting attachments using character-to-token heuristics.
- **Multimodal Allowance**: Dedicated fixed overhead (1,000 tokens per image data URL) is budgeted for visual inputs.
- **Tool Definitions**: Full JSON schema serialization for all registered tools is budgeted into the input size.

#### 3.2 Output Token Reservation & Context Limits
- Function `context_fits(estimated_tokens, max_context_tokens, max_output_tokens)` enforces:
  $$\text{estimated\_tokens} \le (\text{max\_context\_tokens} - \text{max\_output\_tokens})$$
- Provider requests fail fast if input context would encroach upon the configured `max_output_tokens` reservation buffer.

#### 3.3 Distinguishing Estimated vs. Provider-Reported Usage
- **Heuristic Telemetry**: Pre-request token estimations are emitted with `source: "estimated"`.
- **Provider-Reported Telemetry**: Live stream parsers capture provider-returned token metrics (prompt, completion, and total tokens) from SSE chunks and emit them with `source: "provider_reported"`.

#### 3.4 Client-Side Deduplication & History Restoration (`tokenUsage.ts`)
- In `tauri-react/src/components/tokenUsage.ts`, `addUsageOnce` deduplicates usage events by `requestId`.
- Finite numeric validation is performed before registering a request ID; missing or malformed usage metadata does not consume the request ID, ensuring subsequent valid usage packets are processed.
- Historical token usage is restored cleanly across conversation reloads without double counting.

---

### 4. Compaction & Conversation Restoration

#### 4.1 Safe Compaction Boundary (`compaction_prefix_boundary`)
When a thread exceeds the token compaction threshold (`THREAD_COMPACTION_THRESHOLD_TOKENS = 12,000` tokens):
- Compaction isolates a prefix of historical messages for summarization.
- At least the 6 most recent messages are preserved untouched.
- If the proposed boundary splits an assistant tool-call from its tool results, `compaction_prefix_boundary` steps backward to keep all tool calls and their results together.
- Summarization is performed via the model; the summary is saved in `thread.compacted_summary` and `thread.compacted_message_count` is updated.
- Original message history is **never deleted**, preserving an immutable audit log.
- Optimistic concurrency protection: if thread messages are modified while compaction is running, the compaction result is rejected and thread history remains intact.

#### 4.2 Conversation Restoration & Transcript Recovery
- Function `chat_message_to_model_messages` reconstitutes stored chat messages into model messages.
- Stored tool progress records (`[Completed tool result: <name>, call_id=<id>]\n<output>`) are parsed and restored as paired `tool` result messages.
- Malformed tool progress records (missing `call_id` or truncated headers) are preserved as assistant text rather than discarded, guaranteeing zero silent context loss.
- Text attachments stored on chat messages are restored via `append_text_attachments` with deduplication against existing message content.

---

### 5. Automated Regression Test Suite

The implementation is verified by a suite of automated unit and integration tests across both the Rust backend and TypeScript frontend.

#### 5.1 Rust Backend Test Coverage
1. `context_pipeline_tests::restored_tool_progress_preserves_assistant_text_and_call_identity`
   - Validates that assistant messages with persisted tool completions are split into assistant prose and structured `tool` result messages with matching call IDs.
2. `context_pipeline_tests::malformed_persisted_tool_progress_is_preserved_not_dropped`
   - Validates that malformed tool progress lacking call IDs is preserved as assistant text.
3. `context_pipeline_tests::truncated_persisted_tool_header_is_preserved_not_dropped`
   - Validates that truncated tool headers at message boundaries are retained without panic or truncation.
4. `context_pipeline_tests::context_budget_does_not_count_text_attachments_twice`
   - Validates that text attachments present in both messages and arguments are not double-counted in token budgeting.
5. `context_pipeline_tests::attachment_context_does_not_duplicate_text_from_persisted_user_message`
   - Validates that user attachments matching existing prompt contents are not duplicated.
6. `context_pipeline_tests::context_budget_reserves_output_tokens`
   - Verifies the boundary conditions of `context_fits`.
7. `context_pipeline_tests::compaction_boundary_keeps_persisted_native_tool_call_with_results`
   - Validates that compaction boundaries rewind to preserve tool calls and their results in the same segment.
8. `client::tests::messages_json_checked_rejects_duplicate_tool_call_ids`
   - Verifies rejection of duplicate call IDs in assistant tool calls.
9. `client::tests::messages_json_checked_rejects_tool_result_without_call`
   - Verifies rejection of tool results that do not correspond to an active tool call.
10. `client::tests::messages_json_checked_rejects_unresolved_tool_calls_before_user_message`
    - Verifies rejection of intermediate user messages while tool calls remain pending.
11. `client::tests::messages_json_checked_rejects_dangling_tool_calls_at_end`
    - Verifies rejection of requests ending with unanswered tool calls.
12. `client::tests::image_attachment_is_not_treated_as_text_file`
    - Validates that raw data URLs and markdown image links are recognized strictly as image payloads.
13. `tests::context_budget_reserves_output_and_includes_messages_tools_and_images`
    - Tests multilingual content (`你好`), tool definitions, and image payloads in composite token budgeting.
14. `tests::compaction_stores_summary_without_deleting_thread_history`
    - Verifies thread history immutability after compaction.
15. `tests::compaction_rejects_changed_history_without_mutating_thread`
    - Verifies race condition protection when thread history changes during compaction.

#### 5.2 TypeScript Frontend Test Coverage (`tokenUsage.test.ts`)
1. `aggregates token metrics correctly across requests`
2. `deduplicates reports with the same request ID`
3. `allows subsequent valid usage when initial report has missing/zero metadata`
4. `restores historical thread usage without double-counting active run metrics`

---

### 6. Verification Results

| Suite | Command | Result | Pass Count |
|---|---|---|---|
| Rust Backend Tests | `cargo test --lib` | PASS | 159 passed, 1 ignored |
| Rust Formatting | `cargo fmt --check` | PASS | 0 formatting issues |
| Rust Clippy | `cargo clippy --all-targets --all-features -- -D warnings` | PASS | 0 warnings |
| Frontend Tests | `npm test` | PASS | 63 passed |
| Frontend Production Build | `npm run build` | PASS | TypeScript check & Vite build successful |

---

### 7. Acceptance Criteria Verification Matrix

| Requirement / Acceptance Criterion | Implementation Mechanism | Verification Evidence |
|---|---|---|
| **Deterministic context pipeline** | Hierarchy in `run_agent_loop` & `chat_send_stream` | Verified in `context_pipeline_tests` & `INTEGRATION-GUIDE.md` |
| **Instruction hierarchy preserved** | System prompt -> AGENTS.md -> Skills -> Summary -> History -> User | Unit tests & code audit |
| **Tool call / result pairing** | `messages_json_checked` strict validation | 5 unit tests in `client.rs` verifying pairing and rejection of dangling/orphan calls |
| **Preserve history during compaction** | `compaction_prefix_boundary` + immutable storage | `tests::compaction_stores_summary_without_deleting_thread_history` |
| **Never discard incomplete messages** | Fallback to assistant text for malformed progress | `malformed_persisted_tool_progress_is_preserved_not_dropped` |
| **Distinguish estimated vs actual usage** | Telemetry source tagging (`estimated` vs `provider_reported`) | Stream callbacks and telemetry records |
| **Prevent double counting on restore** | Request ID deduplication in `tokenUsage.ts` | `tokenUsage.test.ts` suite |
| **Respect context windows & reserved tokens** | `context_fits` with `max_output_tokens` check | `context_budget_reserves_output_tokens` |
| **Multilingual, code & multimodal context** | Multilingual UTF-8 streaming, data-URL image budgeting | `image_payload_tests`, `fragmented_utf8_is_preserved` |
| **Zero MCP dependencies** | Verified no MCP imports or libraries introduced | Cargo manifest and source audit |
