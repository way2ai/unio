# Unio Next Implementation Plan

## Current State

Unio has moved past the initial crate skeleton and mock-only loop. The current
architecture already has the core domain boundaries in place:

- `apps/cli`: user-facing `unio` command.
- `apps/daemon`: local runtime owner for sessions, runs, approvals, trace, and
  tool execution.
- `crates/protocol`: shared CLI/daemon protocol types.
- `crates/core`: IDs, paths, daemon instance metadata, and shared utilities.
- `crates/agent`: root agent, planner contract, sub-agent contract, and
  skill-agent contract.
- `crates/model`: OpenAI-compatible, Anthropic, and mock model providers.
- `crates/tools`: built-in tool registry and execution contract.
- `crates/security`: permission policy and risk precheck.
- `crates/skills`: skill discovery and structured skill-tool execution.
- `crates/observability`: trace events, token usage, and context ratio events.
- `crates/storage`: SQLite metadata and JSONL transcript/trace persistence.

Implemented runtime capabilities:

- `unio exec` creates or resumes a workspace session through the daemon.
- Root agent can call the model provider and receive model tool calls.
- Mock provider supports local test directives including `mock-tool`,
  `mock-context`, and `mock-usage`.
- Built-in tools include `glob`, `grep`, `read`, `edit`, `write`, `bash`,
  `fetch`, `plan`, and `skill-tool`.
- Tool execution goes through `security` before running.
- Approval queue and approval history exist.
- Planner produces a displayable `PlanSpec`.
- Planner can trigger a mock sub-agent result.
- `skill-tool` discovers `.unio/skills/<name>/SKILL.md` and returns a
  structured skill-agent result without echoing the full skill body.
- Trace records model/tool/approval/planning/sub-agent/skill/context events.
- Context budget emits events at 70%, 85%, and 90% ratios.

## Completed Phase History

### Phase 1: Documentation And New Crate Boundaries

Status: completed.

What was established:

- The project was reset around the new Unio architecture instead of the old
  implementation shape.
- The workspace was organized by domain crates instead of an `engine/*` style
  layout.
- The user-facing package and binary name were normalized to `unio`.
- Architecture documentation was created under `docs/`.
- `docs/get-started.md` was added for first-use workflows.
- The old migration-first framing was dropped; the old system is reference
  material only, not a compatibility constraint.

Acceptance reached:

- The repository has the target crate boundaries.
- Users can read the docs and understand the new runtime shape.
- The project builds around the new package names.

### Phase 2: Minimal Runnable Main Path

Status: completed.

What was implemented:

- `unio` CLI can start/connect to the local daemon.
- The daemon owns session and run lifecycle.
- `unio exec` submits a prompt through the daemon.
- Root agent runs through the model abstraction.
- Mock model provider provides deterministic local behavior.
- CLI displays the daemon response.

Acceptance reached:

- A user can run `unio exec "hello"` and get a response.
- The response goes through `CLI -> daemon -> root agent -> model -> daemon ->
  CLI`.

### Phase 3: Protocol, Storage, And Trace

Status: completed.

What was implemented:

- Shared protocol types for sessions, runs, transcript, trace, tools,
  approvals, and model status.
- SQLite persistence for sessions, runs, approval grants, and related metadata.
- JSONL transcript persistence at message level.
- JSONL trace persistence with `trace_id` lookup.
- CLI commands for `status`, `sessions`, `resume`, and `trace`.
- Token usage and context ratio records.
- Context budget trace events at 70%, 85%, and 90%.

Acceptance reached:

- Sessions can be resumed.
- Transcript records user, assistant, and tool messages.
- Trace can be queried by `trace_id`.
- Context pressure is visible in trace output.

### Phase 4: Tools, Security, Approval, Planner, Sub-Agent, And Skill-Tool

Status: completed.

What was implemented:

- Built-in tool registry with `glob`, `grep`, `read`, `edit`, `write`, `bash`,
  `fetch`, `plan`, and `skill-tool`.
- Security precheck and permission decisions for `default`, `auto`, and
  `full-trust`.
- Approval queue, approval resolve endpoint, and approval history.
- Model-returned tool calls are routed through the same security/tool path as
  explicit `/tool` calls.
- Planner creates a displayable `PlanSpec`.
- Planner can trigger a mock sub-agent and record `sub_agent.completed`.
- Skill discovery reads workspace and user skill roots.
- `skill-tool` returns a structured skill-agent result and avoids returning the
  full `SKILL.md` body.
