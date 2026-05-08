# Repository Guidelines

## Project Overview

Unio is a general-purpose intelligent agent. It can write and modify code, process documents, use tools, manage tasks, and connect to the external world through approved integrations.

The repository is a Rust workspace with CLI, daemon, protocol, agent, model, tools, security, skills, storage, and observability components.

## Core Workflow

Follow a docs-first workflow:

1. Before implementation, update or create the relevant document under `docs/`.
2. Treat documentation as the spec for the change.
3. Implement only after the spec is clear.
4. After completing the task, update related project documents:
   - Update `PLAN.md` when plans, milestones, progress, or task status change.
   - Update `README.md` when user-facing behavior, commands, setup, or usage changes.
   - Update `CHANGE.md` when the project has any meaningful code, behavior, architecture, tool, workflow, or documentation change.
   - Update docs under `docs/` when architecture, design, protocol, security, storage, tools, or agent behavior changes.

Documentation and implementation must stay synchronized.

## Project Structure

- `apps/cli`: user-facing `unio` command.
- `apps/daemon`: local daemon runtime for sessions, runs, approvals, tools, trace, and storage.
- `crates/core`: shared IDs, paths, metadata, and utilities.
- `crates/protocol`: CLI/daemon protocol types.
- `crates/agent`: root agent, planner, sub-agent, and skill-agent contracts.
- `crates/model`: model provider abstraction.
- `crates/tools`: tool registry and execution.
- `crates/security`: permission and approval policy.
- `crates/skills`: skill discovery and execution.
- `crates/storage`: persistence.
- `crates/observability`: trace and context events.
- `docs/`: specs, architecture, decisions, current state, and guides.

Tests live beside implementation in each crate under `#[cfg(test)]`.

## Development Commands

Run from the repository root:

```powershell
cargo fmt --all
cargo test --workspace
cargo build --workspace
cargo run -p unio -- status
cargo run -p unio -- exec "hello"
cargo run -p unio -- tool read --args path=README.md
```
