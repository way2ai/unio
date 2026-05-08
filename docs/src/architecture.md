# Architecture

Unio is a Rust workspace organized around a local daemon. The CLI is thin: it
collects user input and sends work to the daemon. The daemon owns state,
session and run lifecycle, approvals, tool execution, storage, and trace events.

## Workspace Layout

```text
apps/
  cli/       user-facing unio command
  daemon/    local runtime for sessions, runs, tools, storage, and trace

crates/
  core/           IDs, paths, metadata, shared utilities
  protocol/       CLI, daemon, and agent protocol types
  agent/          root agent, planner, sub-agent, and skill-agent contracts
  model/          model provider abstraction and mock provider
  tools/          tool registry and execution contract
  security/       approval policy and risk precheck
  skills/         skill discovery and skill-tool execution
  storage/        SQLite metadata and JSONL transcript stores
  observability/  trace and context events
```

## Runtime Flow

```text
user
  -> unio CLI
  -> daemon
  -> agent
  -> model provider or tool
  -> security approval when needed
  -> storage and trace records
  -> user-visible result
```

## Current Contracts

- `apps/cli` owns user interaction and command parsing.
- `apps/daemon` owns runtime orchestration and persistence.
- `crates/protocol` defines shared request and response types.
- `crates/security` returns allow, deny, or approval-required decisions.
- `crates/tools` executes registered tools only after precheck.
- `crates/storage` persists metadata, transcripts, and trace data.
- `crates/observability` records structured runtime events.

## Design Principles

- Keep model calls, tool execution, and persistence out of the CLI.
- Route every risky operation through the security policy.
- Make local development possible with the mock provider.
- Prefer explicit IDs for sessions, runs, agents, and traces.
