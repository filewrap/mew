# mew

> a tiny coding cat with sharp claws.

```bash
mew
mew init
mew ask "what does this repo do?"
mew edit "fix this bug"
mew name set paww
mew style preview
```

## What is mew?

`mew` is an open-source Rust AI coding agent built for CLI-first workflows, low-resource users, Termux, Linux, macOS, Windows, and custom AI providers.

It is designed to be cute, fast, safe, token-efficient, and powerful.

## Current Phase

See [`docs/PHASES.md`](docs/PHASES.md).

## Development

```bash
cargo build
cargo run -p mew-cli -- --help
cargo run -p mew-cli
cargo run -p mew-cli -- doctor
cargo run -p mew-cli -- init --dry-run
cargo run -p mew-cli -- style preview
cargo run -p mew-cli -- name set paww
cargo run -p mew-cli -- name show
```

## License

Apache-2.0


## Install

### Termux / Linux smart installer

```bash
curl -fsSL https://raw.githubusercontent.com/mahesh953-hub/mew/main/scripts/install.sh | bash
```

Custom repo:

```bash
MEW_REPO_URL=https://github.com/mahesh953-hub/mew bash scripts/install.sh
```

### Cargo

```bash
cargo install --git https://github.com/mahesh953-hub/mew --package mew-cli --force
```



## Providers

Key-based auth for now.

```bash
export OPENAI_API_KEY="..."
export OPENROUTER_API_KEY="..."
export GEMINI_API_KEY="..."
```

```bash
mew provider list
mew provider test openai
mew provider test openrouter
mew provider test gemini
```

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

## Ask / Chat

```bash
mew ask "hello"
mew chat
```

Inside chat:

```txt
/
 /model
 /providers
 /models
 /sessions
 /clear
 /exit
```

## Sessions

```bash
mew session list
mew session show <id>
```

## Install

### Termux / Linux smart installer

```bash
curl -fsSL https://raw.githubusercontent.com/mahesh953-hub/mew/main/scripts/install.sh | bash
```

### Cargo

```bash
cargo install --git https://github.com/mahesh953-hub/mew --package mew-cli --force
```
