# 01 Overview

## Positioning

Unio is a Rust-based local intelligent agent runtime for developer terminal
workflows. It combines a CLI, local daemon, model provider abstraction, tool
execution, approval policy, skills, storage, and observability events in one
workspace.

The current project is best understood as a runnable local agent runtime and
foundation platform, not as a single-purpose script. Users start work through
the `unio` command. The CLI sends session, run, tool, and approval requests to a
local daemon, and the daemon owns state, persistence, and safety boundaries.

## Current Capabilities

- `unio exec` creates or resumes a workspace-bound session.
- Prompts can be submitted through direct commands or the terminal UI.
- `/model` configures persistent model settings, with environment variable
  overrides.
- The mock provider supports local development and testing without real model
  credentials.
- OpenAI-compatible and Anthropic-style providers are implemented.
- Built-in tools support file search, read, write, edit, constrained process
  execution, trusted URL fetch, plan generation, and skill invocation.
- Tool execution goes through `crates/security`, which can allow, deny, or queue
  approval.
- Pending approvals, approval resolution, and approval history are available.
- Skills are discovered from `.unio/skills/<name>/SKILL.md` in workspace and
  user roots.
- SQLite stores sessions, runs, and approval grants.
- JSONL stores transcripts and trace events.
- English and Simplified Chinese mdBook documentation exists.

## Primary Use Cases

- Start a local agent session inside a repository and keep prompts, answers,
  tool calls, and traces attached to that workspace.
- Let the model request file reads, writes, edits, or command execution through
  a controlled tool path.
- Require human approval for side-effectful operations in default mode.
- Use the mock provider to test agent, tool, approval, storage, and trace flows
  deterministically.
- Expose repository-level or user-level workflows to the agent through skills.

## Project Shape

Unio is a Rust workspace with two applications and nine functional crates:

- `apps/cli`: user-facing command.
- `apps/daemon`: local runtime and HTTP service.
- `crates/core`: IDs, paths, and shared metadata.
- `crates/protocol`: shared protocol types for CLI, daemon, and agent.
- `crates/agent`: root agent, planning, sub-agent, and skill-agent contracts.
- `crates/model`: model provider abstraction and configuration resolution.
- `crates/tools`: tool registry and tool execution.
- `crates/security`: permission modes and approval policy.
- `crates/skills`: skill discovery and skill execution.
- `crates/storage`: SQLite and JSONL persistence.
- `crates/observability`: trace and context budget events.

## Current Assessment

The implementation already has a complete local runtime loop: the CLI connects
to the daemon, the daemon manages sessions and runs, the agent can call a model
or tools, tools are constrained by security policy, and results are persisted to
transcripts and traces. Near-term hardening areas include broader tests, model
error reporting, secret handling, public installation details, and more
production-grade runtime behavior.