- Skill trace events are emitted for explicit skill-tool execution.

Acceptance reached:

- Read-only tools execute directly in `default` mode.
- Write/process/network/skill tools require approval in `default` mode.
- Low-risk workspace writes can execute in `auto` mode.
- `full-trust` runs tools directly.
- Planner and sub-agent paths produce trace events.
- `skill-tool` can execute a discovered workspace skill and return structured
  output.

## Next Target

The next target is to turn the current explicit tool and skill execution into a
model-driven agent loop that can handle real tasks through this path:

`CLI -> daemon -> root agent -> model -> tool/skill approval -> execution -> trace/transcript -> CLI`

This phase should keep the system runnable and avoid broad rewrites. The work is
mainly about connecting already-defined pieces into one coherent execution loop.

## Phase 5A: Model-Driven Tool And Skill Loop

Status: completed.

### Goals

- Let root agent execute model-returned tool calls through the same daemon path
  used by explicit `/tool`.
- Preserve permission behavior for all model-requested tools.
- Make `skill-tool` usable from the model provider tool-call interface.
- Record structured trace events for tool, approval, and skill execution.
- Keep transcript message-level and avoid injecting full skill bodies into root
  context.

### Tasks

1. Normalize tool-call arguments
   - Ensure OpenAI-compatible and Anthropic tool-call arguments map into the same
     `ToolCall` shape.
   - Add tests for `skill-tool` model calls. (done)

2. Strengthen daemon tool-call orchestration
   - Keep explicit `/tool` and model-requested tools on one execution path.
   - Add consistent events for:
     - `tool.started` (done)
     - `tool.completed` (done)
     - `tool.failed` (done)
     - `approval.requested` (done)
     - `approval.resolved` (done)
     - `skill.started` (done)
     - `skill.completed` (done)
     - `skill.failed` (done)

3. Improve approval resume behavior
   - When an approval is resolved, append the approved tool result to transcript.
   - Add a trace event that links the approval result back to the original run.
     (done for `approval.resolved`)
   - Add skill-specific trace events after approved or denied skill-tool
     approvals. (done)
   - Make `unio approvals history` sufficient to audit what happened.

4. Add model-driven skill-tool tests
   - Use mock provider to emit `skill-tool`.
   - Create a temporary `.unio/skills/repo/SKILL.md`.
   - Verify returned assistant output contains structured skill-agent result.
   - Verify trace contains skill events.
   - Unit coverage exists for the mock model skill-tool call shape; daemon E2E
     verification remains part of the manual checklist below.

### Acceptance

- `unio exec "mock-tool skill-tool name=repo,request=inspect-modules"` executes
  the skill path.
- In `default` mode, skill-tool requires approval.
- In `full-trust` mode, skill-tool executes directly.
- Trace shows `skill.started` and `skill.completed`.
- Transcript contains the tool result but not the full `SKILL.md` body.

## Phase 5B: Resume And Context Rebuild

Status: completed.

### Goals

- Make resumed sessions useful for real multi-turn work.
- Rebuild root-agent context from message-level transcript.
- Keep context budget visible and traceable.

### Tasks

1. Add transcript filtering
   - Keep recent user/assistant messages. (done)
   - Include compact tool result summaries. (done)
   - Exclude full skill bodies and large tool outputs. (done)

2. Add context budget policy
   - At 70%, record summary-needed state.
   - At 85%, create a compression checkpoint.
   - At 90%, require compaction before continuing large tasks. (done)

3. Add CLI visibility
   - Show latest context ratio in `unio status`. (done)
   - Show context budget events in `unio trace`. (done)

### Acceptance

- `unio resume` displays the active session and recent messages.
- A follow-up `unio exec` can use prior user/assistant context.
- Long runs emit context events and do not silently overflow the budget.

## Phase 5C: CLI Product Surface

Status: completed.

### Goals

- Bring the CLI closer to codex/claude-code style first-use ergonomics before
  investing in a full TUI.

### Tasks

1. Add slash-command compatible CLI aliases
   - `/skills` (done)
   - `/model` (done)
   - `/resume` (done)
   - `/approval` (done)
   - `/update` (done: local configured version check)
   - `?` (done)

