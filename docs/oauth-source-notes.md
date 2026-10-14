# Google OAuth integration source notes

Reviewed 2026-10-14 while implementing Mew's provider authentication.

- Google Antigravity CLI's [installation and authentication guide](https://antigravity.google/docs/cli/install/) describes local sign-in through Antigravity's own CLI and OS keyring, plus an SSH URL/code handoff. It does not document a third-party provider API, reusable client ID, public token exchange contract, or an external-app token export API. Mew therefore must not imitate or scrape Antigravity's private login/token internals.
- Google's [Gemini API OAuth quickstart](https://ai.google.dev/gemini-api/docs/oauth) documents a supported desktop OAuth client flow. It requires a Google Cloud project, an OAuth consent screen, and a desktop OAuth client owned/configured by the user. The example uses OAuth scopes including `cloud-platform` and `generative-language.retriever`, refresh-token caching, and the `x-goog-user-project` header for REST access.

Implementation direction: make OAuth optional alongside existing Gemini API-key auth; require user-configured desktop client ID/secret and project ID; use local loopback redirect + state/PKCE, refresh tokens, and private credential storage. Do not imply that this authenticates Mew to Antigravity itself.
