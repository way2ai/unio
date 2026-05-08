# Hybrid File References

The hybrid CLI supports lightweight file references in prompts with `@path`.
References trigger when `@` appears at the start of the input or immediately
after whitespace, so both `@README.md` and `inspect @src/main.rs` are valid.

File suggestions are served from an in-memory workspace index maintained by a
background thread. The indexer avoids blocking keystrokes and prunes noisy paths
while walking the tree:

- honor simple `.gitignore` and `.npmignore` entries while descending;
- skip hidden directories such as `.git`;
- skip heavy generated directories such as `node_modules`, `target`, `dist`,
  `build`, `.next`, `.cache`, and `coverage`;
- expose only relative workspace file paths to the input suggestion list.

Suggestion ranking favors the user's likely target: exact basename matches
first, then exact relative-path matches, then basename prefix matches, path
prefix matches, shorter paths, and finally lexical order. For example,
`@unio.exe` should list `unio.exe` before `deps/unio.exe`.

Interactive filtering must stay in-memory and allocation-light. The UI must not
recursively scan the filesystem or sort the full match set on each keypress.
The index stores pre-normalized lowercase paths and basenames, and suggestion
queries maintain only the best 50 candidates. Background refreshes are slow
polls for now, not per-frame scans.

The index is intentionally lightweight. It is an interactive hint source, not a
security boundary and not a replacement for tool-level path validation.
