# 05 Technical Introduction

## Technology Stack

Unio is organized as a Rust workspace. Key dependencies include:

- `tokio`: async runtime.
- `axum`: daemon HTTP API.
- `clap`: CLI argument parsing.
- `ratatui` and `crossterm`: terminal UI.
- `reqwest`: model APIs and trusted URL fetch.
- `serde`, `serde_json`, and `toml`: protocol and configuration
  serialization.
- `rusqlite`: SQLite persistence.
- `uuid`: session, run, trace, approval, and agent IDs.
- `chrono`: timestamps.
- `anyhow` and `thiserror`: error handling.

## Model Configuration

Model configuration is resolved by `crates/model`. File configuration lives at:

```text
~/.unio/config.toml
```

Environment variables override the config file. Provider selection:

- `UNIO_MODEL_PROVIDER=openai-compatible` or `openai`: use the
  OpenAI-compatible provider.
- `UNIO_MODEL_PROVIDER=anthropic`: use the Anthropic provider.
- Missing or unknown provider: use mock.
- Real provider requested without an API key: fall back to mock and report
  `fallback_to_mock=true`.

## Data Model

Core IDs use stable prefixes:

- `session_...`
- `run_...`
- `trace_...`
- `approval_...`
- `agent_root_...`
- `agent_planner_...`
- `agent_sub_...`
- `agent_skill_...`

These IDs connect CLI output, daemon API responses, SQLite records, JSONL
transcripts, and trace events.

## Persistence Layout

User-level state lives under:

```text
~/.unio/
  daemon/
    instance.json
    logs/
  sessions/
    state.db
    transcripts/
  traces/
    events.jsonl
  config.toml
```

SQLite tables include:

- `sessions`: session metadata, workspace root, permission mode, and latest run.
- `runs`: prompt, final text, trace, provider, model, token data, and context
  ratio.
- `approval_grants`: approval and denial audit records.

Transcripts and traces use JSONL for append-only writes and human inspection.

## Tool Safety Model

Tool execution is split into two steps:

1. `crates/tools` builds a `ToolPrecheck` from the tool definition and
   arguments.
2. `crates/security` returns a security decision for the current permission
   mode.

This keeps tool implementations small and centralizes risk policy.

Capability types include:

- workspace read
- workspace write
- process execution
- network access
- plan only
- skill tool

The default policy is conservative: workspace reads and plans can run directly;
side-effectful operations require approval.

## Model Tool Calls

Model providers return `ModelResponse`, which may contain text and `ToolCall`
records. The daemon executes tool calls, not the provider or agent. This keeps
model integration separate from tool safety.

The OpenAI-compatible provider maps Unio tools into function tools. The
Anthropic provider parses `tool_use` content blocks. The mock provider supports
special inputs such as `mock-tool` and `mock-usage` for testing tool calls and
context budget behavior.

## Context Budget

The current implementation calculates `context_ratio` from token usage against
an estimated 128,000-token ceiling. Observability emits levels:

- `< 0.70`: normal.
- `>= 0.70`: summarize.
- `>= 0.85`: compress.
- `>= 0.90`: critical.

For large tasks at high context ratios, the daemon requires compaction before
continuing to add work.

## Test Organization

Tests live beside implementation in `#[cfg(test)]` modules. Coverage includes:

- ID prefixes and path resolution.
- Protocol schemas.
- Session, run, approval, and transcript persistence.
- Trace events and context budget events.
- Permission policy matrix.
- Tool registry and tool execution.
- Skill discovery and skill-tool output.
- Mock provider behavior, config overrides, and provider fallback.
- CLI output formatting, slash commands, file reference completion, and TUI
  helpers.
- Daemon session, exec, approval, trace, and transcript API behavior.

## Current Limits

- The daemon is a local HTTP runtime and does not yet model remote multi-tenant
  or distributed deployment.
- The `bash` tool uses simple whitespace splitting and rejects shell composition
  syntax.
- `fetch` only allows a small set of trusted URL prefixes.
- Skill execution currently returns structured summaries rather than executing
  complex scripts.
- Sub-agent support currently has a mock implementation, and planning plus
  parallel execution are still foundational.
- Installation and release instructions depend on how complete public release
  artifacts are.

## Maintenance Guidance

- When adding CLI commands, update `docs/src/cli.md`, Chinese docs, and this
  report's usage guide.
- When adding tools or approval rules, update the tools, security, and
  architecture chapters.
- When changing model providers or configuration precedence, update README,
  mdBook, and this report's technical introduction.
- When changing storage schemas or trace events, update the architecture and
  technical introduction.
- Record meaningful code, behavior, architecture, tool, workflow, or
  documentation changes in `CHANGE.md`.
