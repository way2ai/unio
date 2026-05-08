# Target Architecture

Unio targets a daemon-backed intelligent agent system with one user-facing
entrypoint: `unio`.

## Hybrid CLI

- Bare `unio` opens the hybrid CLI/TUI surface.
- Scriptable commands remain available: `exec`, `status`, `resume`, `trace`,
  `approvals`, `skills`, `models`, `tool`, and `daemon`.
- The hybrid surface owns prompt editing, slash suggestions, file references,
  approval hotkeys, status display, trace timelines, and context indicators.
- CLI/hybrid surfaces never call models or execute tools directly. They submit
  requests to the daemon.

## Daemon

- The daemon is the runtime owner for sessions, conversations, runs, approvals,
  event broadcast, storage, trace persistence, and tool execution.
- It coordinates agent execution and keeps CLI behavior protocol-backed.

## Agent

- Root agent decides whether to answer directly, call tools, invoke the planner,
  or dispatch sub-agents.
- Planner is read-only and returns `PlanSpec`.
- Sub-agents and skill-agents use independent `agent_id` values and isolated
  context, returning structured results instead of raw context dumps.
- Context budgeting records token usage and context ratio, with warnings and
  compression thresholds.

## Tools And Security

- Tools are exposed through a registry and executed through a common contract.
- Security precheck runs before execution.
- `default`: read-only and plan tools are allowed; write, process, network, and
  skill-tool actions require approval.
- `auto`: low-risk workspace actions can run automatically; high-risk actions
  require approval.
- `full-trust`: tool execution is allowed directly.

## Skills

- Skills are discovered from `{workspace}/.unio/skills/` and `~/.unio/skills/`.
- Each skill directory must contain `SKILL.md`.
- Skill execution goes through `skill-tool` and a skill-agent. The root agent
  does not receive full skill bodies.

## Storage And Observability

- SQLite stores sessions, runs, approval grants, model config, update cache, and
  aggregate indexes.
- JSONL stores message-level transcript records and trace events.
- Trace records connect model calls, tools, approvals, token usage, cost, and
  context ratio by `trace_id`.
