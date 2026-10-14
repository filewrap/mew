# mew phases

## Phase 0 — Lock, Name, Repo, Rules

Status: done

- [x] lock project name: mew
- [x] lock CLI binary: mew
- [x] lock default display name: mew
- [x] lock CLI-first direction
- [x] lock Rust stack
- [x] lock Termux/low-resource priority
- [x] lock default providers: OpenAI/Codex, Gemini, OpenRouter
- [x] lock rich Crush-like CLI aesthetic
- [x] lock rename/persona system
- [x] lock caveman skills direction
- [x] lock advanced token system direction
- [x] lock native tools + guard direction
- [x] lock agent-to-agent council direction
- [x] add README
- [x] add LICENSE
- [x] add SECURITY.md
- [x] add CONTRIBUTING.md

## Phase 1 — Rust Workspace + Beautiful CLI Shell

Status: done

- [x] create Cargo workspace
- [x] create `mew-cli`
- [x] create `mew-common`
- [x] create `mew-ui`
- [x] add `mew --help`
- [x] add startup greet
- [x] clear terminal before greet
- [x] add config paths
- [x] add config load/save
- [x] add name commands
- [x] add style commands
- [x] add doctor command
- [x] add `mew init --dry-run`
- [x] add phrase bank
- [x] add responsive terminal wrapper
- [x] add tiny/narrow/normal/wide layout classes
- [x] add tests

## Phase 1 Learnings

- [x] terminal UI must be more attractive and minimal
- [x] banner should feel vectorized and Claude Code-like
- [x] greet should clear terminal
- [x] blocks must wrap and adapt to narrow/wide terminals
- [x] rich output should not depend on TUI
- [x] normal spacing + responsive blocks > fixed giant boxes

## Phase 2 — Provider Brain v0

Status: done

- [x] create `mew-provider`
- [x] create `mew-session`
- [x] provider trait
- [x] OpenAI-compatible adapter
- [x] OpenAI/Codex preset
- [x] OpenRouter preset
- [x] Gemini provider
- [x] key-based config/env auth
- [x] default model config
- [x] per-provider models
- [x] list authorized provider models by default
- [x] list specific provider models
- [x] remote model listing for authorized providers
- [x] active model setting
- [x] custom OpenAI-compatible provider config
- [x] async session save/load/list
- [x] `mew provider list`
- [x] `mew provider test`
- [x] `mew model list`
- [x] `mew model list <provider>`
- [x] `mew model list <provider> --remote`
- [x] `mew model list --all`
- [x] `mew model use`
- [x] `mew model show`
- [x] `mew ask`
- [x] `mew chat`
- [x] slash menu in chat
- [x] streaming renderer for OpenAI-compatible providers
- [x] Gemini fallback stream renderer
- [x] rich markdown rendering
- [x] chat banner
- [x] reduced output flood with blocks/status lines
- [x] `mew session list`
- [x] `mew session show`
- [x] Termux-friendly install script
- [x] local check script
- [x] default `mew` enters endless chat loop
- [x] `/exit` saves and leaves session
- [x] ctrl+c saves and leaves session on next prompt
- [x] `mew session resume <id>`
- [x] compact model table: index/name/id
- [x] remote model list is clean table only
- [x] cute loading animation before/during calls
- [x] response footer with time/model/tokens/session
- [x] improved banner with more Claude/Crush-like visual feel

## Phase 2 Learnings

- [x] only key-based auth for now
- [x] model listing must help low-budget users find available small models
- [x] authorized providers should be prioritized
- [x] terminal output needs stops, blocks, slash menu, status lines
- [x] streaming makes mew feel alive
- [x] markdown must be handled before serious agent phases
- [x] first impression is provider + appearance together
- [x] default command should not print and exit
- [x] session resume is required before agent phase
- [x] model listing must be compact, not flood output
- [x] every model call should show progress and usage footer

## Phase 3 — Project Init + Context Sniffer

Status: done

- [x] `mew init` writes `.mew/`
- [x] `.mew/project.toml`
- [x] `.mew/repo-map.md`
- [x] `.mew/memory.md` (created only if missing; never overwritten)
- [x] AGENT.md / CLAUDE.md / GEMINI.md detection
- [x] create `mew-index` (persistent context index crate; writes `.mew/index.json`)

## Phase 4 — Native Tools + Guard

Status: done

- [x] create `mew-tools`
- [x] create `mew-guard`
- [x] fs tools (workspace-confined list/read/search and exact-match replace)
- [x] git tools (status and bounded diff)
- [x] http tools (explicit HTTPS opt-in, public-address resolution, no redirects, response cap)
- [x] package tools (fixed check/fmt/test commands behind edit-mode approval)
- [x] machine guard (canonical root, symlink/traversal rejection, read/output/time limits)

## Phase 5 — Agent Loop v1

Status: done

- [x] structured tool-call loop for models marked tool-capable
- [x] tool observations returned as provider tool messages
- [x] per-action permissions for edits, checks, and network
- [x] read-only ask mode (`mew ask`, `mew review`, `mew agent --mode ask`)

## Phase 6 — Edit/Fix/Review

Status: done

- [x] patch-first exact-match editing
- [x] diff preview and approval before each write
- [x] fix mode (`mew fix` / `mew agent --mode edit`)
- [x] read-only review mode (`mew review`)

## Phase 7 — Caveman Skills + Token Paw

Status: done

