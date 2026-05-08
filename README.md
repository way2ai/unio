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