2. Improve `unio skills`
   - Show source: workspace or user. (done)
   - Show path. (done)
   - Show first-line description. (done)

3. Improve `unio status`
   - Show daemon status. (done)
   - Show model provider. (done)
   - Show latest session. (done)
   - Show latest trace. (done)
   - Show latest context ratio. (done)
   - Show workspace, daemon started_at, and pending approval count. (done)

3.5. Improve `unio approvals`
   - Pending approvals show id, tool, reason, workspace, and requested_at. (done)
   - Approval history shows approve/deny, reason, workspace, and resolved_at.
     (done)

4. Improve `unio trace`
   - Group events by run. (done)
   - Make tool, approval, skill, and context events easy to scan. (done)

### Acceptance

- A user can inspect model, skills, approvals, sessions, and trace without
  reading internal files.
- CLI output is stable enough to use in integration tests.

## Phase 5D: TUI Foundation

Status: completed as an interactive shell foundation. A full ratatui interface
is deferred to a later UI hardening phase.

### Goals

- Start the TUI only after the daemon protocol is stable enough.
- Keep first TUI minimal but connected to the real runtime.

### Tasks

1. Build top status panel
   - Logo. (done in interactive shell header)
   - Model. (done)
   - Current version. (done)
   - Workspace directory. (done)
   - GitHub URL. (done)

2. Build message stream
   - User prompt. (done in interactive shell)
   - Assistant output. (done through existing exec path)
   - Tool events. (available through trace output)
   - Approval waiting state. (done: prompt tells user to review approvals)

3. Build command handling
   - `/skills` (done)
   - `/model` (done)
   - `/resume` (done)
   - `/approval` (done)
   - `?` (done)

4. Build approval interaction
   - Show pending approval card. (done as `/approval` list)
   - Approve or deny from the TUI. (done as `/approve <id>` and `/deny <id>`)
   - Lock input while waiting if the run cannot continue. (deferred until full
     terminal UI)

### Acceptance

- Running `unio` with no subcommand opens an interactive session. (done)
- A normal prompt returns a daemon-backed response. (done)
- A tool requiring approval displays an approval state. (done)
- The interaction shell uses the same daemon protocol as CLI commands. (done)

## Phase 5E: Interaction Hardening

Status: completed.

### Goals

- Make the daemon-backed shell useful for real work before building a full
  ratatui interface.
- Keep script compatibility for users who only want final text.

### Tasks

1. Show run metadata after `exec`
   - Stage. (done)
   - Run id. (done)
   - Trace id. (done)
   - Model provider/model. (done)
   - Token usage and context ratio. (done)
   - Event summary. (done)

2. Preserve script-friendly output
   - Add `unio exec --quiet <prompt>` to print only final text unless approval
     intervention is required. (done)

3. Improve interactive command ergonomics
   - Keep `/approve <id>` and `/deny <id>`.
   - Add `/trace <id>` for in-session trace inspection. (done)
   - Add concise metadata after each prompt response. (done)

4. Add daemon E2E coverage
   - Model-requested `skill-tool` executes under `full-trust`. (done)
   - Model-requested `skill-tool` enters approval under `default`. (done)
   - Trace and transcript are persisted without leaking full `SKILL.md` bodies.
     (done)

## Phase 5F: Hybrid CLI/TUI Runtime Surface

Status: completed as the first daemon-backed hybrid CLI/TUI slice.

### Goals

- Make bare `unio` open the interactive ratatui surface, matching Codex and
  Claude Code style.
- Keep scriptable subcommands such as `exec`, `status`, `resume`, `trace`, and
  `approvals`.
- Do not expose a second user-facing `tui` command.

### Tasks

1. Make bare `unio` open the hybrid CLI/TUI surface. (done)
2. Render a top status panel with version, model, workspace, daemon URL, and
   pending approvals. (done)
3. Render a message area and input box. (done)
4. Send prompts through the existing daemon `exec` endpoint. (done)
5. Show approval waiting text when a run requires approval. (done via existing
   run metadata and stage display)
6. Add TUI-local `/approval` and `/refresh` commands. (done)
7. Remove the standalone `unio tui` and `/tui` entry points. (done)

## Phase 5G: Hybrid Approval Resolution And Resume Surface

Status: completed.

### Goals

- Let users handle common long-running workflows from the hybrid surface without
  dropping back to plain CLI commands.
