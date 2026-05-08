# CLI

The `unio` binary is the user-facing entry point. It talks to the daemon and
keeps command behavior scriptable for terminals, editors, and automation.

## Common Commands

```powershell
cargo run -p unio -- status
cargo run -p unio -- exec "summarize this repository"
cargo run -p unio -- resume --limit 10
cargo run -p unio -- trace <trace_id> --run <run_id>
```

## Interactive Entry

Run without a subcommand to open the terminal surface:

```powershell
cargo run -p unio
```

Useful prompt features:

- `/` opens command suggestions.
- `@` searches workspace files.
- `Shift+Enter` or `Ctrl+J` inserts a newline.
- `Ctrl+C` twice exits.

## Direct Tool Commands

```powershell
cargo run -p unio -- tool read --args path=README.md
cargo run -p unio -- tool write --args path=notes.txt,content=hello
```

Write and process tools may require approval depending on the selected policy.
