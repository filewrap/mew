# mew

> a tiny coding cat with sharp claws.

```bash
mew
mew setup
mew chat
mew ask "what does this repo do?"
mew fix "handle malformed input in the parser"
mew council "compare two implementation approaches"
mew skills list
mew name set paww
mew style preview
```

## What is mew?

`mew` is an open-source Rust agent system built around a reusable orchestration backend, with a CLI frontend for low-resource users, Termux, Linux, macOS, and Windows. Agents can use different providers, models, tools, skills, and execution limits.

## Runtime backend

`mew-runtime` is the core product surface; CLI commands are one frontend, not the agent architecture. It provides typed run results and lifecycle events, risk-gated tool registries, bounded subagent delegation, token-aware context compaction, MCP tool adapters, dependency-based workflows, and sequential pipelines. The workspace agent, authored workflows, pipelines, and subagents all execute through this runtime.

Rust applications can embed the library directly by registering `AgentProfile`s with a `RuntimeBuilder`, then calling `AgentRuntime::run`, `run_workflow`, or `run_pipeline`. Agent profiles carry their own provider/model, system prompt, skills, tools, and context limits. Workflow steps declare dependencies and can consume prior results using `{{steps.<id>.output}}` or `{{steps.<id>.response_json}}`.

Agent responses use a structured content envelope for text, JSON, media references, artifacts, or opaque provider-specific parts. Built-in providers currently populate text; the envelope is an extension point, not a claim that all built-in models already support multimodal output. Response text uses ordinary Markdown as its base (Markdown v3); Mew's homegrown v4 layer adds optional typed quote and suggestion-button presentation without replacing that base. The shared parser removes valid hidden button annotations from the clean Markdown body and exposes inert suggestions as structured data. See [`docs/markdown-v4.md`](docs/markdown-v4.md) for the syntax and limits.

## Current phase

The reusable runtime and its CLI surfaces now cover native agent runs, authored subagents, dependency workflows, and sequential pipelines across the supported provider backends. See [`docs/PHASES.md`](docs/PHASES.md) and [`docs/architecture.md`](docs/architecture.md).

## Agent modes and safety

`mew ask` and `mew review` use bounded, read-only workspace tools. They can list/read/search project files and inspect Git status/diffs; they cannot edit files, run project commands, or access the network by default. Runtime subagents also start read-only, even when the parent run has broader capabilities.

```bash
mew ask "find where the config is loaded"
mew review
mew agent "explain the project structure" --mode ask
mew fix "correct the parser bug"
```

Edits use exact, unique text replacements. `mew` displays a diff and asks before each change; it rejects paths outside the workspace. Use `--yes` only when you intentionally want to approve every proposed edit and any enabled optional actions without prompts. Package checks are opt-in with `--allow-checks` in edit mode. Network access is opt-in with `--allow-network`, requires per-request approval, uses HTTPS only, blocks private/reserved destinations and redirects, and caps response bodies. Explicit `auto` mode is separate: it enables workspace edits and arbitrary local shell commands without per-action prompts, while keeping Mew-managed network and external/MCP tools disabled. Shell commands still have whatever network access the current OS account provides; treat Auto as local code execution.

Native structured calls use each provider's protocol: OpenAI, OpenRouter, and custom OpenAI-compatible adapters use function tools; Gemini uses native `functionDeclarations`, `functionCall`, and `functionResponse`. A model must be marked tool-capable to run the interactive workspace agent or to delegate to subagents. The built-in Gemini 2.5 models are enabled for tools, including when loading older default configurations.

## Workflows, pipelines, and subagents

Definitions are workspace-local TOML files under `.mew/workflows/` and `.mew/agents/`. Create a complete starting file, edit it, validate it, then run it:

```bash
mew workflow init code-review
mew workflow validate code-review
mew workflow run code-review --input '{"target":"src/"}'

mew pipeline init release-checks
mew pipeline run release-checks --input-file task.json

mew subagent init docs-reviewer
mew subagent list
mew subagent run docs-reviewer "Check the documentation for inaccuracies"
```

Workflow and pipeline documents use format version 1. A workflow runs dependency-ready stages in bounded parallel batches; a pipeline chains stages in order. Each step selects an `agent_id`, prompt, and optional `depends_on`. Inline `[[agents]]` entries and reusable `.mew/agents/<id>.toml` profiles select their own `provider/model`, `system_prompt`, skills, and limits. If `model` is omitted, the active model is used. Profiles may use different supported backends in the same workflow. Tool-capable profiles receive workspace-bounded read-only file and Git tools, and can delegate to the other registered profiles.

