# Hybrid Input Editing

The hybrid CLI owns a small local input buffer instead of treating the prompt as
append-only text. The buffer tracks both text and cursor position.

Supported editing keys:

- `Left` / `Right`: move the cursor by one character.
- `Home` / `Ctrl+A`: move to the start of the prompt.
- `End` / `Ctrl+E`: move to the end of the prompt.
- `Backspace`: delete the character before the cursor.
- `Delete`: delete the character under the cursor.
- `Ctrl+W`: delete the previous word.
- `Ctrl+U`: clear the whole prompt.
- `Shift+Enter` or `Ctrl+J`: insert a newline.
- `Enter`: submit the prompt unless a completion candidate is selected.

Completion behavior uses the text before the cursor. `@` file references and `/`
slash commands can be completed while editing in the middle of a prompt. A
completion replaces the active token before the cursor and keeps the rest of the
prompt intact.

The rendered input line shows the cursor as a styled `|` marker. The marker is a
UI artifact only and is never sent to the daemon.
