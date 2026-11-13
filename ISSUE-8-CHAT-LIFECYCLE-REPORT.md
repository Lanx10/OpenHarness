# Issue 8 — Chat Lifecycle Audit & Progress Report

## Lifecycle observed

1. `Canvas.runPrompt` sets the run busy, creates a thread if needed, appends the user message, creates an assistant draft, and assigns a UUID stream ID.
2. `chat_send_stream` validates inputs, registers that ID (rejecting concurrent duplicates), then runs the agent in a Tokio task.
3. The agent issues provider requests with per-run request IDs, streams text/usage/steps, executes tools, and propagates cancellation into approvals/processes.
4. Completion or failure is recorded in the thread with `run_id` and `outcome`; failures preserve available partial output and a safe notice.
5. The Tauri command sends terminal stream data, returns, and drops the RAII registration. The frontend refreshes the thread and resets busy/stop state in `finally`.

## Risks found in this audit

- Frontend previously replaced partially streamed assistant content with the error string on provider/IPC failure. Now it retains the partial text and appends the failure notice; both callback and rejected-command paths are covered by `toolLifecycle.test.ts`.
- Cancellation previously marked the in-progress assistant draft `done`; it now marks that draft `cancelled` in both error-delivery paths.
- Persisted backend outcomes are created after the task settles; persistence failure can still leave the frontend/backend temporarily divergent. The existing `finally` reload is best-effort and suppresses refresh errors.
- Stream registration and cancellation are ID-scoped, reject duplicate active IDs, and clean up through RAII. Existing tests cover duplicate registration and targeted cancellation at the registry seam; provider/tool cancellation and app shutdown are not end-to-end tested here.
- There is no backend idempotency key for duplicate user-message submissions. UUID stream IDs prevent duplicate active stream IDs, not repeated user intent or replay after uncertain persistence.
- Provider/tool requests have correlation IDs, but structured actionable error types are not consistently exposed; most failures are strings.
- No automatic provider retry is performed. This avoids replaying side-effecting tools without an idempotency protocol, but transient provider failures require a new user action.
- Frontend disconnection and actual Tauri channel delivery failures have not been simulated. `Channel.send` failures are ignored for intermediate chunks; final command result remains the delivery fallback when IPC is live.

## Change in this step

- `stepLifecycle.withFailureNotice` appends a failure notice without discarding partial text and avoids appending the same notice twice. `Canvas.tsx` uses it for both streamed error callbacks and command rejection. Both user and assistant messages receive terminal error/cancelled UI states; cancellation no longer labels an unfinished assistant response as complete.
- `AppState.shutdown` now cancels all active registered streams and clears the registry, with verified cancellation propagation via oneshot channel (`shutdown_cancels_all_active_registered_streams`).
- `register_chat_stream` rejects parallel active streams on the same thread to prevent race conditions during history mutation, compaction, or persistence (`a_second_stream_for_the_same_thread_is_rejected`). Concurrent streams on different threads continue to be supported (`streams_for_different_threads_can_run_concurrently`).
- Fixed streaming text emission: provider streaming emits tokens as they arrive; on completion of a non-tool turn, only any unstreamed suffix (or authoritative divergence) is emitted rather than repeating the entire final response (`streamed_final_reply_is_not_emitted_twice`, `only_the_unstreamed_suffix_is_emitted`).

## Verification

- `npm test` — passed: 65 tests, 0 failed.
- `npm run build` — passed: TypeScript and Vite production build.
- `cargo test --lib` — passed: 166 passed, 1 ignored (includes `chat_lifecycle_tests` and `stream_registry_tests`).
- `cargo fmt --check` — passed: clean formatting.
- `cargo clippy --all-targets --all-features -- -D warnings` — passed: 0 warnings.
- `git diff --check` — passed: no whitespace or line ending issues.

## Remaining work

- Add persisted-message idempotency for duplicate submissions; require an explicit key plus atomic persistence semantics.
- Surface typed error codes and lifecycle correlation metadata to the frontend.
- Exercise stream disconnects, provider interruption, persistence failures, and cancellation using a test transport and Tauri/runtime harness.
- Decide retry policy only after provider calls/tools can be safely distinguished and side effects are idempotent.
