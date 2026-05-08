# Quickstart

Run all commands from the repository root.

## Verify The Workspace

```powershell
cargo fmt --all
cargo test --workspace
cargo build --workspace
```

## Check Status

```powershell
cargo run -p unio -- status
```

## Run A Prompt

```powershell
cargo run -p unio -- exec "hello unio"
```

If model credentials are not configured, Unio uses the mock provider so local
development remains deterministic.

## Configure A Real Model

Unio reads persistent model settings from `~/.unio/config.toml`. Environment
variables override the file for temporary changes.

Use the slash command to create or update the file:

```powershell
cargo run -p unio -- "/model"
```

```toml
[model]
provider = "openai-compatible"
model = "gpt-4o-mini"
base_url = "https://api.openai.com/v1"
api_key = "sk-..."
```

For Anthropic:

```toml
[model]
provider = "anthropic"
model = "claude-3-5-sonnet-latest"
api_key = "sk-ant-..."
```

Supported providers are `openai`, `openai-compatible`, `anthropic`, and `mock`.
If a real provider is selected without an API key, Unio reports the requested
provider but falls back to the mock provider.

## Read A File Through The Tool Layer

```powershell
cargo run -p unio -- tool read --args path=README.md
```

## Inspect Recent Work

```powershell
cargo run -p unio -- resume --limit 10
cargo run -p unio -- trace <trace_id> --run <run_id>
```

## Next Steps

- Use [CLI](cli.md) for the command reference.
- Use [Tools And Approvals](tools-and-approvals.md) before enabling write or
  process execution.
- Use [Skills](skills.md) to add repository-specific workflows.
