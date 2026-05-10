# 04 Usage Guide

## Requirements

Running from source requires the Rust toolchain. The documentation site
optionally requires `mdbook`.

```powershell
cargo fmt --all
cargo test --workspace
cargo build --workspace
```

## Check Status

```powershell
cargo run -p unio -- status
```

This shows daemon status, workspace, session count, pending approval count,
latest trace, context ratio, and current model provider.

## Execute A Prompt

```powershell
cargo run -p unio -- exec "hello unio"
cargo run -p unio -- exec "summarize this repository"
```

If no real model is configured, Unio uses the mock provider. Mock output is
useful for validating the local runtime flow.

## Open The Interactive UI

```powershell
cargo run -p unio
```

Useful interactions:

- Type `/` for slash command suggestions.
- Type `@` for workspace file references.
- Use `Shift+Enter` or `Ctrl+J` for a newline.
- Press `Ctrl+C` twice to exit.
- Use `/model` for model configuration.
- Use `/approval` or `/approvals` for pending approvals.
- Use `/trace <trace_id> [run_id]` for traces.
- Use `/resume [limit]` for recent transcript messages.

## Configure A Model

Interactive configuration:

```powershell
cargo run -p unio -- "/model"
```

The configuration is written to:

```text
~/.unio/config.toml
```

Example:

```toml
[model]
provider = "openai-compatible"
model = "gpt-4o-mini"
base_url = "https://api.openai.com/v1"
api_key = "sk-..."
```

Environment variables have higher precedence:

```powershell
$env:UNIO_MODEL_PROVIDER="openai-compatible"
$env:OPENAI_MODEL="gpt-4o-mini"
$env:OPENAI_API_KEY="sk-..."
```

For Anthropic-style providers:

```powershell
$env:UNIO_MODEL_PROVIDER="anthropic"
$env:ANTHROPIC_MODEL="claude-3-5-sonnet-latest"
$env:ANTHROPIC_API_KEY="..."
```

## Execute Tools Directly

Read a file:

```powershell
cargo run -p unio -- tool read --args path=README.md
```

Write a file:

```powershell
cargo run -p unio -- tool write --args path=notes.txt,content=hello
```

In default mode, writes wait for approval. To intentionally allow a request:

```powershell
cargo run -p unio -- tool write --approval full-trust --args path=notes.txt,content=hello
```

## Approve Tool Calls

List pending approvals:

```powershell
cargo run -p unio -- approvals
```

Approve:

```powershell
cargo run -p unio -- approvals approve approval_xxx
```

Deny:

```powershell
cargo run -p unio -- approvals deny approval_xxx
```

View approval history:

```powershell
cargo run -p unio -- approvals history
```

## Use Skills

Create a workspace skill:

```powershell
New-Item -ItemType Directory -Force .unio\skills\repo-helper
Set-Content .unio\skills\repo-helper\SKILL.md "# Repo helper`nUse for repository analysis."
```

List skills:

```powershell
cargo run -p unio -- skills
```

Invoke a skill tool:

```powershell
cargo run -p unio -- tool skill-tool --approval full-trust --args name=repo-helper,request=inspect-modules
```

## Inspect Sessions And Traces

List sessions:

```powershell
cargo run -p unio -- sessions
```

Resume recent records:

```powershell
cargo run -p unio -- resume --limit 10
```

Query traces:

```powershell
cargo run -p unio -- trace trace_xxx
cargo run -p unio -- trace trace_xxx --run run_xxx
```

## Run The Documentation Site

```powershell
cargo install mdbook
mdbook serve docs
mdbook serve docs/zh
```

## Development Verification

Before finishing code or documentation changes, run:

```powershell
cargo fmt --all
cargo test --workspace
cargo build --workspace
```

For documentation changes, if mdBook is installed:

```powershell
mdbook build docs
mdbook build docs/zh
```
