## Unreleased

- Added mdBook site source under `docs/src/` with `docs/book.toml` and
  `docs/src/SUMMARY.md`. Existing `docs/` content is migrated into
  `docs/src/` pages for Overview, Get Started, Architecture, hybrid surface
  sub-pages, Target Architecture, Download, Site Operations, and Decisions.
- Added `.github/workflows/deploy-site.yml` to build and publish the mdBook
  site to GitHub Pages on every push to the default branch and on
  `workflow_dispatch`. The workflow runs `cargo test --workspace` before
  building the site.
- Updated `README.md` with local preview and site deployment instructions.

- Optimized hybrid `@` file references: line-start `@path` now triggers
  suggestions, and suggestions come from a background workspace file index
  instead of synchronous per-keystroke directory traversal.
- The file reference index prunes simple `.gitignore` and `.npmignore` rules,
  hidden directories, and heavy generated folders such as `node_modules`,
  `target`, `dist`, and `build`.
- File reference suggestions now precompute lowercase path metadata, use
  non-blocking reads, keep only the top 50 ranked candidates, and avoid sorting
  the full match set on each keypress.
- Hybrid slash commands now support prefix-matched suggestions, keyboard
  selection, and Enter completion.
- Hybrid input editing now supports cursor movement, Home/End, Ctrl+A/Ctrl+E,
  Backspace/Delete, Ctrl+W, Ctrl+U, and multiline prompts with Shift+Enter or
  Ctrl+J.