- Keep the UI behavior protocol-backed rather than adding a second runtime path.

### Tasks

1. Add hybrid `/approve <id>` and `/deny <id>` commands. (done)
2. Add hybrid `/resume` to show the latest transcript. (done)
3. Add hybrid `/trace <id>` to inspect run events inline. (done)
4. Add status refresh after approval resolution. (done)
5. Keep all approval, resume, and trace behavior on existing daemon endpoints.
   (done)

## Phase 5H: Hybrid Interaction Polish

Status: completed.

### Goals

- Improve the first hybrid surface so it feels usable for longer sessions.
- Keep visual work restrained and focused on agent/runtime state.

### Tasks

1. Add visible run stage labels for planning, tool calling, waiting approval,
   and completed runs. (done)
2. Add a compact help view for supported slash commands. (done)
3. Keep the input area stable with long prompts. (done)
4. Add a simple scroll offset for the message area. (done)
5. Add focused unit coverage for text formatters used by the hybrid surface.
   (done)

## Phase 5I: Hybrid Event Timeline

Status: completed.

### Goals

- Make model, tool, approval, skill, and context events easier to inspect during
  a run.
- Reuse existing trace records instead of adding a separate event store.

### Tasks

1. After each completed run, load its trace by `trace_id`. (done)
2. Render a compact event timeline in the hybrid message stream. (done)
3. Highlight token/context events in the timeline text. (done)
4. Keep full trace inspection available through `/trace <id>`. (done)
5. Add tests for compact trace timeline formatting. (done)

## Phase 5J: Hybrid Slash Command Parity

Status: completed.

### Goals

- Bring the hybrid surface closer to the existing CLI slash surface.
- Avoid duplicating business logic by extracting formatting helpers.

### Tasks

1. Add hybrid `/skills`. (done)
2. Add hybrid `/model` and `/models`. (done)
3. Add hybrid `/update`. (done)
4. Keep `?` help aligned with supported hybrid commands. (done)
5. Add formatter tests for model and skill output. (done)

## Phase 5K: Hybrid Approval Hotkeys And Input Modes

Status: completed.

### Goals

- Make approval handling faster in the hybrid surface.
- Preserve explicit slash commands for auditability and script parity.

### Tasks

1. Track the latest pending approval id in hybrid state. (done)
2. Add `a` hotkey to approve the latest pending approval when input is empty.
   (done)
3. Add `d` hotkey to deny the latest pending approval when input is empty.
   (done)
4. Show the hotkey target in the status panel when one exists. (done)
5. Add tests for parsing the latest approval id from pending approval responses.
   (done)

## Phase 6A: Runtime Integration Tests For CLI/Hybrid Surfaces

Status: completed.

### Goals

- Lock down the current product surface before adding more agent behavior.
- Test protocol-facing flows through the CLI/daemon boundary where possible.

### Tasks

1. Add CLI integration tests for `exec --quiet`. (done through CLI parser and
   quiet output coverage)
2. Add CLI integration tests for slash-compatible prompt commands. (done)
3. Add daemon-backed approval resolution tests that mirror hybrid hotkey behavior.
   (done)
4. Add transcript/trace formatter snapshot-style tests. (done)
5. Document any remaining manual hybrid checks in the verification checklist.
   (done)

## Phase 6B: CLI Command Output Consolidation

Status: completed.

### Goals

- Keep CLI and hybrid text surfaces consistent as more commands are added.
- Reduce duplicated printing logic before adding richer runtime views.

### Tasks

1. Extract approval history formatting. (done)
2. Extract status formatting. (done)
3. Extract tool execution response formatting. (done)
4. Add tests for approval history, status, and tool output formatters. (done)
5. Keep CLI commands as thin wrappers around fetch/format/print helpers. (done)

## Phase 6C: Command Error And Exit Surface

Status: completed.

### Goals

- Make CLI failures easier to diagnose during real use.
- Keep error text consistent across daemon connectivity, approvals, tool calls,
  and trace lookup.

### Tasks

1. Add helper context to daemon connection failures. (done)
2. Add helper context to request failures for exec, tool, trace, and approvals.
   (done)
3. Keep not-found style daemon errors readable. (done through status-code
   context)
4. Add tests around parse/format helpers where no live daemon is required.
   (done in Phase 6A/6B)
5. Document expected behavior for daemon-not-running and daemon-start-failed
   cases. (done below)

