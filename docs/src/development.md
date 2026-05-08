# Development

## Commands

Run from the repository root:

```powershell
cargo fmt --all
cargo test --workspace
cargo build --workspace
```

Run the CLI:

```powershell
cargo run -p unio -- status
cargo run -p unio -- exec "hello"
```

Run a direct tool command:

```powershell
cargo run -p unio -- tool read --args path=README.md
```

## Repository Guidelines

- Keep documentation and implementation synchronized.
- Update `CHANGE.md` for meaningful behavior, architecture, workflow, or
  documentation changes.
- Update `README.md` when setup, usage, or user-facing behavior changes.
- Add or update docs under `docs/` when architecture, protocol, security,
  storage, tools, or agent behavior changes.

## Tests

Tests live beside implementation in each crate under `#[cfg(test)]`. Prefer
focused unit coverage near the code that owns the behavior.
