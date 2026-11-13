# OpenHarness Agent Guide

## Project layout

- `tauri-react/`: primary Tauri 2 + React 19 + TypeScript desktop app.
- `tauri-react/src-tauri/`: Rust backend, Tauri commands, and agent runtime.
- `app/`: separate Rust/eframe application crate.
- Root `Cargo.toml`: another standalone Rust crate. These Rust crates are not one Cargo workspace.

## Working rules

- Preserve unrelated user changes. Check `git status --short` before editing.
- Keep changes scoped; follow existing patterns and avoid new dependencies unless required.
- Never expose secrets, API keys, or private chain-of-thought in logs or UI.
- Run the narrow relevant test/check first, then applicable full checks. Report commands and actual results; never claim unrun checks passed.
- Do not commit unless explicitly asked.

## Windows PowerShell commands

Run frontend and Tauri commands from `tauri-react/`:

```powershell
npm install
npm run dev
npm test
npm run build
npx tauri dev
npx tauri build --no-sign
```

- `npm run dev`: starts Vite at `http://localhost:1420`.
- `npm test`: runs the tests configured in `tauri-react/package.json`.
- `npm run build`: TypeScript check followed by Vite production build.
- `npx tauri dev`: launches the desktop app in development mode; requires frontend dependencies and Rust toolchain.
- `npx tauri build --no-sign`: builds local installers without signing. Output is under `tauri-react/src-tauri/target/release/bundle/`.

Rust backend checks from `tauri-react/src-tauri/`:

```powershell
cargo check
cargo test --lib
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

- For format fixes, run `cargo fmt` in the same directory.
- Run `cargo clippy` when changing Rust code; address new warnings or report pre-existing failures.

Standalone Rust crates:

```powershell
cargo check --manifest-path app/Cargo.toml
cargo test --manifest-path app/Cargo.toml
cargo check --manifest-path Cargo.toml
cargo test --manifest-path Cargo.toml
```

Only run the standalone crate commands when the corresponding `app/` or root Rust crate is in scope.

## Debugging workflow

1. Reproduce the issue and identify whether it is in React/Vite, the Tauri Rust backend, or a standalone Rust crate.
2. Inspect relevant code and tests; use `git status --short` to protect unrelated edits.
3. For frontend-only failures, run `npm test` and `npm run build` from `tauri-react/`.
4. For Tauri/backend failures, run `cargo check` and focused Rust tests from `tauri-react/src-tauri/`; launch `npx tauri dev` from `tauri-react/` when runtime behavior needs reproduction.
5. For standalone Rust failures, run the relevant `--manifest-path` command above.
6. Fix the smallest cause, add/update a focused regression test for non-trivial behavior, then rerun the relevant checks.
7. Review `git diff` and report root cause, changed files, checks run, and any remaining failure.

## Definition of done

- The requested behavior is implemented with no unrelated changes.
- Relevant tests and checks have been run, with results reported accurately.
- `git diff` has been reviewed; unresolved failures or deferred work are stated clearly.
