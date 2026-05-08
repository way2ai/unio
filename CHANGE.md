## Unreleased

- Rewrote the documentation set around product and developer workflows instead
  of historical implementation notes. The mdBook now contains Overview,
  Install, Quickstart, CLI, Tools And Approvals, Skills, Architecture,
  Development, Release, and Site Operations pages.
- Rebuilt the GitHub Pages landing experience with mdBook theme customizations:
  hero section, command examples, capability cards, workflow section, and
  English/Chinese language switcher.
- Added synchronized Simplified Chinese mdBook pages under `docs/zh/src/` and
  removed the unused gettext PO translation file.
- Removed historical implementation documentation, including decision records,
  future-architecture notes, current-state notes, and narrow terminal-surface
  implementation pages.
- Updated `README.md`, `docs/README.md`, and `docs/get-started.md` to point to
  the new documentation structure.
- Updated `.github/workflows/release.yml` to stop building the macOS x86_64
  release artifact. Release targets are now Linux x86_64, Windows x86_64, and
  macOS Apple Silicon.
