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

## Model Configuration

The `/model` slash command shows and updates the persistent model settings in
`~/.unio/config.toml`.

```powershell
cargo run -p unio -- "/model"
```

`/model` shows the active provider, then prompts for provider, model, base URL,
and API key in either the terminal surface or a direct slash invocation. The
flow updates `~/.unio/config.toml`; environment variables still override the
file.

## Run Rendering

Interactive and direct command output renders model and tool activity as readable
run summaries. Tool events are grouped by purpose instead of exposing raw event
names:

- `Running`: model execution and stage changes.
- `Tool`: completed tool calls with the tool name and concise target.
- `Approval`: tool calls waiting for approval or approval decisions.
- `Skill`: skill-tool activity with the skill name when available.
- `Done`: final stage, model, token usage, and context ratio.

Raw trace event names remain available through `/trace <trace_id>` for debugging.

## Direct Tool Commands

```powershell
cargo run -p unio -- tool read --args path=README.md
cargo run -p unio -- tool write --args path=notes.txt,content=hello
```

Write and process tools may require approval depending on the selected policy.
