# Get Started

This guide shows how to build and use the current Unio refactor.

## Requirements

- Rust stable toolchain
- Cargo
- Windows, macOS, or Linux shell

```powershell
rustc --version
cargo --version
```

## Build And Test

Run from the repository root:

```powershell
cargo fmt --all
cargo test --workspace
cargo build --workspace
```

## Start Unio

Launch the hybrid CLI/TUI:

```powershell
cargo run -p unio
```

The bare `unio` entry opens the interactive surface. Scriptable commands remain
available:

```powershell
cargo run -p unio -- status
cargo run -p unio -- exec "hello unio"
cargo run -p unio -- resume --limit 10
cargo run -p unio -- trace <trace_id> --run <run_id>
```

If model credentials are not configured, Unio falls back to the mock provider.

## Hybrid Input

Useful keys in the hybrid surface:

- `/`: show slash command suggestions.
- `@`: search workspace files.
- `Up` / `Down`: select suggestions or scroll history.
- `Left` / `Right`: move inside the input line.
- `Home` / `End`, `Ctrl+A` / `Ctrl+E`: jump to start or end.
- `Ctrl+W`: delete previous word.
- `Ctrl+U`: clear the prompt.
- `Shift+Enter` or `Ctrl+J`: insert a newline.
- `Ctrl+C` twice: exit.

Examples:

```text
/re
inspect @README.md
plan a refactor
```

## Tools And Approvals

Run tools directly:

```powershell
cargo run -p unio -- tool read --args path=README.md
cargo run -p unio -- tool write --args path=notes.txt,content=hello
```

In `default` mode, writes and process execution require approval:

```powershell
cargo run -p unio -- approvals
cargo run -p unio -- approvals approve approval_xxx
cargo run -p unio -- approvals deny approval_xxx
```

Use `full-trust` only when intentionally allowing execution:

```powershell
cargo run -p unio -- tool write --approval full-trust --args path=notes.txt,content=hello
```

## Skills

Unio discovers skills from:

```text
{workspace}/.unio/skills/
~/.unio/skills/
```

Create and list a workspace skill:

```powershell
New-Item -ItemType Directory -Force .unio\skills\repo-helper
Set-Content .unio\skills\repo-helper\SKILL.md "# Repo helper`nUse for repository analysis."
cargo run -p unio -- skills
```

Invoke a skill through the tool contract:

```powershell
cargo run -p unio -- tool skill-tool --approval full-trust --args name=repo-helper,request=inspect-modules
```

## Mock Development Paths

Use mock directives when developing without model credentials:

```powershell
cargo run -p unio -- exec "mock-tool read path=README.md"
cargo run -p unio -- exec "mock-tool skill-tool name=repo-helper,request=inspect-modules"
cargo run -p unio -- exec "mock-usage input=90000,output=1000"
```

For design details, read [Architecture](architecture.md),
[Hybrid Input Editing](hybrid-input-editing.md), and
[Target Architecture](target-architecture.md).