Templates are `{{input}}`, `{{input.path}}`, `{{steps.<id>.output}}`, and `{{steps.<id>.response_json}}`; references to earlier steps must also appear in `depends_on`. Workflow files are validated before any model call. Workflow stages receive read-only permissions, and each run returns JSON with per-step status, response, usage, transcript, and errors. For example:

```toml
version = 1
id = "review"
kind = "workflow"
max_parallel_steps = 4
fail_fast = true

[[agents]]
id = "reviewer"
model = "openai/gpt-4.1-mini"
system_prompt = "Review the requested workspace area. Cite concrete files and findings."
skills = ["review"]

[[agents]]
id = "security"
model = "gemini/gemini-2.5-pro"
system_prompt = "Inspect the same task for security concerns and report evidence."

[[steps]]
id = "correctness"
agent_id = "reviewer"
prompt = "Review {{input.target}} for correctness and missing tests."

[[steps]]
id = "security"
agent_id = "security"
prompt = "Review {{input.target}} for security issues."

[[steps]]
id = "summary"
agent_id = "reviewer"
depends_on = ["correctness", "security"]
prompt = "Combine these reviews into prioritized actions:\n{{steps.correctness.output}}\n{{steps.security.output}}"
```

Subagent definitions have the same profile fields and are automatically registered for `mew agent` delegation. `mew subagent run <id> <prompt>` runs one profile directly. Every delegated run is bounded and children receive read-only permissions.

For example, `.mew/agents/docs-reviewer.toml`:

```toml
id = "docs-reviewer"
model = "gemini/gemini-2.5-flash"
system_prompt = "Review documentation for clarity, accuracy, and examples that disagree with the implementation. Cite file paths and lines."
skills = ["review"]
max_steps = 8
# context_tokens and response_reserve may also be set per profile.
```

## Skills and context budget

Built-in skills include `caveman`, `review`, and `tests`. Add project-specific Markdown skills in `.mew/skills/<name>.md`, list them with `mew skills list`, and load them explicitly with `--skill <name>`.

The token budgeter estimates input size when the provider does not report usage, compacts older turns within the selected model’s context limit, and preserves structured tool-call/observation pairs. Provider-reported token counts are preferred in the run summary.

## Mew Council

```bash
mew council "design a safe project-wide config migration"
```

The command runs planner, coder, and reviewer passes and exports a Markdown transcript to `.mew/council/`.

## MCP

`mew mcp serve` runs a read-only native-tools MCP server over stdio (JSON-RPC). It exposes workspace file listing/reading/search and Git inspection tools.

```bash
mew mcp tools -- <server-command> [args...]
mew mcp call <tool-name> '<arguments-json>' -- <server-command> [args...]
```

The MCP client starts the supplied server process directly (never through a shell), checks its advertised tool list before calls, and uses stdio transport. Embedding applications can register advertised MCP tools with `mew-runtime`; they are classified as external actions and require an explicit capability and approver.

## Development

```bash
cargo build --workspace
cargo run -p mew-cli -- --help
cargo test --workspace
cargo run -p mew-cli -- doctor
cargo run -p mew-cli -- init --dry-run
cargo run -p mew-cli -- style preview
```

## Install

### Termux / Linux smart installer

```bash
curl -fsSL https://raw.githubusercontent.com/filewrap/mew/main/scripts/install.sh | bash
```

Custom repo:

```bash
MEW_REPO_URL=https://github.com/filewrap/mew bash scripts/install.sh
```

### Cargo

```bash
cargo install --git https://github.com/filewrap/mew --package mew-cli --force
```

## Providers

On the first bare `mew` launch, a short setup wizard checks the current folder and settings path, lets you connect **OpenRouter (recommended)** or **OpenAI**, verifies the key against the provider, fetches available chat models, and saves your choice. API keys are entered without echo and saved separately from `config.toml`; on Unix, the credentials file is restricted to the current user (`0600`). The file is local and not encrypted, so keep access to your user account protected. You can instead export a provider key in your shell before setup. The built-in Gemini provider also supports Google's documented Desktop OAuth flow; see [advanced TUI features](docs/advanced-tui-features.md) for setup. This is Gemini API OAuth, not Antigravity CLI authentication.

