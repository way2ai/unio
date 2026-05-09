# Real Model E2E Test Spec

Date: 2026-05-09
Scope: ReAct behavior, slash commands, tools, approvals, and trace observability in real-model runtime.

## Objectives

- Verify real provider execution without mock fallback.
- Validate ReAct behavior across tool success and failure.
- Validate slash command root and parameter forms.
- Validate tool success/failure and approval resolution flows.
- Validate trace/run/session observability.

## Environment

- Workspace: `F:\github\way2ai\unio`
- CLI: `cargo run -p unio -- ...`
- Runtime status requirement:
  - `provider` is non-mock.
  - `fallback_to_mock` is `false`.

## Test Matrix

1. ReAct
- `exec` prompt that requires `read` tool and final answer synthesis.
- `exec` prompt that requires `glob` count and final answer.
- `exec` prompt that triggers tool failure and recovery.

2. Slash Commands
- Root forms: `/help`, `/skills`, `/approval`, `/pending`, `/approve`, `/deny`, `/trace`, `/refresh`, `/update`, `/resume`, `/new`, `/quit`.
- Parameter forms: `/trace <trace_id> [run_id]`, `/resume <limit>`, `/approval <mode>`, `/approve <id>`, `/deny <id>`.
- `/model` direct invocation behavior in non-interactive CLI context.

3. Tools
- Success: `tool read`, `tool glob`, `tool grep`, `tool bash`, `tool fetch`.
- Failure: invalid `tool grep` args, invalid `tool bash` command.
- Approval-mode behavior with `default|auto|full-trust`.

4. Approvals
- Pending request creation from elevated-risk tool call.
- Resolve via approve path and observe result status.
- Resolve via deny path and ensure stable user-facing behavior.

5. Observability
- Query latest trace.
- Query explicit `trace_id + run_id`.
- Validate tool events and run completion visibility.

## Acceptance Criteria

- P0:
  - Real provider active and model turn completes.
  - `read/bash/glob/grep/fetch` success path works.
  - `/trace` and `/resume` commands work in direct CLI invocation.
- P1:
  - Approval flow handles already-resolved IDs gracefully.
  - `/approval <mode>` and `/model` direct invocation work per docs.
- P2:
  - ReAct returns final synthesized answer after tools, not only raw tool output.
