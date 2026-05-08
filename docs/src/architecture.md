# Architecture

Unio is a local daemon-orchestrated agent system. Users interact through the
`unio` hybrid CLI or scriptable subcommands. The daemon owns runtime state,
session/run lifecycle, approvals, tools, storage, and trace persistence.

## Workspace Layout

```text
apps/
  cli/       user-facing unio command and hybrid terminal surface
  daemon/    local runtime owner for sessions, runs, approvals, tools, trace

crates/
  protocol/       CLI/daemon/agent protocol types
  core/           IDs, paths, metadata, shared utilities
  agent/          root agent, planner, sub-agent, skill-agent contracts
  model/          OpenAI-compatible, Anthropic, and mock providers
  tools/          tool registry and execution contract
  security/       approval policy and risk precheck
  skills/         skill discovery and skill-tool execution
  observability/  trace and context events
  storage/        SQLite metadata and JSONL transcript/trace stores
```

## Main Flow

```text
user
  -> hybrid CLI
  -> daemon
  -> root agent
  -> planner? -> sub-agent/tool?
  -> daemon
  -> hybrid CLI
  -> user
```

## ID Model

- `session_id`: long-lived workspace conversation.
- `conversation_id`: one user request chain.
- `run_id`: one agent execution.
- `agent_id`: root, planner, sub-agent, or skill-agent instance.
- `trace_id`: observability correlation id.

## Current Contracts

- `apps/cli` owns user interaction only.
- `apps/daemon` owns runtime orchestration and persistence.
- `crates/security` decides allow, deny, or approval-required outcomes.
- `crates/tools` executes tools only after security precheck.
- `crates/storage` persists SQLite records and message-level JSONL transcripts.
- `crates/observability` records trace and context events.

See also:

- [Hybrid File References](hybrid-file-references.md)
- [Hybrid Slash Commands](hybrid-slash-commands.md)
- [Hybrid Input Editing](hybrid-input-editing.md)
