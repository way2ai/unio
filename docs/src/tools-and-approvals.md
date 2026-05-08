# Tools And Approvals

Tools are registered capabilities that the daemon can execute after security
precheck. The approval layer separates ordinary reads from operations that can
change files or run processes.

## Read Tool

```powershell
cargo run -p unio -- tool read --args path=README.md
```

## Write Tool

In default policy, writes require approval:

```powershell
cargo run -p unio -- tool write --args path=notes.txt,content=hello
```

List and resolve approvals:

```powershell
cargo run -p unio -- approvals
cargo run -p unio -- approvals approve approval_xxx
cargo run -p unio -- approvals deny approval_xxx
```

## Full Trust Mode

Use `full-trust` only when you intentionally allow the requested operation:

```powershell
cargo run -p unio -- tool write --approval full-trust --args path=notes.txt,content=hello
```

## Policy Contract

- `allow`: the tool can run immediately.
- `deny`: the tool is blocked.
- `approval-required`: the tool waits for an explicit user decision.

Tool implementations should stay small and delegate risk decisions to
`crates/security`.
