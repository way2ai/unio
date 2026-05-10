# Unio Implementation Plan

## Current State

Unio is a Rust-based intelligent agent runtime with a CLI, local daemon, agent
contracts, model provider abstraction, tools, approvals, skills, storage, and
observability.

Implemented capabilities:

- `unio exec` creates a new workspace session through the daemon by default.
- The root agent can call a model provider configured from environment
  variables or `~/.unio/config.toml` and receive model tool calls.
- `/model` configures or switches the persistent model settings from the CLI.
- `/approval` shows or sets the current interactive approval mode.
- `/pending` lists pending approval requests in the interactive terminal.
- Root slash commands support guided usage for approval resolution and trace
  lookup when arguments are omitted.
- Interactive slash commands include `/resume` to switch to an existing
  workspace session and `/new` to create and switch to a new session.
- The interactive terminal surface clears submitted prompts from the editing
  area immediately while the agent run continues.
- Completed interactive turns now render as a friendly execution block with
  `> user input`, `process`, `result`, and `Worked for` duration lines, plus
  colored status dots for process rows.
- Approval-required states now render a dedicated review card with request
  preview and keyboard selection (`Up/Down` + `Enter`), while keeping numeric
  actions `1/2/3` for approve once, approve with full-trust session switch, or
  deny.
- Daemon turn execution now retries once after a tool failure by appending the
  structured tool failure result back into model context.
- The mock provider supports deterministic local development.
- Built-in tools include file, process, planning, and skill execution paths.
- Tool execution goes through `crates/security` before running.
- Approval queue and approval history exist.
- Skills are discovered from `.unio/skills/<name>/SKILL.md`.
- Trace records cover model, tool, approval, planning, skill, and context
  events.
- The mdBook documentation site is available in English and Simplified Chinese.
- Real-model E2E validation spec and report for ReAct/slash/tools now exist:
  `docs/real-model-e2e-test-spec.md` and
  `docs/real-model-e2e-test-report.md`.

## Documentation Plan

Status: completed for the current documentation refresh.

The active documentation set is:

- `README.md`: repository entry point.
- `docs/README.md`: documentation source guide.
- `docs/get-started.md`: short pointer into the active mdBook source.
- `docs/src/`: English mdBook source.
- `docs/zh/src/`: Simplified Chinese mdBook source.
- `docs/theme/head.hbs`: shared mdBook landing page and language switcher
  customization.
- `docs/documentation-refresh.md`: specification for the current documentation
  refresh.
- `docs/arti-doc-report-spec.md`: specification for the bilingual technical
  review report under `arti_doc/`.
- `arti_doc/`: chapter-level bilingual technical review report covering project
  overview, architecture, functional modules, usage, and technical introduction.

The mdBook page structure is synchronized across languages:

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

## Release Plan

Release artifacts are built from `v*` tags by `.github/workflows/release.yml`.

Current targets:

- Linux x86_64: `x86_64-unknown-linux-gnu`
- Windows x86_64: `x86_64-pc-windows-msvc`
- macOS Apple Silicon: `aarch64-apple-darwin`

The workflow builds `unio` and `unio-daemon`, packages the binaries, uploads
artifacts, and publishes a GitHub Release.

## Near-Term Work

1. Fix ReAct continuation quality issues found in real-model E2E runs
   (tool-only termination and failed recovery in targeted prompts).
2. Fix slash command behavior gaps found in direct invocation
   (`/approval <mode>` switching and `/model` direct flow error handling).
3. Expand and stabilize tests around approval policy, tool execution, storage, and trace
   records.
4. Keep documentation synchronized with command behavior and release targets.
5. Add installation details when the first public release artifact is
   available.
6. Continue improving model provider error reporting and secret handling.

## Verification Checklist

Before marking implementation work complete:

- Run `cargo fmt --all`.
- Run `cargo test --workspace`.
- Build English docs with `mdbook build docs` when mdBook is installed.
- Build Chinese docs with `mdbook build docs/zh` when mdBook is installed.
- Check changed documentation for stale links.