Expected command error behavior:

- `unio status` without an instance file returns `daemon not running; run
  unio daemon start`.
- Auto-start commands add `failed to start or connect to unio daemon` around
  spawn/connect failures.
- Request failures identify the operation, for example `daemon rejected exec
  request` or `failed to decode trace lookup response`.

## Phase 6D: Storage And Trace Query Refinement

Status: completed.

### Goals

- Make trace and transcript retrieval more useful as sessions grow.
- Keep JSONL message-level persistence unchanged.

### Tasks

1. Add optional limit handling to transcript loading. (done)
2. Add optional run id filtering to trace lookup. (done)
3. Preserve current response shapes for existing callers. (done)
4. Add daemon handler tests for transcript limit and trace run filtering. (done)
5. Update CLI/hybrid callers only after protocol additions are stable. (deferred to
   Phase 6E)

## Phase 6E: CLI/Hybrid Query Controls

Status: completed.

### Goals

- Expose the new transcript and trace query controls through user-facing
  commands.
- Keep default CLI/hybrid behavior unchanged.

### Tasks

1. Add `unio resume --limit <n>`. (done)
2. Add `unio trace <trace_id> --run <run_id>`. (done)
3. Add hybrid `/resume <n>` shorthand. (done)
4. Add hybrid `/trace <trace_id> <run_id>` shorthand. (done)
5. Add parser and formatter tests for the new query controls. (done)

## Phase 6F: File Reference And Command Hint Follow-Through

Status: completed.

### Goals

- Turn the hybrid UI hints for `/`, `?`, and `@` into more complete command
  behavior.
- Keep command handling local to the CLI unless daemon context is required.
- This phase only parses file references and improves help rendering; it does
  not preview or validate referenced files.

### Tasks

1. Add parser support for `@path` references in prompts. (done)
2. Add a real `/help` view aligned with the bottom `?` hints. (done)
3. Add tests for file-reference parsing and help rendering. (done)
4. Allow `@path` at line start as well as after whitespace. (done)
5. Serve `@` suggestions from a lightweight background file index that prunes
   ignore files, hidden folders, and heavy generated directories. (done)
6. Optimize `@` suggestion filtering for millisecond-level interaction by using
   precomputed path metadata and top-50 candidate ranking. (done)
7. Add prefix-matched slash command suggestions with keyboard selection and
   Enter completion. (done)

## Phase 6G: Hybrid Input Editing

Status: completed.

### Goals

- Improve editing ergonomics inside the hybrid input line.
- Keep terminal behavior predictable and local to the CLI.

### Tasks

1. Add cursor movement inside the input line. (done)
2. Add delete-word and clear-line shortcuts. (done)
3. Add multiline prompt support with explicit keybinding. (done)
4. Add tests for input editing state transitions. (done)

## Non-Goals For The Next Phase

- Do not rewrite the whole project again.
- Do not add compatibility with old sessions or old skill formats.
- Do not implement a production-grade OS sandbox yet.
- Do not build decorative terminal UI surfaces before the protocol and runtime behavior are
  stable.
- Do not inject full skill contents into the root-agent context.

## Phase 7A: mdBook Site And GitHub Pages Automation

Status: completed.

### Goals

- Publish the Unio documentation as a static site via GitHub Pages.
- Reuse and improve the existing `docs/` content rather than adding a parallel
  documentation tree.
- Keep the CI pipeline honest: tests must pass before the site is published.

### Tasks

1. Create `docs/book.toml` with conservative default configuration. (done)
2. Create `docs/src/SUMMARY.md` with the first-version table of contents. (done)
3. Migrate existing docs into `docs/src/` pages:
   - `overview.md` from `docs/README.md`. (done)
   - `get-started.md` from `docs/get-started.md`. (done)
   - `architecture.md` from `docs/architecture/README.md`. (done)
   - `hybrid-file-references.md`, `hybrid-slash-commands.md`,
     `hybrid-input-editing.md` from `docs/architecture/`. (done)
   - `target-architecture.md` from `docs/target-architecture/README.md`. (done)
   - `decisions/` from `docs/decisions/`. (done)
4. Add `docs/src/download.md` describing source-only build. (done)
5. Add `docs/src/site-operations.md` describing local preview, CI, Pages setup,
   and troubleshooting. (done)
