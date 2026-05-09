# Unio

Unio is a Rust-based intelligent agent runtime for local developer workflows.
It combines a CLI, daemon, model abstraction, tools, approvals, skills, storage,
and observability into one workspace.

## Start

```powershell
cargo run -p unio -- status
cargo run -p unio -- exec "hello unio"
cargo run -p unio -- tool read --args path=README.md
```

If model credentials are not configured, Unio uses the mock provider for local
development.

Configure a real provider interactively:

```powershell
cargo run -p unio -- "/model"
```

In the interactive terminal surface, submitted prompts clear from the editing
area immediately and appear in the message stream while the agent runs.
Each completed turn is rendered as a friendly block with `> user input`,
`process`, `result`, and `Worked for` duration lines, and colored status dots
for process steps.
Root slash commands such as `/approval`, `/approve`, `/deny`, and `/trace`
support guided usage without required arguments.
Session handling now defaults to creating a new session each time you start
`unio` in a workspace. Use `/resume` in TUI mode to pick and switch to an
existing workspace session (`Up/Down`, `Enter`, `Esc`), or `/new` to create and
switch to a fresh one immediately.
When approvals are required, the TUI shows an inline review card with preview
and keyboard selection (`Up/Down` select, `Enter` confirm). Numeric actions
(`1` yes, `2` yes and allow session edits, `3` no) remain available.
If a model-requested tool call fails, Unio feeds that failure result back to
the model and allows one automatic retry in the same turn.

The slash command shows the active provider, then prompts for model settings and
updates `~/.unio/config.toml`:

```toml
[model]
provider = "openai-compatible"
model = "gpt-4o-mini"
api_key = "sk-..."
```

Environment variables such as `UNIO_MODEL_PROVIDER`, `OPENAI_MODEL`, and
`OPENAI_API_KEY` override the file.

## Develop

```powershell
cargo fmt --all
cargo test --workspace
cargo build --workspace
```

## Documentation

The project site is an mdBook published to GitHub Pages.

```powershell
cargo install mdbook
mdbook serve docs
mdbook serve docs/zh
```

Read the docs:

- [Overview](./docs/src/overview.md)
- [Install](./docs/src/install.md)
- [Quickstart](./docs/src/quickstart.md)
- [CLI](./docs/src/cli.md)
- [Architecture](./docs/src/architecture.md)
- [Release](./docs/src/release.md)