```bash
mew                 # first run: setup; after setup: chat
mew setup           # run setup again/change provider or model
mew chat             # start interactive CLI chat directly
mew provider list
mew provider test openai
mew provider test openrouter
mew provider login gemini   # open browser for optional Google Gemini API OAuth
mew provider test gemini
```

Custom OpenAI-compatible endpoints can be added without putting a key in the config file. Set the referenced environment variable in your shell, then validate it:

```bash
mew provider add local-llm http://localhost:1234/v1 LOCAL_LLM_API_KEY my-model
export LOCAL_LLM_API_KEY="..."
mew provider test local-llm
mew model use local-llm/my-model
```

Public custom endpoints must use HTTPS; plain HTTP is accepted only for localhost/loopback.

## Models

Only authorized providers are shown by default.

```bash
mew model list
mew model list --all
mew model list openai
mew model list openrouter
mew model list gemini
mew model list openrouter --remote
mew model use openai/codex-mini-latest
mew model show
```

## Chat and sessions

```bash
mew
mew chat
mew session list
mew session show <id>
mew session resume <id>
```

Inside chat, native workspace tools are available when the selected model supports structured tool calls. `/mode ask` keeps file access read-only; `/mode edit` enables exact-match patches with a visible diff and approval for every change; `/mode auto` enables local edits and arbitrary shell execution but disables Mew-managed network and external/MCP tools; shell commands retain OS-level network access. Configured local stdio MCP servers are loaded automatically from global `~/.config/opencode/opencode.json` (or `$XDG_CONFIG_HOME/opencode/opencode.json`), project `opencode.json`/`.opencode/opencode.json`, or `.mew/mcp.json`; in non-Auto modes, MCP calls are external actions and still require per-call approval. This chat path currently supports local stdio MCP servers, not remote transports. Every built-in and discovered workspace skill is active for chat. See [advanced TUI features](docs/advanced-tui-features.md) for keybindings, model favorites/variants, exact-action grants, lifecycle hooks, managed background commands, worktrees, offline sharing, Auto mode, and Gemini OAuth.

Useful in-chat controls:

```text
/                         show the command menu and custom commands
/model [#N|provider/model] show models and switch by row or id
/providers · /models      inspect providers and configured models
/provider test <id>       check an API connection
/provider add <id> <url> <key-env> <model>
/agents                  list built-in and workspace agents
/agent <id> <task>        directly ask a named agent (read-only delegation)
/skills · /skill <name>   list always-on skills or show one skill
/mcp                      list automatically loaded MCP servers
/connect                  inspect provider setup
/new · /clear             start a fresh session
/undo · /redo              undo/restore a chat exchange (not file changes)
/compact · /summarize      summarize earlier conversation
/editor · /export         compose in $EDITOR or export Markdown
/share · /unshare         create/remove an offline local HTML transcript
/favorite · /favorites    pin/list favorite models
/variant                 choose a generation preset for this model
/keybindings             list configurable terminal bindings
/approvals               inspect or revoke exact-action grants
/background              list Mew-managed background commands
/worktree list           list this repository's worktrees
/hooks status            inspect project lifecycle-hook setup
/sessions · /resume <id>  list or switch saved sessions
/continue                 resume the latest saved session
/exit · /q                save the active session and exit
/workflows · /pipelines  list saved definitions
/workflow <id> [JSON]     run a validated workflow
/pipeline <id> [JSON]     run a validated sequential pipeline
/cwd · /cd <path>         inspect or change the active workspace
/init                    index the current project
```

The bare `mew` first launch runs setup; once setup is complete, `mew` opens chat. `mew chat` always requests chat directly. All built-in and `.mew/skills/*.md` skills are loaded automatically and remain active when a session is resumed. Create custom slash commands as Markdown files in `.opencode/commands/`, `.mew/commands/`, or `~/.config/opencode/commands/`; the file name becomes the command (for example, `test.md` becomes `/test`). Commands support `description`, `agent`, `model`, and `subtask` frontmatter, `$ARGUMENTS`, positional `$1`/`$2` placeholders, and `@file` references. Shell-output placeholders (an exclamation mark followed by a backtick command) require explicit confirmation before execution. Workflow/pipeline tools are also available to the chat agent, while their definitions remain read-only and validated before execution.

## License

Apache-2.0
