# Advanced TUI and local workflow features

This guide documents Mew-owned implementations of the requested CLI patterns. Similarity to another product is behavioral only; Mew does not reuse its source, assets, private APIs, or authentication internals.

## Model preferences and keybindings

Inside `mew chat`:

- `/favorite` toggles the active `provider/model` in persistent favorites; `/favorites` lists them. The searchable model picker labels and prioritizes favorites and recently used models.
- `/variant` opens the preset selector; `/variant balanced|precise|creative|deterministic` sets a per-model temperature preset. The presets use temperatures `0.2`, `0.1`, `0.8`, and `0.0`, respectively, in both tool-capable runtime calls and plain provider calls.
- Typing `/` as the first character of an empty draft opens command suggestions immediately; `/` later in a draft remains ordinary message text.
- `/keybindings` lists actions and current bindings. `/keybind set <action> <chord[,chord...]>` accepts aliases such as `ctrl+p,f1`, including `f1` through `f24`, and rejects collisions; `/keybind reset <action>` restores that action's defaults. Preferences are saved with the user config. F1 opens the command palette and F2 completes slash commands by default; the original Ctrl+P and Tab shortcuts remain available.

The command palette and transcript/editor bindings use the same persisted action map; this does not add OpenCode-specific command-template compatibility.

## Auto mode and approval grants

`/mode auto` in chat, or `mew agent --mode auto`, is a distinct explicit mode. It enables local workspace edits and arbitrary local shell commands without per-action prompts. It does **not** enable Mew's built-in network-fetch or external/MCP tools. In chat, use `/mode ask` for read-only inspection or `/mode edit` for approval-gated exact-match edits and supported checks.

Auto mode runs commands as the current operating-system user. It is intended for work you trust and should be treated as local code execution, not as a prompt-only safety setting. Shell commands still have whatever filesystem and network access the OS account provides. Foreground commands are time-limited and output-bounded. The environment is cleared except for basic path/home/temp/locale variables.

For approved tool actions in non-Auto modes, Mew can grant an exact action fingerprint for the current session or workspace when the approval dialog offers those scopes. `/approvals` shows the current session/workspace grant counts; `/approvals clear` revokes them. Workspace files store SHA-256 fingerprints of the exact approval summary rather than the summary text. A changed path, arguments, or preview requires a new approval.

## Background commands

Auto-mode agents can start Mew-managed background commands and query, read bounded output, or cancel them. In chat, `/background` lists managed commands; `/background status <id>`, `/background output <id>`, and `/background cancel <id>` inspect and stop one. Commands run from the active workspace. On Unix, commands use a separate process group so cancellation and Mew shutdown also stop child processes.

The manager is process-local: these commands are not detached services and are not resumed after Mew exits. Output is retained only while that Mew process is running, with a 64 KiB per-command cap.

## Lifecycle hooks

Hooks are disabled unless `.mew/hooks.toml` explicitly sets `enabled = true`. Create a disabled starter file with `mew hooks init`, inspect it with `mew hooks status`, and parse/validate it with `mew hooks validate`.

```toml
enabled = true
timeout_seconds = 10

[[hooks]]
event = "pre_tool_use"
command = ["python3", "scripts/mew-pre-tool.py"]
blocking = true
```

Supported events are `session_start`, `user_prompt_submit`, `pre_tool_use`, `post_tool_use`, and `session_end`. Each command is executed directly (no shell string evaluation) from the workspace, with JSON event data on stdin, bounded stdout/stderr, a timeout, and a cleared environment except basic execution variables. A blocking pre-tool hook can deny an operation by exiting unsuccessfully or printing one JSON object such as `{"decision":"deny","reason":"policy reason"}`. Post-tool hooks run after the action and cannot roll it back. Slash-command controls are not passed through the user-prompt hook.

Only add hook commands you trust: they are local programs in your project and run with your user permissions.

## Git worktrees

Use `mew worktree list`, `mew worktree create <branch> [path]`, and `mew worktree remove <path>`. Creation creates a branch when it does not exist or attaches an existing branch, and rejects paths outside the repository's parent. Removal is limited to registered non-primary worktrees with no modified, untracked, or ignored files, and asks for confirmation. Chat exposes the same operations through `/worktree list`, `/worktree create <branch> [path]`, and `/worktree remove <path>`.

## Local conversation sharing

`/share` writes a self-contained offline HTML transcript under `.mew/shares/`; `/unshare` removes that generated local file. It does not publish or upload conversation history to a public share service.

## Google Gemini API OAuth

Mew supports Google’s documented **Gemini API OAuth** for the built-in Gemini provider. This is separate from Google Antigravity (“Agy”): Google publishes no third-party Antigravity OAuth/token contract used by Mew, so Mew does not impersonate its private CLI login flow.

Setup requires a Google Cloud project with the Generative Language API enabled, a configured OAuth consent screen/test user, and a **Desktop app** OAuth client. Set these environment variables in the shell that launches Mew:

```bash
export MEW_GEMINI_OAUTH_CLIENT_ID="<desktop-client-id>"
export MEW_GEMINI_OAUTH_CLIENT_SECRET="<desktop-client-secret>"
export MEW_GEMINI_OAUTH_PROJECT_ID="<google-cloud-project-id>"
mew provider login gemini
```

`mew provider login gemini` asks the system URL handler to open the Google authorization URL in the default browser, and prints the URL as a fallback either way. Mew listens on a random `127.0.0.1` loopback port for five minutes. The flow uses state validation, PKCE S256, offline access, and Google's documented `cloud-platform` and `generative-language.retriever` scopes. If Mew runs over SSH, forward the exact printed callback port to the remote loopback before completing consent. Then verify with `mew provider test gemini` or `mew model list gemini`.

The first-run `mew setup` wizard currently configures OpenRouter or OpenAI API keys; it does not start Gemini OAuth. Run the provider-login command above separately after configuring the required Google OAuth environment variables.

Refresh/access tokens are stored locally at `$XDG_CONFIG_HOME/mew/gemini-oauth.json` or `~/.config/mew/gemini-oauth.json` (Unix, mode `0600`), and `%APPDATA%\\mew\\gemini-oauth.json` (Windows). `mew provider logout gemini` deletes the local token file. An existing Gemini API-key environment variable continues to take precedence over OAuth.

Official flow references: [Gemini API OAuth quickstart](https://ai.google.dev/gemini-api/docs/oauth) and [Google OAuth for installed apps](https://developers.google.com/identity/protocols/oauth2/native-app).
