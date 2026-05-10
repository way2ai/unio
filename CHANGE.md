## Unreleased

- Added a chapter-level bilingual technical project report under `arti_doc/`,
  with a report specification in `docs/arti-doc-report-spec.md`.
- Fixed session permission-mode switching for slash `/approval <mode>` by
  updating the existing workspace session mode on resolve (instead of keeping
  stale mode).
- Fixed approval resolution idempotency UX: resolving a non-existing/already
  resolved approval now returns a structured `not_found` result instead of a
  daemon 404 error path.
- Improved daemon ReAct completion behavior when tool loops do not converge:
  bounded loops now emit explicit loop-limit/final-synthesis fallback text
  instead of silently ending with ambiguous state.
- Stabilized daemon approval deny test by removing dependence on
  model-generated tool calls and using deterministic pending-approval fixtures.
- Improved `/model` config write error reporting for permission-denied cases,
  with explicit guidance to use env vars or grant file write permission.
- Added real-model E2E validation documents:
  `docs/real-model-e2e-test-spec.md` and
  `docs/real-model-e2e-test-report.md` (ReAct, slash commands, tools,
  approvals, and trace observability coverage).
- Recorded current validation findings:
  ReAct continuation can end with raw tool output in some flows, recovery after
  tool failure is not reliable in targeted prompts, `/approval <mode>` direct
  invocation did not switch mode in this run, `/model` direct invocation
  returned `os error 5` in this context, and repeated approval resolution on an
  already-resolved ID returned daemon `404`.
- Captured workspace test status during this validation run:
  `cargo test --workspace` currently fails at daemon test
  `approval_resolution_denies_pending_tool_and_records_denied_trace`
  (`apps/daemon/src/lib.rs:1748` index out of bounds).
- Updated session lifecycle behavior: `unio`/`unio exec` now create a new
  session by default for each workspace run, TUI `/resume` now opens a
  workspace session picker (`Up/Down`, `Enter`, `Esc`) to switch active
  sessions, and `/new` creates and switches to a fresh session.
- Updated interactive approval slash commands:
  `/approval` now shows or sets the current approval mode
  (`default|auto|full-trust`), and `/pending` shows pending approvals.
- Added guided root slash behavior for `/approve`, `/deny`, and `/trace`.
  Root forms now guide usage, while parameterized forms remain supported.
- Added persistent real-model configuration through `~/.unio/config.toml`,
  with environment variables still taking precedence and mock fallback
  preserved when real provider credentials are missing.
- Collapsed model configuration to a single `/model` slash command that opens
  provider and model selection while removing `/models`, `/model config`, and
  `/model use`.
- Added friendlier CLI and TUI run rendering for model, tool, skill, approval,
  and completion events.
- Updated interactive TUI turn rendering to a structured block format:
  `> user input`, `process`, `result`, and `Worked for` duration.
- Added colored status dots in interactive TUI rendering for process rows,
  with dot color driven by step status.
- Fixed TUI approval-wait rendering: when a run is waiting for approval, hide
  the `result` block and keep only tool-step progress.
- Added approval review card UX in TUI, including operation preview derived
  from pending tool arguments plus arrow-key selection (`Up/Down` + `Enter`);
  numeric shortcuts `1/2/3` remain supported.
- Updated daemon tool orchestration to feed failed tool results back into model
  context and allow one automatic retry with adjusted tool choices.
- Updated daemon tool orchestration to run a bounded ReAct continuation after
  tool calls (not only failures), so successful tool execution is followed by a
  model summary instead of stopping at raw tool output.
- Updated approval resolution flow: when a tool approval is granted and tool
  execution completes, daemon now triggers one automatic follow-up model turn
  and returns that final answer in approval response payload.
- Updated `glob` tool output to always include `pattern` and `count` (and
  matched paths when present), so the agent can reliably answer quantity
  questions instead of ending with empty tool output.
- Fixed approval follow-up behavior for failed tool execution: daemon now logs
  structured tool failure output into transcript and runs a follow-up model
  turn, so users see actionable continuation instead of stopping at
  `status: failed`.
- Added environment-aware command guidance in both system prompt and `bash`
  tool description so model-generated commands match host shell conventions.
- Fixed Windows process-output decoding for tool execution by decoding
  non-UTF-8 command output with GBK fallback, preventing garbled CLI text.
- Updated the interactive terminal surface so submitted prompts clear from the
  editing area immediately, before the agent response returns.
- Rewrote the documentation set around product and developer workflows instead
  of historical implementation notes. The mdBook now contains Overview,
  Install, Quickstart, CLI, Tools And Approvals, Skills, Architecture,
  Development, Release, and Site Operations pages.
- Rebuilt the GitHub Pages landing experience with mdBook theme customizations:
  hero section, command examples, capability cards, workflow section, and
  English/Chinese language switcher.
- Added synchronized Simplified Chinese mdBook pages under `docs/zh/src/` and
  removed the unused gettext PO translation file.
- Removed historical implementation documentation, including decision records,
  future-architecture notes, current-state notes, and narrow terminal-surface
  implementation pages.
- Updated `README.md`, `docs/README.md`, and `docs/get-started.md` to point to
  the new documentation structure.
- Updated `.github/workflows/release.yml` to stop building the macOS x86_64
  release artifact. Release targets are now Linux x86_64, Windows x86_64, and
  macOS Apple Silicon.
