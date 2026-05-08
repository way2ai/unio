# Documentation Refresh Specification

Date: 2026-05-08

## Goal

Refresh Unio documentation so it presents the project as a product and
developer tool, not as an implementation-history record. The GitHub Pages site should use
mdBook as the publishing engine while making the first page feel closer to a
product page: clear positioning, install commands, capability sections, and
developer entry points.

## Scope

- Remove documents whose main purpose is recording earlier implementation
  history, including decision records, future-architecture notes, current-state
  notes, and narrow terminal-surface implementation pages.
- Rewrite the English and Simplified Chinese mdBook sources with a synchronized
  table of contents.
- Keep architecture documentation, but describe the current runtime and crate
  boundaries directly.
- Keep operational documentation for local mdBook preview and GitHub Pages.
- Update root-facing documents so `README.md`, `CHANGE.md`, and `PLAN.md`
  match the new documentation direction.
- Remove the macOS x86_64 build target from the release workflow.

## mdBook Structure

The English site under `docs/src/` and the Chinese site under `docs/zh/src/`
will use these pages:

- Overview
- Install
- Quickstart
- CLI
- Tools And Approvals
- Skills
- Architecture
- Development
- Release
- Site Operations

The old implementation-history pages will not be linked from `SUMMARY.md` and
will be removed from version control.

## Product Page Direction

The mdBook overview page will include HTML sections for a stronger GitHub Pages
landing experience:

- A hero section with the `Unio` name, concise positioning, and install/run
  commands.
- A capability grid covering local orchestration, tools, approvals, skills,
  storage, and observability.
- A workflow section showing how users move from prompt to tool execution,
  approval, trace, and resume.
- Calls to action linking to install, quickstart, architecture, and releases.

The custom theme file will add CSS for those sections while preserving mdBook's
normal reading experience and language switcher.

## Verification

- Confirm `release.yml` only lists Linux x86_64, Windows x86_64, and macOS
  Apple Silicon targets.
- Confirm documentation searches no longer contain stale historical page links.
- Run `cargo fmt --all`.
- Run `cargo test --workspace`.
- Run `mdbook build docs` and `mdbook build docs/zh` if `mdbook` is available in
  the environment.
