# mew architecture

```text
mew-cli (terminal frontend) ──┐
mew-desktop (native GUI) ─────┴─ shared Rust application/runtime crates:
  ├─ mew-common       config, paths, shared types
  ├─ mew-ui           responsive terminal rendering
  ├─ mew-core         workspace-agent adapter, tools, and approvals
  ├─ mew-runtime      reusable agent/workflow execution backend
  │    ├─ mew-provider provider/model abstraction per agent
  │    ├─ mew-tokens   context budgeting, compaction, usage accounting
  │    ├─ mew-skills   skill context injected per agent
  │    └─ mew-mcp      external MCP tools registered as guarded runtime tools
  ├─ mew-session      persisted conversation transcripts
  ├─ mew-index        project snapshot and context index
  ├─ mew-guard        canonical workspace boundary and bounded reads
  ├─ mew-tools        filesystem, git, explicit checks, opt-in HTTPS
  ├─ mew-council      existing planner → coder → reviewer workflow
  └─ mew-markdown     shared Markdown baseline + Mew v4 response decoder
```

`mew-runtime` is the application backend; it is not coupled to terminal I/O. The CLI and desktop are separate frontends over shared Mew configuration, provider, session, and runtime crates. An embedding application registers named agent profiles with independent providers, models, prompts, skills, limits, and tool registries. The CLI's `mew-core::run_agent` entry point is a compatibility layer over that runtime and adapts the existing workspace tools and approval callback; the current desktop slice uses the same core adapter in read-only Ask mode.

Interactive chat uses the same runtime through a history-aware adapter. Native workspace tools can be toggled with `/tools on|off`; `/mode ask|edit` controls the permission set, with every edit still requiring a visible diff and explicit approval. Workspace profiles, bounded subagent delegation, selected session skills, and validated read-only workflow/pipeline tools are registered in the same chat runtime. `/cd` updates the canonical workspace root for subsequent file tools; text-only chat remains available for models without native tool-call support.

## Runtime capabilities

- **Agent runs:** typed run results, conversation transcript, tool-call trace, provider usage, token estimates, metadata, and structured lifecycle events.
- **Responses:** a forward-compatible content envelope for text, JSON, image/audio/video references, artifacts, and opaque provider-specific payloads. Current built-in model adapters still produce text; non-text provider output requires an adapter that populates these parts.
- **Markdown v3 + Mew v4:** ordinary Markdown remains the response baseline. Runtime/chat system prompts add optional Mew format guidance; `mew-markdown` turns Markdown blockquotes into typed quote blocks and hidden button comments into inert follow-up suggestions. Runtime results retain both clean Markdown and presentation blocks; the UI chooses how to render them.
- **Subagents:** the runtime exposes `delegate_to_agent` only when a tool-capable model has another registered agent available. Delegation is depth/run/tool-call bounded, children receive read-only permissions by default, and `.mew/agents/<id>.toml` profiles can use a provider/model different from the parent.
- **Workflows and pipelines:** `mew workflow` validates and runs user-authored versioned TOML dependency graphs; `mew pipeline` uses the same format with sequential stage chaining. Independent workflow steps execute in bounded parallel batches. Prompts can use `{{input}}`, `{{input.path}}`, `{{steps.<id>.output}}`, or `{{steps.<id>.response_json}}`. Results retain per-step success/error state and failure-dependent steps are marked skipped.
- **Tools and MCP:** tools publish JSON schemas and risk classes. Read-only tools can run directly; write, execute, network, and external tools require an explicit run capability and an approver unless an embedding application deliberately enables auto-approval. Advertised MCP tools are adapted as `External` tools.
- **Budget and observability:** input history is compacted within per-agent context budgets while preserving tool-call/observation pairs. Model/tool timeouts, output caps, total run/tool-call limits, usage fields, and event hooks bound and expose execution.

## Permission boundary

The workspace root is canonicalized once per command. File paths are relative to that root; parent traversal and symlink escapes are rejected. Ask/review mode exposes read-only filesystem and Git tools. Edit mode exposes only exact-match replacements, which require a visible diff and approval. Package checks and public HTTPS reads are separate opt-ins and require per-action approval unless the user passes `--yes`.

The native MCP server exposes read-only tools. External MCP servers are launched directly as child processes over stdio; mew does not evaluate their commands through a shell. An embedding application that registers an external MCP tool must separately grant the `External` capability and provide an approver (or consciously choose auto-approval).

## Provider and protocol boundary

Provider adapters define model capabilities and execute their native structured-call protocols. OpenAI, OpenRouter, and custom OpenAI-compatible providers use function tools; Gemini uses function declarations/calls/responses on `generateContent`. Built-in Gemini 2.5 models are tool-enabled, including under older default configurations. Other models remain individually capability-configurable. The runtime result envelope is broader than current provider adapters, so it does not claim that built-in models already support every media type.

The Markdown v3/v4 response contract is Mew-owned and documented in [`markdown-v4.md`](markdown-v4.md); it is separate from MCP, whose client uses JSON-RPC over stdio and requests protocol version `2024-11-05` during initialization.
