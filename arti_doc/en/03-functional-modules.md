# 03 Functional Modules

## `apps/cli`

The CLI is the user entry point. It owns command parsing, the interactive TUI,
slash commands, file reference completion, daemon client calls, and output
rendering.

Main capabilities:

- `exec`: submit a prompt and create or reuse a session.
- `resume`: inspect recent transcript messages.
- `sessions`: list sessions.
- `skills`: list discovered skills.
- `models`: inspect model status.
- `status`: inspect daemon and runtime state.
- `trace`: query traces.
- `tool`: execute a tool directly.
- `approvals`: list, approve, or deny approvals.
- `daemon start/status`: manage the local daemon.
- no subcommand: enter the TUI.

The TUI supports `/` command suggestions, `@` workspace file references, a model
configuration wizard, approval shortcuts, and common trace/resume/status flows.

## `apps/daemon`

The daemon is the runtime core. It starts the local HTTP service, builds
`DaemonState`, opens the SQLite store, initializes the JSONL trace store, and
maintains the in-memory pending approval queue.

Responsibilities:

- Expose the local HTTP API.
- Resolve and create sessions.
- Execute user turns.
- Call the root agent.
- Execute tool calls returned by the model.
- Manage pending approvals and approval history.
- Persist transcripts, runs, and approval grants.
- Write trace and context budget events.

The daemon is the composition layer for agent, tools, storage, and observability.

## `crates/core`

The core crate provides foundational shared types:

- `SessionId`, `RunId`, `TraceId`, `ApprovalId`, and `AgentId`.
- `AgentKind` and ID prefixes.
- `WorkspacePaths`, including user and workspace skill roots.
- `UserPaths`, including `~/.unio` daemon, session, transcript, and trace paths.
- daemon instance file read/write helpers.

This crate avoids business logic and focuses on shared structures used across
other crates.

## `crates/protocol`

The protocol crate defines shared data structures used by the CLI, daemon, and
agent:

- `PermissionMode`
- `RunStage`
- session resolve and list types
- exec turn request and response types
- transcript message schema
- model status
- daemon status
- trace query and trace event records
- tool execute request and response types
- approval list, resolve, and history types

It is the main source for cross-process and cross-module contracts.

## `crates/agent`

The agent crate defines agent abstractions and the current root agent runtime.

Key types:

- `AgentRuntime`: session, run, trace, input, history, and permission context for
  one agent run.
- `AgentOutcome`: final text, optional plan, tool calls, events, provider
  summary, and token data.
- `RootAgent`, `SubAgent`, and `SkillAgent` traits.
- `PlanSpec`, `PlanStep`, and `StepExecutor`.

The current root agent generates a fixed plan shape for planning triggers.
Otherwise, it calls the model provider. If the model returns tool calls, the
daemon handles execution.

## `crates/model`

The model crate owns provider abstraction, configuration resolution, and API
adapters.

Capabilities:

- `ModelProvider` trait.
- `ModelRequest` and `ModelResponse`.
- `ProviderConfig` from config file plus environment variables.
- `ResolvedProvider` selecting mock, OpenAI-compatible, or Anthropic.
- `MockModelProvider` for deterministic local behavior and test tool requests.
- `OpenAiCompatibleProvider` for chat completions APIs.
- `AnthropicProvider` for messages APIs.

The config file is `~/.unio/config.toml`; environment variables override it.

## `crates/tools`

The tools crate defines the tool registry and execution logic. Built-in tools:

- `glob`: find files by wildcard.
- `grep`: search text.
- `read`: read workspace files.
- `edit`: replace an exact text fragment in a file.
- `write`: create or overwrite files.
- `bash`: run constrained commands without shell composition syntax.
- `fetch`: fetch trusted URLs only.
- `plan`: return read-only plan JSON.
- `skill-tool`: invoke a skill agent.

Tool execution uses `ToolExecutionContext` for workspace root, user home, and
permission mode.

## `crates/security`

The security crate provides the centralized approval policy. It does not execute
tools. Given a `ToolPrecheck` and `PermissionMode`, it returns:

- `Allow`
- `RequireApproval`
- `Deny`

Capability types include workspace read, workspace write, process execution,
network access, plan only, and skill tool. This crate is the center of risk
decisions for tools.

## `crates/skills`

The skills crate discovers skills from two roots:

- `{workspace}/.unio/skills/`
- `~/.unio/skills/`

Each skill directory contains a `SKILL.md`. The first non-empty line is used as
the description, and skills can be injected as callable tool definitions.
`skill-tool` execution returns a structured result without exposing the full
private skill body.

## `crates/storage`

The storage crate contains two persistence paths:

- SQLite for sessions, runs, and approval grants.
- JSONL transcripts for per-session user, assistant, and tool messages.

The SQLite store supports session resolution, session listing, run insertion,
latest context ratio lookup, and approval history lookup.

## `crates/observability`

The observability crate owns the trace event JSONL store and context budget
events.

Capabilities:

- Append trace events.
- Summarize latest trace ID and event count.
- Query events by trace ID.
- Generate summary, compression, and critical context events from context ratio.

This makes tool, approval, model, and context state inspectable after a run
finishes.
