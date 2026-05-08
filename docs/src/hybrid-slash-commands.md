# Hybrid Slash Commands

The hybrid CLI treats `/` as an interactive command prefix. When the current
input starts with `/` and contains no whitespace, the bottom hint area shows
matching slash commands.

Examples:

- `/` lists the available commands.
- `/re` narrows suggestions to commands such as `/resume` and `/refresh`.
- `/app` narrows suggestions to approval-related commands.

The suggestion list uses the same keyboard model as file references:

- `Up` and `Down` move the selected suggestion.
- `Enter` inserts the selected command plus a trailing space when the typed
  text is not already an exact command.
- `Enter` executes the command when the input already equals a supported command.

Slash suggestions are local to the CLI. Execution still goes through the
existing daemon-backed command handlers.