6. Create `.github/workflows/deploy-site.yml`:
   - Triggers on push to default branch and `workflow_dispatch`. (done)
   - Installs Rust stable and caches Cargo artifacts. (done)
   - Runs `cargo test --workspace` before building. (done)
   - Runs `mdbook build docs`. (done)
   - Publishes `docs/book/` via `configure-pages`, `upload-pages-artifact`, and
     `deploy-pages`. (done)
7. Update `README.md` with site preview and deployment instructions. (done)
8. Update `CHANGE.md` with site and workflow additions. (done)

### Acceptance

- `mdbook build docs` succeeds locally after `cargo install mdbook`.
- Push to the default branch triggers the workflow and publishes the site.
- `cargo test --workspace` is a required step before site publication.
- `docs/book/` is excluded from version control.

## Phase 7B: Download Page And Release Automation

Status: completed.

### Goals

- Publish pre-built binaries for all major platforms via GitHub Releases.
- Make the download page useful before binaries exist by providing links and
  clear source-build fallback instructions.

### Tasks

1. Create `.github/workflows/release.yml` triggered on `v*` tag push. (done)
   - Matrix: Linux x86_64, Windows x86_64, macOS Intel, macOS Apple Silicon.
   - Each job builds `unio` and `unio-daemon`, packages into `.tar.gz` / `.zip`.
   - Final `release` job attaches all archives to a GitHub Release.
2. Update `docs/src/download.md` with a per-platform binary download table. (done)
   - Links point to `https://github.com/way2ai/unio/releases/latest/download/`.
   - Source build section retained as fallback.

### Acceptance

- Pushing `git tag v0.1.0 && git push origin v0.1.0` triggers the workflow.
- GitHub Release is created with 4 platform archives attached.
- Download page table links resolve to the correct archive names.

## Phase 7C: Chinese/English Multilingual Site

Status: completed.

### Goals

- Support Chinese (Simplified) as a second language on the documentation site.
- Default language remains English; Chinese is accessible via a language switcher.
- Keep a single source-of-truth `docs/src/` rather than duplicating pages.

### Tasks

1. Add `[preprocessor.gettext]` to `docs/book.toml`. (done)
2. Create `docs/po/zh-CN.po` with full Chinese translations for all pages. (done)
3. Create `docs/theme/head.hbs` with EN / 中文 language switcher. (done)
   - Active language highlighted; switcher is position-fixed in the top-right.
4. Update `.github/workflows/deploy-site.yml`: (done)
   - Install `mdbook-i18n-helpers` alongside `mdbook`.
   - Add Chinese build step: output to `docs/book/zh-CN/`.
   - Chinese subdirectory is included in the same Pages artifact.

### Acceptance

- `mdbook build docs` produces English site at `docs/book/`.
- `MDBOOK_BOOK__LANGUAGE=zh-CN mdbook build docs -d book/zh-CN` produces Chinese
  site at `docs/book/zh-CN/`.
- Language switcher appears on every page and navigates between EN and 中文.
- CI publishes both language builds to GitHub Pages.



Run these after each meaningful slice:

```powershell
cargo fmt --all
cargo test --workspace
```

For daemon-level checks:

```powershell
$env:CARGO_TARGET_DIR = "target-check"
cargo build --workspace
.\target-check\debug\unio-daemon.exe 127.0.0.1:7882
```

Then in another shell:

```powershell
.\target-check\debug\unio.exe status
.\target-check\debug\unio.exe exec "mock-tool read path=README.md"
.\target-check\debug\unio.exe exec "mock-tool skill-tool name=repo,request=inspect-modules"
.\target-check\debug\unio.exe trace <latest_trace_id>
```

Expected trace events for skill-tool:

- `skill.started`
- `tool.completed`
- `skill.completed`

Manual hybrid checks:

```powershell
.\target-check\debug\unio.exe
```

Inside the hybrid surface:

- Submit `hello` and verify assistant output plus timeline appear.
- Run `/skills`, `/model`, `/update`, `/approval`, `/resume`.
- Type `/re` and verify slash command suggestions narrow and Enter completes.
- Type `@README` and verify file suggestions appear and Enter inserts a
  highlighted reference.
- Verify Left/Right, Home/End, Ctrl+W, Ctrl+U, and Shift+Enter editing.
- Trigger a write approval, run `/approval`, then approve with `a` or deny with
  `d` while the input is empty.
