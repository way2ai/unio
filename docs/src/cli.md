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

Interactive slash commands include `/skills`, `/model`, `/approval`, `/pending`,
`/approve`, `/deny`, `/resume`, `/new`, `/trace`, `/refresh`, `/update`, and `/quit`.
`/approval` shows or updates the current approval mode for the workspace session.
`/pending` shows pending approval requests.

Session behavior:

- Each `unio` workspace entry creates a new session by default.
- `/resume` opens a workspace session picker in TUI mode (`Up/Down` select,
  `Enter` switch, `Esc` cancel).
- `/new` creates a new session immediately and switches the TUI to it.

Slash commands support root entry without required arguments. When context is
missing, Unio guides the next step instead of failing immediately.

When a normal prompt is submitted, the terminal surface immediately clears the
editing area and moves the prompt into the message stream before waiting for the
agent response. The response is appended when the daemon finishes the run.

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

Interactive output renders each submitted prompt as a friendly execution block
instead of raw event logs. The block follows this shape:

- `> <user input>` source line
- `process` section with short event lines from trace activity
- `result` section with the assistant final response
- `Worked for <duration>` summary computed from trace timestamps

The TUI renders a colored status dot before each process step. Dot
color reflects status (for example: running, completed, approval-required, or
failed).
While a run is in progress, the render area updates dynamically with a live
`Considering... (Ns)` status line until the final response arrives.
When a run pauses for approval, the UI keeps the tool approval step and omits
the `result` block to avoid showing internal waiting text.
Approval prompts render as a dedicated review card with action/target preview
and keyboard selection: `Up/Down` to choose, `Enter` to confirm.
Numeric shortcuts remain available: `1` approve once, `2` approve and switch
session to full-trust, `3` deny.
File references prefixed with `@` are resolved against the workspace before
submission. Common typos are auto-normalized (for example `@.cargo-lock` to
`@Cargo.lock`), and unresolved references are surfaced in the UI.

Tool events are still grouped by purpose instead of exposing raw event names:

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
