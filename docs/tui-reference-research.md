# Mew TUI Reference Research

**Reviewed:** 2026-10-14
**Scope:** Official documentation/repository review for the requested interface references. No external code was copied. Repository files and license/notice details must be checked at the exact revision before any source is ported.

## Project findings

| Reference | License/source status | Useful patterns for an independent Mew implementation |
|---|---|---|
| [OpenCode](https://github.com/anomalyco/opencode) | Official repository is MIT. Candidate UI packages include `packages/tui`, `packages/ui`, and `packages/theme`; the fetched directory listings do not clear individual files, assets, or dependencies. | Slash commands plus command palette; searchable provider/model pickers; skills and MCP discovery; session switching, editor handoff, file references, configurable keybindings/themes. Keep provider auth separate from model choice. |
| [MiMo Code](https://github.com/XiaomiMiMo/MiMo-Code) | Repository is MIT **and has a separate [USE_RESTRICTIONS.md](https://github.com/XiaomiMiMo/MiMo-Code/blob/main/USE_RESTRICTIONS.md)**. Check both it and file-level notices before reuse. Reviewed docs describe vivid/minimal visuals and note animation performance concerns; they do **not** establish a Miku/pet feature. | Keyboard-first command/model selection, customization, vivid/minimal or reduced-motion treatment, and simple first-run setup. Miku's identity, art, animation, and copy should be original to Mew. |
| [Claude Code](https://github.com/anthropics/claude-code) | Proprietary; its [LICENSE.md](https://github.com/anthropics/claude-code/blob/main/LICENSE.md) reserves rights and refers to commercial terms. No core implementation source was cleared for porting. | Independently implement documented workflow stages, plan-before-edit, explicit manual/edit/auto risk modes, modular skills/instructions, resumable sessions, and separate enforcement/approval hooks. |
| [Google Antigravity CLI](https://github.com/google-antigravity/antigravity-cli) | Reviewed GitHub metadata reports no license (`license: null`); no reusable source candidate was established. Treat the implementation and distinctive assets as proprietary. | Independently implement browser OAuth and remote-terminal code handoff, model selection, skill discovery, keyboard shortcuts, and compact focus overlays. Do not copy its source or expressive UI. |
| [Charmbracelet Crush](https://github.com/charmbracelet/crush) | Current main uses **FSL-1.1-MIT**, not an immediately unrestricted MIT grant. It excludes defined Competing Use and offers a version-specific future MIT grant after two years. A commercial Mew TUI may be substantially similar; do not port source without version/use clearance or permission. | Component separation (transcript, composer, dialogs, lists, diff, notifications), explicit progress/cancellation, deterministic/cached animation, cell-width awareness, reduced motion. Use as design inspiration only unless source clearance is established. |
| [Vercel fx](https://github.com/vercel-labs/fx) | Official repo is Apache-2.0; source is in Zig. Check exact files and `THIRD_PARTY_NOTICES.md`; preserve required notices, mark modifications, and include applicable NOTICE attributions if code is ported. | Inline scrollback-friendly transcript; UI boundaries for input/transcript/footer/approval/resize; resumable sessions; separate scriptable and interactive flows. |

## Primary references opened

### OpenCode

- [TUI guide](https://opencode.ai/docs/tui/)
- [Models](https://opencode.ai/docs/models/)
- [Skills](https://opencode.ai/docs/skills/)
- [MCP servers](https://opencode.ai/docs/mcp-servers/)
- [Configuration](https://opencode.ai/docs/config/)
- [MIT license](https://raw.githubusercontent.com/anomalyco/opencode/dev/LICENSE)
- [TUI package](https://github.com/anomalyco/opencode/tree/dev/packages/tui), [UI package](https://github.com/anomalyco/opencode/tree/dev/packages/ui), [theme package](https://github.com/anomalyco/opencode/tree/dev/packages/theme)

### MiMo Code

- [Official overview](https://mimo.xiaomi.com/mimocode/start)
- [Repository/README](https://github.com/XiaomiMiMo/MiMo-Code)
- [MIT license](https://github.com/XiaomiMiMo/MiMo-Code/blob/main/LICENSE)
- [Use restrictions](https://github.com/XiaomiMiMo/MiMo-Code/blob/main/USE_RESTRICTIONS.md)
- [Themes](https://mimo.xiaomi.com/mimocode/themes)
- [Keybindings](https://mimo.xiaomi.com/mimocode/keybinds)
- [Installation](https://mimo.xiaomi.com/mimocode/install)

### Claude Code

- [Repository/license](https://github.com/anthropics/claude-code/blob/main/LICENSE.md)
- [Common workflows](https://docs.anthropic.com/en/docs/claude-code/common-workflows)
- [Skills](https://docs.anthropic.com/en/docs/claude-code/skills)
- [Memory/instructions](https://docs.anthropic.com/en/docs/claude-code/memory)
- [Permission modes](https://docs.anthropic.com/en/docs/claude-code/permission-modes)
- [Permissions](https://docs.anthropic.com/en/docs/claude-code/permissions)
- [Hooks](https://docs.anthropic.com/en/docs/claude-code/hooks-guide)
- [CLI reference](https://docs.anthropic.com/en/docs/claude-code/cli-reference)

### Google Antigravity CLI

- [Product overview](https://antigravity.google/product/antigravity-cli/)
- [Install/auth](https://antigravity.google/docs/cli/install/)
- [CLI overview](https://antigravity.google/docs/cli/overview/)
- [Prompting](https://antigravity.google/docs/cli/prompting/)
- [Features](https://antigravity.google/docs/cli/features/)
- [CLI reference](https://antigravity.google/docs/cli/reference/)
- [Skills](https://antigravity.google/docs/skills/)
- [Repository](https://github.com/google-antigravity/antigravity-cli)
- [Repository API metadata](https://api.github.com/repos/google-antigravity/antigravity-cli)

### Crush

- [Repository](https://github.com/charmbracelet/crush)
- [Current license](https://github.com/charmbracelet/crush/blob/main/LICENSE.md)
- [Configuration guide](https://github.com/charmbracelet/crush/tree/main/docs/config)
- [UI component tree](https://github.com/charmbracelet/crush/tree/main/internal/ui)
- [Animation implementation](https://github.com/charmbracelet/crush/blob/main/internal/ui/anim/anim.go)
- [Skills tree](https://github.com/charmbracelet/crush/tree/main/.agents/skills)

### Vercel fx

- [Repository](https://github.com/vercel-labs/fx)
- [Apache-2.0 license](https://raw.githubusercontent.com/vercel-labs/fx/main/LICENSE)
- [UI source tree](https://github.com/vercel-labs/fx/tree/main/src/ui)
- [Vercel AI Gateway/fx docs](https://vercel.com/docs/ai-gateway/coding-agents/fx)
- [Tools and background commands](https://fx.sh/docs/capabilities/tools)
- [Sessions](https://fx.sh/docs/using-fx/sessions)

## Porting guidance

1. Preserve Mew's existing Rust runtime, CLI commands, provider configuration, agent/workflow definitions, skills, and MCP behavior behind a distinct terminal presentation/input layer.
2. Begin with a testable keyboard prompt/editor, transcript/status surfaces, and model/session/command pickers that call existing commands. Keep non-interactive CLI paths intact.
3. Implement manual/edit/automatic interaction modes as Mew-owned policy presets only when backed by current runtime permissions; the UI must not pretend prompt text enforces policy.
4. For code reuse, inspect and pin exact OpenCode or Vercel fx files first, then audit per-file notices, bundled assets, and dependencies. Review MiMo's extra restrictions before considering it. Defer Crush source until clearance; do not copy Claude Code or Antigravity source.
5. Create Miku and all Mew-specific art, animation frames, labels, and personality independently.

**Research limitation:** Findings summarize official sources opened during the review. Package trees were inspected but not every candidate source file or transitive dependency was individually audited. This is source triage, not legal advice or file-by-file license clearance.


## Pinned clone source inspection (2026-10-14)

Three eligible repositories were cloned outside the Mew worktree at `/workspace/mew-reference-sources/` for direct inspection; no source code was copied into Mew. Clone identifiers are OpenCode `6a7cedcbf1`, MiMo Code `002cb4c9`, and Vercel fx `f244a6fb` (short commit IDs from the current clones; record the full SHA before relying on them as long-term pins).

The actual [OpenCode command palette source](https://github.com/anomalyco/opencode/blob/dev/packages/tui/src/component/command-palette.tsx) builds filtered options from registered commands, exposes titles/descriptions/categories/shortcuts, hides the palette command itself, can prepend suggested actions, and adds settings choices. The [model picker](https://github.com/anomalyco/opencode/blob/dev/packages/tui/src/component/dialog-model.tsx) separates connected-provider state, recents and favorites; offers fuzzy search, free-model labeling, provider/integration selection, favorite toggling, and variant selection. These are reference patterns only; the inspected SolidJS components are not Rust and have not been ported.

The actual [MiMo command dialog source](https://github.com/XiaomiMiMo/MiMo-Code/blob/main/packages/cli/src/cli/cmd/tui/component/dialog-command.tsx) registers enabled/visible command options, includes aliases and stable English identifiers as searchable terms even when labels are localized, shows keybinding/category metadata, and gates shortcut actions while text editing. MiMo's root [MIT license](https://github.com/XiaomiMiMo/MiMo-Code/blob/main/LICENSE) lists Xiaomi/MiMo Code (2026) and OpenCode (2025) notices. Its separately reviewed [use restrictions](https://github.com/XiaomiMiMo/MiMo-Code/blob/main/USE_RESTRICTIONS.md) prohibit unlawful or malicious use, rights violations, harassment/discrimination, military use, malicious cyber activity, unauthorized protected-data/platform access, and autonomous high-risk action without oversight. No MiMo code or assets were copied.

The actual [Vercel fx input-presentation source](https://github.com/vercel-labs/fx/blob/main/src/ui/footer/input_presentation.zig) is a Zig rendering/geometry module for composer rows and picker surfaces; it accounts for terminal display width, clipped rows, slash/file/model pickers, approval presentation, and bounded input geometry. It is not Rust and was inspected for architecture/edge cases, not copied. Its root [Apache-2.0 license](https://github.com/vercel-labs/fx/blob/main/LICENSE) requires recipients to receive the license, modified files to carry prominent change notices, applicable source notices to be retained, and NOTICE attributions to be reproduced if the work includes a NOTICE file. The cloned [third-party notices](https://github.com/vercel-labs/fx/blob/main/THIRD_PARTY_NOTICES.md) identify separate cuelume-derived UI sounds and Unicode data; neither is part of Mew's new prompt/Miku code.

The implementation in Mew so far is an independent Rust/crossterm prompt and original Miku animation; it does not embed these components or their expressive text/art. See the [Mew TUI plan](tui-plan.md) for the staged follow-up.

Full clone SHAs verified with a clean worktree: OpenCode `6a7cedcbf1823eeb1c8dfc4baf5a7951854f578b`; MiMo Code `002cb4c97bd1346829dbb08d98242913df4575d0`; Vercel fx `f244a6fbd31b32f48af64bd49506dc5fa50bf4aa`.

## Six-CLI synthesis (2026-10-14)

This supplement records findings verified in the six supplied item notes and their cited official sources. **Documented behavior** below is attributed to the reference; **Mew priorities** are independent design recommendations, not claims about the reference implementations. No source, artwork, branding, or distinctive prose was copied.

### Verified behaviors and useful patterns

| CLI | Verified, project-specific pattern | Evidence |
|---|---|---|
| [OpenCode](https://opencode.ai/docs/tui/) | Ctrl+P palette and persistent preferences; command templates, configurable keybindings, provider/model selection with recents/favorites/variants, lazy skills, MCP integrations, layered config, and durable/shareable sessions. Shared conversations publish history/metadata until unshared. | [Commands](https://opencode.ai/docs/commands/), [Models](https://opencode.ai/docs/models/), [Skills](https://opencode.ai/docs/skills/), [MCP](https://opencode.ai/docs/mcp-servers/), [Config](https://opencode.ai/docs/config/), [Share](https://opencode.ai/docs/share/) |
| [MiMo Code](https://github.com/XiaomiMiMo/MiMo-Code) | SSH workflow uses remote `mimo serve` and local `mimo attach`; Vivid/Minimal toggle addresses animation load. README documents build vs read-only plan modes, multi-provider setup, skill ranking/aliases, and opt-in `/dream` and `/distill` knowledge/workflow proposals. | [README](https://github.com/XiaomiMiMo/MiMo-Code), [License](https://github.com/XiaomiMiMo/MiMo-Code/blob/main/LICENSE), [Use restrictions](https://github.com/XiaomiMiMo/MiMo-Code/blob/main/USE_RESTRICTIONS.md) |
| [Claude Code](https://github.com/anthropics/claude-code) | Docs specify permission modes/rules, Markdown skills, layered advisory instructions distinct from enforcement, lifecycle hooks with pre/post semantics, resumable/worktree workflows, and print/pipeline modes with structured output and limits. | [Permissions](https://docs.anthropic.com/en/docs/claude-code/permissions), [Skills](https://docs.anthropic.com/en/docs/claude-code/skills), [Memory](https://docs.anthropic.com/en/docs/claude-code/memory), [Hooks](https://docs.anthropic.com/en/docs/claude-code/hooks), [CLI reference](https://docs.anthropic.com/en/docs/claude-code/cli-reference) |
| [Google Antigravity CLI](https://antigravity.google/docs/cli/overview/) | Docs describe SSH-friendly TUI/auth handoff, per-conversation model selection and mid-run switch semantics, plugin management, editor handoff/multiline prompt, and file-level artifact review with approve/reject and comments. | [Install/auth](https://antigravity.google/docs/cli/install/), [Models](https://antigravity.google/docs/models/), [Plugins](https://antigravity.google/docs/cli/plugins), [Prompting](https://antigravity.google/docs/cli/prompting), [Artifact review](https://antigravity.google/docs/cli/artifacts) |
| [Charmbracelet Crush](https://github.com/charmbracelet/crush) | README/docs describe multi-session/project context, model switching while retaining context, provider/model and command pickers, skills, and configurable UI preferences. Source-tree inventories show separated UI areas; detailed animation behavior was not verified. | [README](https://github.com/charmbracelet/crush/blob/main/README.md), [Config docs](https://github.com/charmbracelet/crush/tree/main/docs/config), [UI tree](https://github.com/charmbracelet/crush/tree/main/internal/ui) |
| [Vercel fx](https://github.com/vercel-labs/fx) | Docs describe scrollback-first searchable/copyable transcript, scoped approval grants/rules, persisted resumable sessions with recovery/compaction, lazy skills, and background commands. Tool results can be retained and retrieved without unbounded prompt context. | [Tools](https://fx.sh/docs/capabilities/tools.md), [Sessions](https://fx.sh/docs/using-fx/sessions.md), [Permissions](https://fx.sh/docs/configure-fx/permissions.md), [Skills](https://fx.sh/docs/capabilities/skills) |

### Original Rust priorities for Mew (recommendations)

1. Build typed action/command and provider/model registries; provide searchable palettes/pickers with descriptions, shortcuts, recents/favorites, stable IDs, and accessible reduced-motion rendering.
2. Define validated, versioned layered configuration and independently authored skill metadata; expose origins, permissions, and policy provenance. Load skills lazily and distinguish advisory instructions from enforced controls.
3. Make permission state explicit and testable (read-only plan, prompt, approved edits, scoped grants); preview actions and treat shell expansion/hooks as policy-gated operations.
4. Preserve sessions with resume/fork/compact/export, explicit privacy controls, and optional worktree isolation. Keep transcript, composer, pickers, approvals, and artifact review as distinct responsive state/surfaces.
5. Prioritize SSH-safe auth handoff, credential redaction, provider capability metadata, context-preserving model switching, bounded tool output, structured pipeline mode, and robust resize/scrollback behavior.

### Source, license, and use constraints

- **OpenCode:** inspected root MIT requires retaining copyright/permission notices in copies or substantial portions; this does not clear all files, dependencies, assets, or third-party materials. Figma skill materials have a separate notice excluding them from OpenCode MIT. Do not assume separate MiMo permissions apply. [Root license](https://github.com/anomalyco/opencode/blob/dev/LICENSE), [Figma notice in inspected clone](https://github.com/anomalyco/opencode/tree/dev/packages/plugins/figma/skills).
- **MiMo Code:** root MIT notices must be retained for copied substantial portions, but repository `USE_RESTRICTIONS.md` separately restricts specified uses; bundled assets/skills have their own terms. Do not copy source, assets, or branding without item-level review and legal clearance of restrictions. [MIT license](https://github.com/XiaomiMiMo/MiMo-Code/blob/main/LICENSE), [Use restrictions](https://github.com/XiaomiMiMo/MiMo-Code/blob/main/USE_RESTRICTIONS.md).
- **Claude Code:** inspected `LICENSE.md` reserves all rights and subjects use to Commercial Terms; public docs/examples do not grant product-source reuse. Do not copy code, prompts, UI, assets, branding, or prose. [License](https://github.com/anthropics/claude-code/blob/main/LICENSE.md).
- **Google Antigravity CLI:** inspected GitHub metadata reports `license: null`; inspected tree had no LICENSE and no application source. Public availability is not permission: do not port its code, assets, examples, or branding absent explicit clearance. [Repository metadata](https://api.github.com/repos/google-antigravity/antigravity-cli), [inspected tree](https://api.github.com/repos/google-antigravity/antigravity-cli/git/trees/9eebbedf051ffd75adb15e468f48b7b75be4b15b?recursive=1).
- **Crush:** FSL-1.1-MIT's Competing Use definition includes substantially similar functionality or a commercial substitute; its future MIT grant is version/date-specific. Given Mew's similar coding-agent CLI purpose, do not copy/adapt Crush source absent explicit clearance/legal review. Preserve applicable notices if later permitted. [Exact license](https://github.com/charmbracelet/crush/blob/main/LICENSE.md).
- **Vercel fx:** root Apache-2.0 is not file/dependency clearance; any selected material requires applicable license/copyright notices, modification marking, and NOTICE attribution where applicable. Audit third-party notices and file-specific terms; do not reuse branding, sounds, or artwork. [License](https://github.com/vercel-labs/fx/blob/main/LICENSE), [Third-party notices](https://github.com/vercel-labs/fx/blob/main/THIRD_PARTY_NOTICES.md).

**Failed item:** OpenCode's `/docs/sessions/` URL returned 404. Session lifecycle claims above are limited to the inspected TUI and Share documentation; no separate sessions-page claims are made. Other reported item research had no failures. These are research constraints, not legal advice or file-by-file clearance.


## Mew implementation and provenance audit (2026-10-14)

**Bottom line:** no source code, prompts, artwork, branding, or distinctive prose was copied or adapted from any of the six reference CLIs. The reference material informed product-level design choices only. Mew's TUI is implemented independently in Rust with `crossterm`; its selectors route into Mew's existing command/runtime paths. “Inspired by” below means a comparable user-facing pattern, not migrated implementation code or exclusive provenance.

| Reference | Mew features aligned with the reference | Not ported / important distinction | Mew evidence |
|---|---|---|---|
| OpenCode | Searchable slash-command palette and provider/model/session/skill/workflow/pipeline choices; relevance-ranked fuzzy search; model favorites, recents and generation variants; configurable bindings; offline local HTML conversation export. | Palette opens from `/` or Ctrl+P. Sharing is a local file, not a public share service; OpenCode-compatible command templates are not claimed. | `crates/mew-ui/src/selector.rs`; `crates/mew-common/src/config.rs`; `crates/mew-cli/src/commands/chat.rs` |
| MiMo Code | Searchable first-run provider/model choices and persisted reduced-motion preference. | Miku is original Mew work. No remote `serve`/`attach`, skill ranking/aliases, `/dream`/`/distill`, or equivalent Vivid/Minimal theme system. | `crates/mew-cli/src/commands/setup.rs`; `crates/mew-cli/src/commands/style.rs`; `crates/mew-ui/src/miku.rs` |
| Claude Code | Ask/read-only and approval-gated Edit behavior, saved/resumable sessions, Markdown skills, opt-in lifecycle hooks, Git worktrees, and a separate Auto mode. | Mew's own hook schema/events and mode policy are independent; exact-action grants are not a general Claude-compatible permission-rule language. | `crates/mew-cli/src/commands/chat.rs`; `crates/mew-cli/src/commands/hooks.rs`; `crates/mew-cli/src/commands/worktree.rs`; `crates/mew-tools/src/hooks.rs`; `crates/mew-core/` |
| Google Antigravity CLI | Model selection within a saved chat, skill discovery, and exact-diff approval for workspace changes. The built-in Gemini provider separately supports Google's documented Gemini API OAuth. | Gemini API OAuth is **not** Antigravity OAuth. No public third-party Antigravity token contract was established; Mew does not emulate its private login flow. No SSH/auth handoff, plugin manager, editor handoff, multiline composer, or artifact-review workflow. | `crates/mew-provider/src/oauth.rs`; `crates/mew-provider/src/gemini.rs`; `crates/mew-cli/src/commands/provider.rs` |
| Charmbracelet Crush | Resize-aware transcript/composer shell, searchable selectors, progress/error/cancellation states, saved sessions, reduced motion, and separate approval/diff surfaces. | Independent Rust implementation; no Crush source adapted. Its current FSL competing-use terms are why it remains inspiration-only. | `crates/mew-ui/src/shell.rs`; `crates/mew-ui/src/prompt.rs`; `crates/mew-ui/src/selector.rs`; `crates/mew-cli/src/commands/chat.rs` |
| Vercel fx | Transcript scrolling, `--inline`, saved/resumed/compactable sessions, TTY editor plus line-oriented piped input, exact-action session/workspace approval grants, and managed background-command start/status/output/cancel. | Grants are exact preview fingerprints, not a user-authored policy-rule engine; background results live only for the current Mew process. Inspected fx source was Zig; none was ported into Mew's Rust UI. | `crates/mew-ui/src/shell.rs`; `crates/mew-cli/src/commands/chat.rs`; `crates/mew-tools/src/lib.rs` |

### TUI hardening in this pass

The prompt editor and searchable selector now treat extended Unicode grapheme clusters as editing units: left/right, backspace/delete, word deletion, composer clipping, and picker backspace do not split combining sequences or joined emoji. This uses the generic `unicode-segmentation` crate; it is not code from any reference CLI. Regression tests cover combining accents, skin-tone/ZWJ emoji, and a family emoji viewport. Focused TUI/CLI package tests pass; a full-workspace test run fails while compiling the on-hold `mew-desktop` frontend because of egui/eframe API mismatches and a borrow error, not a crates.io/glutin resolution failure.


### Gap-closure implementation follow-up (2026-10-14)

The implementation adds persistent model favorites/recents, per-model temperature presets, configurable prompt/action keybindings, and local offline HTML conversation sharing. It also adds a distinct Auto mode for local edits and arbitrary shell execution; Mew-managed network and external/MCP tools remain disabled, while shell commands retain the OS account's own filesystem/network access. Exact-action session/workspace approval grants, disabled-by-default lifecycle hooks, bounded process-local background-command management, and Git worktree create/list/remove flows with path and ignored-file protections are implemented.

Google Gemini API OAuth uses the documented Desktop-app loopback/PKCE flow. This is **not** Antigravity OAuth; Mew does not reuse an unsupported private Antigravity login/token contract. The targeted CLI/core/common/session/provider/tools/terminal-UI test set passes (**80 tests**), `cargo fmt` and `git diff --check` pass, and CLI help smoke checks cover agent Auto mode, hooks, worktrees, and Gemini login/logout. No desktop frontend files were edited or built; full-workspace validation was intentionally not run while that GUI remains on hold.

### Implementation-level source review and selector follow-up (2026-10-14)

The actual implementation files in the available OpenCode, MiMo Code, and Vercel fx checkouts were inspected and compared with Mew's Rust selector, prompt, shell, and chat command paths. No source, assets, or expressive UI were copied.

- **OpenCode:** inspected `packages/tui/src/component/command-palette.tsx`, `dialog-model.tsx`, and `ui/dialog-select.tsx`. The generic selector ranks fuzzy matches across title/category/search text, weights title more strongly, and supports grouped/suggested entries. Mew already had bounded results, fuzzy subsequence filtering, cancellation, Unicode-grapheme query deletion, model favorites/recents, and model details; the concrete selector gap was that matches remained in source order. Mew now independently ranks label/value/description relevance, contiguousness, word boundaries, and gaps, with stable source-order tie breaking.
- **MiMo Code:** inspected `dialog-command.tsx`, `dialog-model.tsx`, `dialog-theme-list.tsx`, and TUI keybinding/theme/visual contexts, plus `LICENSE` and `USE_RESTRICTIONS.md`. Modal-safe key dispatch, curated model sections, and preview-then-confirm theme selection informed the comparison only; no MiMo implementation or assets were reused. Its separate use restrictions remain applicable in addition to its MIT notice terms.
- **Vercel fx:** inspected `src/ui/footer/input_presentation.zig`, picker and viewport modules, approval readiness/UI modules, and `src/ui/approval_screen.zig`, together with `LICENSE` and `THIRD_PARTY_NOTICES.md`. Its structured footer geometry and visibility-gated approval presentation are larger architectural candidates, not copied into this focused selector change. No Apache-2.0 source or third-party sound/Unicode material was ported.
- **Claude Code and Google Antigravity CLI:** no application implementation checkout was available in `/workspace/mew-reference-sources`; the local prior research notes remain secondary context only. Publicly readable docs/examples are not treated as implementation source or reuse permission.
- **Charmbracelet Crush:** no source checkout was available, and its FSL-1.1-MIT Competing Use restriction may apply to a similar coding-agent TUI. Its implementation was not inspected or used to guide this change; no clearance is claimed.

The new fuzzy ranking is an independent Rust implementation in `crates/mew-ui/src/selector.rs`; tests cover label relevance, multi-field matching, deterministic ties, and existing Unicode-safe query editing. After this change, `cargo test -p mew-ui -p mew-cli` passes all 57 tests and `cargo fmt --all -- --check` passes. `cargo test --workspace` remains blocked only by compile errors in the on-hold desktop frontend; no desktop files were changed. This records observed source behavior and Mew's own gap closure, not a code port or legal clearance for other material.


## Direct TUI source comparison for the full-screen rewrite (2026-10-14)

**Pinned source checkouts:** OpenCode `6a7cedcbf1823eeb1c8dfc4baf5a7951854f578b`; MiMo Code `002cb4c97bd1346829dbb08d98242913df4575d0`.

### Component-level review

- **OpenCode:** inspected the TUI `session-frame`, session message parts, prompt composer, command palette, spinner/spinner-frame, shimmer text, and tab-pulse implementations. The useful behavior patterns are distinct message/tool blocks; a continuously updated composer; searchable actions with descriptions/categories/shortcuts; and a deliberately timed sweep/pulse that redraws as an animation rather than relying on a static spinner glyph.
- **MiMo Code:** inspected the session route/footer, prompt and command-dialog components, startup loader, spinner/task list, background pulse, and starry-background source. The useful patterns are explicit session/status regions, keyboard-first command search, intentional loading/activity surfaces, and an animation preference that can turn motion off.
- **Mew before this pass:** `crates/mew-ui/src/prompt.rs` owned raw-mode input and full-frame redraws, while `shell.rs` had only a compact status/transcript arrangement; activity largely surfaced as a one-line status plus a small Miku expression. The command picker already existed behind `/` or Ctrl+P, but was not discoverable from a substantial home/composer surface.

### Mew changes made independently

- Reworked the full-screen view into a workspace/model/status header, bordered user/assistant/tool blocks, an original Miku home panel, and distinct error/activity cards. Long errors now appear once in their card instead of being repeated as both a header and a body string.
- Added a colored moving block-wave during active work, tied to the existing animation preference; reduced motion holds a stable bar and sprite frame.
- Replaced the single-row shell composer with a two-/three-line viewport, Shift/Alt+Enter line breaks, Up/Down movement across draft lines, width-aware clipping, and bracketed-paste handling. Enter still submits; existing slash execution and the searchable Ctrl+P palette are unchanged and their shortcuts are now surfaced.
- Added a persistent alternate-screen session with bracketed-paste lifecycle cleanup, hidden cursor during status snapshots, visible cursor while editing, and changed-row snapshot drawing to avoid clearing/flickering the whole screen every animation tick. Ordinary line-oriented/non-TTY operation remains unchanged.

### License and use-restriction review

OpenCode's inspected TUI source is covered by the repository MIT license (root notice: `Copyright (c) 2025 opencode`). MiMo Code's root MIT notice names `MiMo Code, Xiaomi Corporation` (2026) and `opencode` (2025), and the repository separately includes `USE_RESTRICTIONS.md`. Those restrictions were read; this UI-only change adds no new action/permission path and does not alter Mew's existing approval, cancellation, or execution controls. **No OpenCode/MiMo source code, assets, animation frames, or distinctive copy were copied into Mew**; the block sweep and layout are new Rust/crossterm code, and the Miku sprite remains Mew's original art. Accordingly no third-party code notice was triggered by this rewrite; the exact pins, license notices, and restriction review are retained here for traceability.

### Verification

The focused `mew-ui` and `mew-cli` test suites were run after the rewrite; the UI tests cover fixed-width layout, scrolling, animation/reduced-motion, duplicate-error prevention, the actual runtime model-request status string, and multiline editing. Record the final command results at delivery if additional workspace tests are run.

Final verification: `cargo test -p mew-ui` passed **30 unit tests and 4 UI smoke tests**; `cargo test -p mew-cli` passed **29 tests**. `cargo test --workspace` is blocked only by the separately paused `mew-desktop` crate: its existing code targets removed `egui::SidePanel`/`TopBottomPanel` and the old `eframe::App::update` API, and also has a pre-existing `model_id` borrow/move error. No desktop files were changed for this TUI task.


## OpenCode and MiMo source-tree audit and Mew follow-up (2026-10-14)

### What was inspected

The local checkouts at `/workspace/mew-reference-sources/opencode` and `/workspace/mew-reference-sources/MiMo-Code` were fully inventoried at the file-path level. The detailed source review focused on terminal UI/composer, input, theme, keybinding, renderer/session framing, and relevant tests. This was **not** a literal full-text read of every file in the roughly 8,500-file OpenCode tree or 2,000-file MiMo tree; binary assets, generated/vendor content, and unrelated packages were not all read. The checkouts contain mismatched filesystem-vs-Git path counts, and the discrepancy was not reconciled. This inventory is not a file-by-file license clearance.

### Transferable source findings

- **OpenCode:** the prompt is a textarea component with centralized action-key bindings; it separates visible text from structured attachment metadata, guards asynchronous paste/suggestion writes against stale editor state, and restores a failed submission only when the user has not started a newer draft. Its theme system uses semantic roles, supports dark/light modes, validates references, and preserves a cached first-frame palette. Selected tests cover paste normalization, Unicode cursor/history offsets, and responsive layout. Its Solid/reactive architecture and theme assets are not suitable to copy into this Rust/crossterm implementation.
- **MiMo Code:** textarea actions are separated from app-level keybindings, focus and disabled states are explicit, draft/history state can survive view changes, and footer status text is normalized/clamped so essential controls retain their width. Its root license is MIT and it also carries `USE_RESTRICTIONS.md`; bundled voice code, binary assets, and third-party skills have separate notices. No MiMo code, asset, theme, or branding was reused.
- **Mew:** the existing editor already uses a typed input buffer, bracketed-paste handling, resize-aware layout, explicit cursor placement, and a deterministic render-first shell. It already has a width-aware footer and semantic theme wrappers; the concrete visual defect was muted text/borders mapped directly to ANSI bright black. The current composer renderer emits a prompt marker only on the first input line; blank continuation rows contain spaces.

### Independent Mew changes

- Added a `Dim` palette role that uses terminal-relative intensity rather than hard-coded ANSI 90 (bright black); legacy themes using that role remain legible without changing their accent colors, and `no-color` remains unstyled.
- Made `mew-dark` the first-listed and default Mew-owned palette, with white primary text and cyan accents; fresh config now selects it.
- Added a regression test for the reported `/se\n\n` draft, asserting that the first row has the sole `›` marker and two blank continuation rows remain aligned. Isolated PTY runs against the installed CLI before and after rebuilding both produced one marker and blank aligned continuation rows; the rebuilt binary also emitted SGR 2 for muted roles and no SGR 90. The reported three-arrow view still does not reproduce from the current binary, so a pre-existing live process or different rendering path remains possible.
- No OpenCode/MiMo source, prompt, theme, artwork, bundled assets, or distinctive wording was copied or adapted. Root licenses and MiMo's separate use restrictions were reviewed; nested asset/dependency terms were not exhaustively cleared, so reuse remains out of scope.

### Verification

`cargo fmt --all -- --check`, `git diff --check`, and `cargo test -p mew-ui -p mew-common -p mew-cli` pass. The focused run passed 36 `mew-ui` unit tests plus 4 UI smoke tests, 4 `mew-common` unit tests plus 1 config smoke test, and 29 `mew-cli` tests. The release CLI was rebuilt and installed at `/usr/local/cargo/bin/mew`; the saved local theme is now `mew-dark`. Full workspace validation remains excluded because the desktop frontend is on hold and has known egui/eframe compilation failures; no desktop files were changed.
