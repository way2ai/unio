# 工具与审批

工具是 daemon 可执行的已注册能力。执行前会经过安全预检查。审批层把普通读取与会修改文件或运行进程的操作分开。

## 读取工具

```powershell
cargo run -p unio -- tool read --args path=README.md
```

## 写入工具

默认策略下，写入需要审批：

```powershell
cargo run -p unio -- tool write --args path=notes.txt,content=hello
```

查看并处理审批：

```powershell
cargo run -p unio -- approvals
cargo run -p unio -- approvals approve approval_xxx
cargo run -p unio -- approvals deny approval_xxx
```

## Full Trust 模式

只有在明确允许该操作时才使用 `full-trust`：

```powershell
cargo run -p unio -- tool write --approval full-trust --args path=notes.txt,content=hello
```

## 策略合同

- `allow`：工具可以立即运行。
- `deny`：工具被阻止。
- `approval-required`：工具等待用户显式决策。

工具实现应保持小而清晰，风险决策交给 `crates/security`。
