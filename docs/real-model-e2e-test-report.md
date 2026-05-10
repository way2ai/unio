# Real Model E2E Test Report

Date: 2026-05-09
Workspace: `F:\github\way2ai\unio`
Provider: `openai-compatible / kimi-k2.6`
Fallback to mock: `false`

## Summary

- Baseline runtime is healthy and real model path is active.
- Core tool execution works (`read/glob/grep/bash/fetch` success paths passed).
- Multiple slash commands work in direct invocation mode.
- Several high-impact behavior gaps were reproduced and are actionable.

## Executed Scenarios

1. Runtime status
- `cargo run -p unio -- status`
- Result: passed.

2. Slash commands (root forms)
- `/help`, `/skills`, `/approval`, `/pending`, `/approve`, `/deny`, `/trace`, `/refresh`, `/update`, `/resume`, `/new`, `/quit`
- Result: mostly passed.

3. Slash commands (parameter forms)
- `/trace <trace_id> <run_id>`, `/resume 2`, `/approval auto|default|full-trust`, `/approve <id>`, `/deny <id>`
- Result: partial pass, with failures listed below.

4. Tool command coverage
- `tool read --args path=README.md`
- `tool glob --args pattern=*.md`
- `tool grep --args query=slash,path=README.md`
- `tool bash --args "command=cmd /c echo hello"`
- `tool fetch --args url=https://docs.rs/anyhow/latest/anyhow/`
- Result: passed on valid args; invalid arg shape fails as expected.

5. ReAct prompts
- Prompt requiring read+answer synthesis: passed.
- Prompt requiring glob count and return integer: incorrect behavior observed in one run (`pattern=**/*.md` -> `count=0`), and tool-output-only termination in another run.
- Prompt requiring failure then recovery: failed to recover; run ended on failed tool.

6. Observability
- `/trace` without args (guided) and explicit `/trace <trace_id> <run_id>` both returned trace timeline.
- Result: passed.

## Findings

1. P0: ReAct may terminate with raw tool output instead of final synthesized answer.
- Evidence: run `run_be590770-079b-4441-86b4-bce4863f6b37` / trace `trace_a777a6b5-7754-4666-96e7-094c8afe69ed`.
- Observed repeated `tool.react_continued` + repeated `glob`, then completion with raw tool block.

2. P0: Failure-recovery prompt did not recover after tool failure.
- Evidence: run `run_45a3948a-e034-4037-b09c-2ab2cb63a3fe` / trace `trace_b1d2a7f2-0eb1-40ad-b570-273975dc849e`.
- Observed final state `tool=bash status=failed error=program not found`, no successful follow-up result.

3. P1: `/approval <mode>` direct slash invocation did not switch mode.
- Evidence: `/approval auto` and `/approval default` both report `approval_mode: full-trust`.
- Expected: mode should change according to argument.

4. P1: `/model` direct invocation fails in this context with OS error 5.
- Evidence: `cargo run -p unio -- "/model"` -> `Error: 拒绝访问。 (os error 5)`.
- Expected: interactive model setup should be usable from direct slash path or fail with explicit non-interactive guidance.

5. P1: Denying an already-resolved approval returns daemon 404 error.
- Evidence: after approving `approval_fbf89e58-13ea-48e9-9595-817dcea6fe31`, running `/deny` on same id returns:
  `daemon rejected approval resolution` + `HTTP status client error (404 Not Found)`.
- Expected: graceful "already resolved/not found" user-facing message.

6. P1: Workspace tests currently fail in daemon approval deny unit test.
- Evidence: `cargo test --workspace` failed at
  `tests::approval_resolution_denies_pending_tool_and_records_denied_trace`
  with index out of bounds in `apps/daemon/src/lib.rs:1748:42`.

## Pass/Fail Snapshot

- Passed:
  - Real-model runtime activation and status.
  - Core tools success paths.
  - Trace query and resume query.
  - Most slash root commands.
- Failed or unstable:
  - ReAct completion quality in multi-step tool scenarios.
  - Slash mode switch behavior for `/approval <mode>`.
  - `/model` direct slash execution in this CLI context.
  - Idempotent handling of repeated approval resolution.
  - One daemon unit test in workspace suite.
