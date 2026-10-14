# Mew TUI Implementation Plan

**Status:** Phase 10 is active. The first full-screen TTY shell slice is implemented; this does not claim the complete TUI rewrite or a GUI is complete.

## Product boundary

The terminal application remains a first-class Rust frontend. A separate native desktop frontend now targets Windows, macOS, and Linux in `crates/mew-desktop`; its first vertical slice reuses the canonical config, provider/model registry, sessions, and runtime. Packaging and update expectations remain open. Keep provider, session, agent, tool, skill, workflow, MCP, and configuration implementations canonical; every frontend should present and invoke them rather than clone their behavior. Preserve non-interactive/piped commands and existing CLI semantics.

## Current milestone

The chat prompt has a TTY-only crossterm editor with UTF-8-aware cursor movement, history, slash completion, and a resize-aware shell showing the saved transcript, provider/model/mode/session, composer, and navigation hints. PageUp/PageDown scroll the transcript, and `--inline` selects the prior scrollback-style presentation; piped/non-TTY input remains line-oriented. This is an incremental shell layer on the existing command loop: command output and approval prompts still use the existing handlers, and the shell redraws at the next prompt rather than owning every progress/error event yet. Workspace-edit approvals now have a visibly distinct panel but retain the same exact diff and existing approval callback.

Miku remains original Mew-specific work: a fixed-grid pixel cat, idle/blink loop, thinking/running expressions, and local `/miku`/`/meow` actions that do not call a model or tool. An OpenAI-compatible provider's HTTP 401 is translated into specific expired/invalid-key recovery guidance without echoing the provider response body; `mew setup` is the replacement path.

This is an incremental TUI layer, not a new runtime or a completed full-screen application. Existing command execution, transcript storage, approvals, and session behavior remain authoritative while the terminal frontend evolves.

## Next implementation stages

### Stage 1 — Persistent chat shell

Continue the resize-aware shell by routing runtime progress, cancellation, errors, and approvals into explicit shell states instead of inline output. The first integration now consumes existing runtime lifecycle events and preserves the same request, approval, and `--inline`/non-TTY paths.

### Stage 2 — Command and context navigation

Use the keyboard command palette and searchable session/model/provider selectors through existing list/switch handlers. Searchable workflow/pipeline selection and JSON input forms now invoke the existing validated run paths. Continue contextual project/file references and editor handoff only where current Mew behavior supports them; do not create parallel definitions or permission paths.

### Stage 3 — Preference and accessibility polish

Configurable keybindings are implemented in user preferences. Continue TUI-specific accessibility polish; themes and reduced-motion/static presentation are implemented. Test terminal widths, Unicode cell widths, resize, approval prompts, history, cancellation, and both TTY and piped operation. Check the actual Termux/low-resource path before enabling optional effects by default.

### Stage 4 — Desktop frontend

The target is a local native desktop app for Windows, macOS, and Linux, implemented as a separate Rust frontend over the canonical runtime. The first egui shell provides provider/model selection, saved-session navigation, and read-only Ask-mode chat. Packaging/update expectations, desktop approval UX, and broader skills/MCP/workflow navigation remain follow-up work; do not fork command/tool behavior into a second backend.

## Design and source rules

Miku's name, art, animations, expressions, and copy are original Mew-specific work. Use interface concepts from the reference survey as design guidance, but do not copy source or distinctive branding from proprietary/unlicensed projects. OpenCode's MIT and Vercel fx's Apache-2.0 sources are candidates for narrow ports only after checking the exact file, asset, dependency, and notice. MiMo Code also has a separate use-restrictions document. Crush's current FSL competing-use terms make its code off-limits for this similar coding-agent UI absent specific clearance or permission. Claude Code and Antigravity source/expressive assets are not porting candidates. See [TUI reference research](tui-reference-research.md) for the opened official sources and constraints.

## Verification strategy

1. Keep pure editor/state logic unit-tested: Unicode editing, cursor movement, history boundaries, completion, and viewport width.
2. Smoke-test rendered UI at tiny/narrow/normal/wide terminal widths; assert stable cell widths and preserve Miku sprite dimensions.
3. Test provider authentication errors against a local mock 401 response; assert setup guidance and prove the response body is not surfaced.
4. Run `cargo fmt --all -- --check` and `cargo test --workspace` after each UI milestone. Verify CLI smoke behavior with and without TTY input; do not require provider secrets for these checks.
5. Keep the existing command/runtime boundary and non-interactive behavior covered by regression tests.

## Milestone update — 2026-10-14

This slice now adds a compact status ribbon on every chat prompt and a keyboard-searchable command palette. The palette includes built-in actions and project/global custom slash commands, and hands selections back to the existing command parser. `/model` and `/models` can search authorized model catalogs; `/providers` can lead into a provider's models; `/sessions` and bare `/resume` search saved sessions; `/skills` searches skills and opens the selected instructions. First-run setup now shows Miku's original pixel portrait and uses searchable provider/model choices when stdout is a TTY, preserving the line-oriented fallback. Both `mew --help` and `mew help` advertise the same quick-start routes.

## Milestone update — 2026-10-14 (runtime states and launch forms)

The full-screen shell now renders active and completed states using lifecycle events emitted by the existing runtime, including model-request, tool-start/finish, and agent-run events. `Ctrl+C` cancels in-flight agent, text-provider, and workflow/pipeline futures; saved-session behavior remains in place. Miku uses original thinking, running, celebrate, and error poses, with periodic animation redraws while requests or saved runs are pending. Full-screen errors keep actionable details visible in the transcript area, including provider setup/recovery errors, while the compact status line retains the state. Existing approval callbacks and exact diff review are unchanged.

`/workflow`, `/pipeline`, `/workflows`, and `/pipelines` now offer searchable run selection in a TTY and an object-shaped JSON input prompt; direct command syntax remains available for scripts and non-interactive use. The UI calls existing validated workflow/pipeline functions and does not change their read-only permission constraints. `mew style motion reduced|full` persists the existing animation preference, which is honored by chat prompts, mascot commands, inline spinners, and full-screen progress. Focused UI/core/CLI tests cover event forwarding, error layout, picker routing, reduced motion, and cancellation signaling. Configurable keybindings are implemented; real Termux usability validation remains open. The separate desktop app is tracked in Phase 12 and remains a distinct, incomplete frontend.

The initial full-screen shell is wired into bare chat, explicit `mew chat`, and saved-session resume. Its transcript viewport and composer are resize-aware and keyboard-scrollable; runtime progress/cancellation/error states, validated workflow/pipeline launch forms, reduced-motion support, and configurable keybindings are implemented. Exact-diff approval is still performed by the existing callback, with a separate approval panel. Real Termux usability validation remains open. Existing agent, workflow, MCP, provider, and permission paths stay authoritative. Gemini API OAuth is implemented through the documented provider-login command; the first-run wizard still supports OpenRouter/OpenAI API keys only. Do not port private Antigravity/Claude Code mechanisms; OAuth follows documented provider-supported flows, with consent, callback, and token-storage behavior reviewed for each target platform.

### Desktop milestone — 2026-10-14

The target OS question is resolved: Windows, macOS, and Linux. `crates/mew-desktop` is a separate native egui app, not a browser product or a replacement for the TUI. The first working slice loads the shared config/provider registry, displays configured provider/model choices and saved sessions, saves new chat turns, and routes tool-capable models through `mew-core` with `AgentMode::Ask`, no external tools, no auto-approval, and no execute/network/write permissions. Text-only models use the same configured provider for ordinary chat. Cross-platform release builds and packaging remain unverified.
