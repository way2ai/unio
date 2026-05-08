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
