# Unio

Unio is a Rust-based agent project being rebuilt around a clean daemon, agent,
tooling, security, skills, storage, and observability architecture.

Current status: this repository contains the daemon-backed hybrid CLI, session
runtime, mock/provider model path, tools/security flow, storage/trace support,
skills, approvals, resume/trace commands, and interactive `@`/`/` completions.
It is an active first usable refactor target, not a finished Codex/Claude Code
replacement.

Start here:

- [Get Started](./docs/get-started.md)
- [Architecture](./docs/architecture/README.md)
- [Hybrid Input Editing](./docs/architecture/hybrid-input-editing.md)
- [Hybrid Slash Commands](./docs/architecture/hybrid-slash-commands.md)
- [Target Architecture](./docs/target-architecture/README.md)

## Documentation Site

The `docs/` directory is the mdBook source for the project site, published to
GitHub Pages automatically on every push to the default branch.

Preview locally:

```sh
cargo install mdbook
mdbook serve docs
```

Build without serving:

```sh
mdbook build docs
```

Output is placed in `docs/book/` (excluded from version control). See
[docs/src/site-operations.md](./docs/src/site-operations.md) for CI and
deployment details.
