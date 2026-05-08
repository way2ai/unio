# Unio Documentation

These documents describe the new Unio system after the from-zero refactor. Old
`engine/*` code, old sessions, old traces, and old `.skills` layouts are not
compatibility constraints.

Recommended reading order:

- [Get Started](./get-started.md): build, run, use the hybrid CLI, tools,
  approvals, skills, resume, trace, and tests.
- [Architecture](./architecture/README.md): runtime layers, main execution
  flow, crate boundaries, and current architecture contracts.
- [Hybrid File References](./architecture/hybrid-file-references.md):
  interactive `@path` suggestions and workspace indexing.
- [Hybrid Slash Commands](./architecture/hybrid-slash-commands.md):
  interactive `/` command suggestions and completion behavior.
- [Hybrid Input Editing](./architecture/hybrid-input-editing.md): cursor
  movement, delete shortcuts, and multiline prompt behavior.
- [Target Architecture](./target-architecture/README.md): long-term agent,
  model, tools, security, storage, observability, and UX goals.
- [Decisions](./decisions/): architecture decision records.
- [Current State](./current-state/README.md): how the old system is treated as
  reference only.

Keep docs and implementation synchronized. Update `PLAN.md`, `README.md`,
`CHANGE.md`, and the relevant `docs/` file when behavior changes.