- [x] built-in and explicitly selected workspace skills
- [x] model-context token budgeting
- [x] compact older turns while preserving tool-call/observation pairs
- [x] provider token usage or a clear estimate in run summaries

## Phase 8 — Mew Council

Status: done

- [x] sequential role-based model talks
- [x] planner/coder/reviewer roles
- [x] Markdown transcript export to `.mew/council/`

## Phase 9 — MCP

Status: done

- [x] stdio JSON-RPC MCP client (`tools/list`, `tools/call`)
- [x] stdio native MCP server
- [x] read-only native tools bridge

## Phase 10 — Native Rust TUI (GUI is a separate frontend)

Status: in progress — first full-screen shell slice integrated; GUI/platform scope remains open

- [x] terminal-aware prompt editor with UTF-8-safe cursor editing and non-TTY fallback
- [x] arrow-key prompt history and slash-command/custom-command completion
- [x] responsive chat header with original fixed-grid Miku pixel cat
- [x] Miku thinking/running expressions in the live request indicator
- [x] Miku idle blink loop in the composer and local `/miku`/`/meow` animation
- [x] safe, actionable expired-provider-key recovery without echoing HTTP 401 bodies
- [x] persistent compact chat status ribbon and searchable keyboard command palette, including project/global slash commands
- [x] searchable model, provider-to-model, saved-session, and skill selectors backed by existing handlers
- [x] Miku-branded setup with searchable provider/model choices and a line-oriented fallback
- [x] quick-start guidance in both `mew --help` and `mew help`
- [x] initial full-screen TTY transcript viewport, fixed composer/footer, resize redraw, and PageUp/PageDown scrolling
- [x] explicit `--inline` scrollback-style fallback and unchanged non-TTY line input
- [x] separate approval-required panel before the existing exact-diff approval prompt
- [x] runtime lifecycle progress, in-flight cancellation, and visible error/completion states across the full-screen shell
- [x] searchable workflow/pipeline picker with JSON input launch form, reusing existing runtime and permission paths
- [x] persistent full/reduced motion preference honored by chat prompts, spinners, and full-screen progress
- [x] built-in theme selection through `mew style list` and `mew style set <theme>`
- [x] configurable, persisted keybindings for prompt and full-screen chat actions
- [ ] usability verification on Termux
- [x] Start the separate desktop frontend under Phase 12, without replacing this terminal shell

## Phase 11 — Reusable Runtime Backend

Status: runtime orchestration, authored CLI definitions, and native tool protocols for supported provider backends implemented

- [x] create `mew-runtime` as a frontend-independent library
- [x] run typed agent profiles with provider/model, skills, tools, and per-agent context limits
- [x] expose structured run results, tool-call traces, usage, metadata, and lifecycle events
- [x] add bounded, read-only-by-default subagent delegation
- [x] add workspace-authored subagent profiles with list/show/init/direct-run commands and automatic delegation registration
- [x] add validated, versioned TOML dependency workflows with CLI list/show/init/validate/run commands
- [x] add first-class sequential-pipeline CLI commands and runtime API with stage-to-stage dataflow
- [x] execute independent ready workflow steps in bounded parallel batches
- [x] preserve per-profile provider/model selection across OpenAI-compatible, OpenRouter, custom-compatible, and Gemini backends
- [x] implement Gemini native function declarations, calls, and function responses for agent tools/delegation
- [x] add structured response content parts for text, JSON, media references, artifacts, and opaque payloads
- [x] add guarded MCP-advertised tool adapters
- [x] route the existing workspace agent command through the runtime
- [x] test tool dispatch, subagent delegation, pipeline dataflow, cycle rejection, and UTF-8 output caps
- [x] define Markdown v3 as the base and implement Mew's silent v4 quote/button layer
- [ ] extend provider adapters to populate non-text response parts

## Implementation boundary

The CLI is a frontend over `mew-runtime`; reusable APIs and CLI definitions cover agents, subagents, workflows, pipelines, tools, skills, token budgeting, MCP tool adapters, and structured Markdown-v4 response presentation. Native structured calls are implemented using each supported provider's protocol; model-specific capability flags remain enforced. Current built-in provider responses populate text in the response envelope, and non-text media output remains adapter/model dependent. The Markdown v4 extensions are Mew-owned, optional presentation annotations layered over ordinary Markdown; see [`markdown-v4.md`](markdown-v4.md).

## Phase 12 — Cross-platform Desktop App

Status: in progress — the native read-only chat source slice is present, but `cargo test --workspace` now reaches compilation and fails on egui/eframe API and ownership errors in `crates/mew-desktop/src/main.rs`; desktop work remains parked

- [x] Confirm Windows, macOS, and Linux as desktop targets
- [x] Add a native Rust/egui desktop binary, separate from the CLI/TUI
- [x] Reuse canonical config, provider/model registry, session persistence, and `mew-core` runtime
- [x] Add saved-chat navigation, provider/model selection, a chat composer, runtime progress, and original Miku styling
- [x] Keep the first desktop mode strictly Ask/read-only; no edit, execute, network, or external-tool capability is granted
- [ ] Resolve egui/eframe API and ownership errors, then compile/test `mew-desktop` (desktop GUI work remains parked)
- [ ] Add explicit desktop approval UI before enabling any risk-bearing tool capability
- [ ] Integrate selectable skills, MCP, workflows, and additional workspace/session controls through existing APIs
- [ ] Define packaging/update/signing and verify builds on Windows and macOS
- [ ] Add GUI interaction tests and verify session/model edge cases
