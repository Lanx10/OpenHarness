# OpenHarness Canvas, Activity Crumbs & Chat Redesign Backlog

Consolidated specification for implementing chat redesign, activity crumbs, and canvas enhancements.

---

## 1. Chat Bubble & Message Redesign
- **User Message:**
  - Modern card container with subtle surface background (`#171917` / `#1c1f1b`), border, rounded corners (8–10px), aligned to right or full-width chat track.
  - Avatar or clear role identifier (`User`).
  - Formatted attachment pills if files are attached.
  - Floating/hover action row: Copy, Edit, Delete.
- **Assistant Message:**
  - Distinct subtle background or minimal flush layout with left accent or branding.
  - Avatar (`OpenHarness` brand mark or model provider tag).
  - Clean typography scale with dedicated spacing.
  - Floating/hover action row: Copy text, Regenerate/Retry (direct model retry), Edit, Delete.
- **Markdown & Code Rendering:**
  - Full GitHub-flavored markdown parsing (headings H1–H4, unordered/ordered lists, blockquotes, tables, links, bold/italic, horizontal rules).
  - Code blocks: syntax-highlighted blocks with language badge, line numbers, and 1-click "Copy Code" button.
  - Inline code tag styling with monospace font, subtle background and border.

---

## 2. Real-Time Activity Crumbs (Thinking, Read, Grep, Edit, Command)
- **Problem:** Currently `run_agent_loop` in Rust executes tools (`read_file`, `grep_search`, `write_file`, `run_command`, etc.) silently while the UI only shows `<p>Thinking…</p>`.
- **Backend Event Stream:**
  - Extend Tauri `StreamChunk` in `lib.rs` to emit `step` events:
    ```rust
    pub struct StepEvent {
        pub id: String,
        pub kind: String,   // "thinking" | "read" | "grep" | "edit" | "command" | "git"
        pub label: String,  // e.g. "Reading src/components/Canvas.tsx"
        pub status: String, // "running" | "done" | "error"
        pub detail: Option<String>,
    }
    ```
- **Frontend Step Crumbs Component (`MessageCrumbs.tsx`):**
  - Horizontal/vertical interactive pill list displayed inside the assistant response:
    - `[ ⟳ Thinking (1.2s) ]`
    - `[ 📄 Reading tauri-react/src/App.tsx · done ]`
    - `[ 🔍 Grepping "apiChatStream" · done (4 matches) ]`
    - `[ ✏️ Editing tauri-react/src/style.css · done ]`
    - `[ ⌨️ Running cargo check · done ]`
  - Collapsible/expandable details preview for each step (shows file path, grep query, or exit code).
  - Active step has subtle pulsing indicator or animated spinner.

---

## 3. Tool Approval & Error Handling
- In-chat inline approval card instead of blocking modal dialog:
  - Tool name, target path/command, diff/argument preview.
  - "Approve once", "Always allow for this session", and "Reject" buttons.
- Tool errors surfaced gracefully inside the step pill (`crumb-error`) with retry option.

---

## 4. Canvas Split-View & Artifact Workspace (ChatGPT Canvas-style)
- Dual-pane layout toggle: Chat on left/center, editable Canvas Document on right (collapsible, resizable).
- Interactive artifact editor:
  - Render code/markdown/documents in an editable editor panel (textarea or lightweight editor with line numbers).
  - Version history slider / diff view against previous turns.
  - Quick action toolbar: "Apply to workspace file", "Copy", "Regenerate", "Run in terminal".

---

## 5. Browser Side Panel
- Independent right-docked panel, not shared with Files/Git components.
- Address input handles URLs, localhost dev servers, and search queries.
- Back, forward, reload, viewport presets (desktop/tablet/mobile), and open externally.
- Display URL and loading/error state. Sites that block embedding must have a clear external-open fallback.
- Future additions: DevTools/console, capture/inspect page, clipboard URL shortcut.

---

## 6. Files & Git Panels
- File explorer: preserve directory state, handle large trees efficiently, and provide clear errors.
- Git: detect external repository changes; add search/filter and interactive staging/unstaging.
- Keep Files, Git, Browser as separate independent panels, each with its own width, state, and close behavior.

---

## 7. Terminal
- Keep the unified Terminal action; decide whether shell selection (PowerShell/CMD) should return as a session option.
- Add command history and optional tab renaming.
- Show when output is truncated at the 200,000-character limit.

---

## 8. Chat & App-Shell Follow-ups
- Add global search/command palette across threads.
- Add useful keyboard shortcuts and visible shortcut hints.
- Audit accessibility: focus visibility, keyboard navigation, labels, reduced-motion support.
- Preserve composer drafts and provide undo for draft edits.

---

## Implementation Order
1. Redesign user/assistant message layout and responsive behavior.
2. Improve Markdown and code-block rendering/actions.
3. Implement structured tool lifecycle events and activity crumbs end-to-end.
4. Improve approval/error presentation and tool output visibility.
5. Add Canvas split view and artifact editor.
6. Address Browser, Files/Git, Terminal, and app-shell follow-ups.

---

## Acceptance Criteria
- Chat bubbles render correctly at desktop and narrow widths; message actions remain keyboard accessible.
- Markdown examples render headings, lists, links, tables, blockquotes, inline code, and fenced code safely.
- Tool events show truthful lifecycle states, including approval requested, approved/rejected, completed, and failed; no hidden tool output is misrepresented as assistant reasoning.
- Canvas edits are explicit, previewable, reversible, and never write project files without user confirmation.
- Browser remains independently docked right; Files/Git panel state does not control or overwrite it.
- `npm run build` and `cargo check` pass; focused regression tests cover address parsing, chat rendering, and tool event lifecycle.

---

## Notes
- Treat generated or model-provided content as untrusted; escape/sanitize rendered Markdown and keep file/command approvals in place.
- The proposed designs are backlog items only. Do not treat them as implemented until code and tests verify them.
- Reduce scope to the smallest safe slice during implementation; avoid adding Monaco or other large dependencies unless a native/current dependency does not meet the editor requirement.
- Browsers can block iframe embedding by policy; preserve the external browser fallback.
- The old `missing-canvas-tool-items.md` backlog file has been deleted and superseded by this consolidated document.
- Ponytail: initial Canvas editor may start with plain text/code editing; add rich editor capabilities only when usage justifies them.

---

## Ready-to-Use Implementation Prompt

```markdown
Implement the backlog in `CANVAS_REDESIGN_BACKLOG.md` in OpenHarness. Start by inspecting the current chat, tool loop, stream event, Browser, Files/Git, Terminal, CSS, and test implementations. Preserve unrelated dirty changes and existing user approvals/security boundaries. Build the smallest useful vertical slice first: redesign user and assistant chat messages responsively, expand safe Markdown/code rendering using already-installed dependencies, then add truthful per-message tool activity crumbs by carrying structured lifecycle events from Rust `run_agent_loop` through the Tauri stream into React message state. Show thinking only as a generic activity state; never expose private chain-of-thought. Show tool name, concise action, and running/completed/failed/approval status; expose bounded tool output only where safe. Keep approvals for writes and commands. Keep Browser panel independent and right-docked; do not reuse Files/Git panel components. Add or update focused tests for address parsing, markdown safety, and tool lifecycle. Run `npm run build`, `cargo check`, and relevant tests. Do not claim Canvas split-view/editor or any other backlog item is done unless implemented and verified. Report changed files, verified commands, and deferred items.
```
