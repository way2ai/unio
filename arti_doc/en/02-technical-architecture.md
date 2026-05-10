# 02 Technical Architecture

## Overall Architecture

Unio uses a local daemon-centered architecture. The CLI is comparatively thin:
it parses commands, renders the terminal interface, reads the daemon instance
file, and calls the daemon HTTP API. The daemon owns sessions, runs, approvals,
tool execution, storage, and traces.

```text
user
  -> unio CLI / TUI
  -> local daemon HTTP API
  -> root agent
  -> model provider or tool registry
  -> security approval policy
  -> SQLite / JSONL storage
  -> trace and user-visible output
```

This split keeps model calls, tool execution, and persistence out of the CLI and
leaves room for editor integrations, background processes, or other automation
entry points.

## Workspace Layers

```text
apps/
  cli       command parsing, TUI, daemon client, output rendering
  daemon    local HTTP runtime, orchestration, persistence

crates/
  core           IDs, paths, daemon instance metadata
  protocol       shared request/response and transcript schemas
  agent          root agent, planner, sub-agent and skill-agent contracts
  model          provider configuration and model API adapters
  tools          built-in tool registry and execution implementations
  security       permission mode and approval decision matrix
  skills         SKILL.md discovery and skill-tool execution
  storage        SQLite session/run/audit data and JSONL transcripts
  observability  trace event store and context budget events
```

The broad dependency direction is: applications compose the domain crates;
`protocol` provides shared wire and data types; `core` provides IDs and paths;
`tools` depends on `security` and `skills`; and the daemon combines agent,
tools, storage, and observability into the runtime.

## Runtime Flow

1. The CLI parses a user command such as `exec`, `tool`, `approvals`, `trace`,
   or an interactive prompt.
2. The CLI checks or starts the daemon and reads the HTTP URL from
   `~/.unio/daemon/instance.json`.
3. The CLI calls daemon endpoints such as `/exec`, `/tools/execute`,
   `/approvals`, or `/traces/query`.
4. The daemon resolves or creates a workspace session and creates run,
   conversation, and trace IDs for each execution.
5. The root agent builds model messages from recent transcript history and the
   current user input.
6. The provider returns text or tool calls; the daemon executes returned tool
   calls one by one.
7. Each tool call first goes through security precheck, which decides whether it
   is allowed, denied, or waiting for approval.
8. The daemon persists the run, transcript, approval grant, and trace event.
9. The CLI renders a readable status, summary, approval prompt, or trace
   timeline.

## Daemon API

The daemon uses `axum` to expose a local HTTP API. Key routes include:

- `GET /status`: daemon, session, approval, trace, and model status.
- `GET /models`: current model provider summary.
- `GET /sessions`: session list.
- `POST /sessions/resolve`: resolve or create a session by workspace.
- `POST /sessions/transcript`: load a session transcript.
- `POST /traces/query`: query events by trace ID, optionally filtered by run ID.
- `POST /exec`: execute one user turn.
- `POST /tools/execute`: execute a tool directly.
- `GET /approvals`: list pending approvals.
- `GET /approvals/history`: list approval history.
- `POST /approvals/resolve`: approve or deny a pending approval.

## Agent And Model

`crates/agent` defines the `RootAgent`, `SubAgent`, and `SkillAgent` traits plus
their task and result types. The current root agent creates a `PlanSpec` for
planning triggers and otherwise calls `ResolvedProvider`. Model messages include
a system message, recent transcript history, and the current user input.

`crates/model` resolves configuration through `ProviderConfig`. Environment
variables take precedence over `~/.unio/config.toml`. Provider forms include:

- `mock`: deterministic local provider for development and tests.
- `openai-compatible`: `/chat/completions` style API.
- `anthropic`: `/messages` style API.

If a real provider is requested but the API key is missing, Unio falls back to
mock and marks `fallback_to_mock` in status output.

## Tools And Approvals

`crates/tools` provides the built-in registry. Tool definitions include name,
description, capability, and default risk. Before execution, a `ToolPrecheck` is
constructed and sent to `crates/security`.

Permission modes:

- `Default`: allow workspace reads and plan; writes, processes, network, and
  skill tools require approval.
- `Auto`: allow low-risk workspace writes and trusted low-risk network access;
  elevated risk still requires approval.
- `FullTrust`: allow directly.

In default mode, file writes, process execution, network access, and skill-tool
calls are queued for approval. Users can approve or deny them through CLI or TUI.

## Storage And Observability

`crates/storage` uses SQLite for:

- sessions
- runs
- approval grants

Transcripts are stored as one JSONL file per session. Message records cover user,
assistant, and tool messages.

`crates/observability` stores trace events in JSONL. Each event contains trace
ID, run ID, kind, message, optional token usage, and timestamp. Context budget
events are generated from `context_ratio` with levels such as `normal`,
`summarize`, `compress`, and `critical`.

## Architecture Principles

- The CLI does not own model calls, tool execution, or persistence.
- Every risky tool operation goes through one security policy.
- Explicit IDs connect sessions, runs, conversations, agents, approvals, and
  traces.
- The mock provider keeps local development usable without external
  credentials.
- The shared protocol crate keeps CLI and daemon data contracts aligned.
