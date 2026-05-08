# Unio Implementation Plan

## Current State

Unio is a Rust-based intelligent agent runtime with a CLI, local daemon, agent
contracts, model provider abstraction, tools, approvals, skills, storage, and
observability.

Implemented capabilities:

- `unio exec` creates or resumes a workspace session through the daemon.
- The root agent can call a model provider configured from environment
  variables or `~/.unio/config.toml` and receive model tool calls.
- `/model` configures or switches the persistent model settings from the CLI.
- The mock provider supports deterministic local development.
- Built-in tools include file, process, planning, and skill execution paths.
- Tool execution goes through `crates/security` before running.
- Approval queue and approval history exist.
- Skills are discovered from `.unio/skills/<name>/SKILL.md`.
- Trace records cover model, tool, approval, planning, skill, and context
  events.
- The mdBook documentation site is available in English and Simplified Chinese.

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

1. Keep CLI and daemon contracts aligned as commands are added.
2. Expand tests around approval policy, tool execution, storage, and trace
   records.
3. Keep documentation synchronized with command behavior and release targets.
4. Add installation details when the first public release artifact is
   available.
5. Continue improving model provider error reporting and secret handling.

## Verification Checklist

Before marking implementation work complete:

- Run `cargo fmt --all`.
- Run `cargo test --workspace`.
- Build English docs with `mdbook build docs` when mdBook is installed.
- Build Chinese docs with `mdbook build docs/zh` when mdBook is installed.
- Check changed documentation for stale links.
