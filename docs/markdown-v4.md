# Mew Markdown v4 response layer

Mew uses ordinary Markdown as its response baseline (the project's Markdown v3 layer). Markdown remains the canonical body and must remain useful in renderers that do not know Mew's extension. `mew-markdown` adds a small, optional v4 interpretation layer above that body; it is a Mew-owned response convention, not a claim of compatibility with an external Markdown-v4 standard.

## What the layer does

- **Markdown blockquotes** remain ordinary Markdown and are also exposed as typed `Quote` blocks so a capable frontend can render them as a distinct quote/callout form.
- **Suggestion buttons** are encoded as one standalone, HTML-comment annotation. HTML comments are hidden by ordinary Markdown renderers; Mew parses valid annotations into typed data and removes the protocol line from the clean Markdown body.
- All other content stays Markdown. Code fences are never parsed as protocol. Invalid or over-limit annotations are retained as ordinary text instead of being silently dropped.

## Button syntax

```markdown
<!-- mew:v4:button {"label":"Show an example","prompt":"Give a concrete example of the previous explanation."} -->
```

The JSON object must contain exactly two strings:

- `label`: non-empty, single-line, at most 80 UTF-8 bytes.
- `prompt`: non-empty, single-line, at most 1,000 UTF-8 bytes.

A response may contain up to eight buttons. Control characters are rejected. The prompt is a suggested user follow-up, **not** a tool invocation, URL, command, permission grant, or autonomous action. A frontend that makes the suggestion interactive may submit its prompt as a new user message after the user selects it. The runtime itself does not execute buttons. The CLI currently renders suggestions as labeled, inert UI blocks.

## Model guidance and API

The shared `MODEL_INSTRUCTIONS` text is appended to runtime agent system prompts and new/resumed CLI chat system prompts. It explicitly keeps ordinary Markdown as the base and makes buttons optional.

`parse_response` returns a `ResponseDocument` containing:

- `markdown`: the clean baseline body with recognized button annotations removed.
- `blocks`: ordered `Markdown`, `Quote`, and `Button` presentation blocks.

Runtime `AgentResponse` preserves both the text content and parsed presentation. Consumers may use `as_text()` for a Markdown-only fallback, or inspect `presentation` to choose a richer display. The parser and typed data model live in `mew-markdown`, separate from terminal rendering, so other frontends can reuse the same contract.

## Example

```markdown
## Result
The cache avoids repeated work.

> Cache only data whose freshness requirements you understand.

<!-- mew:v4:button {"label":"Show a code example","prompt":"Show a small code example of this cache pattern."} -->
```

A baseline client sees the heading, explanation, and quote. A Mew-aware client also gets a typed quote block and an optional follow-up suggestion; it does not see the recognized annotation in the clean Markdown body.
