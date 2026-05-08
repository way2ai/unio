# 快速开始

所有命令都在仓库根目录运行。

## 验证工作区

```powershell
cargo fmt --all
cargo test --workspace
cargo build --workspace
```

## 查看状态

```powershell
cargo run -p unio -- status
```

## 运行提示词

```powershell
cargo run -p unio -- exec "hello unio"
```

如果没有配置模型凭据，Unio 会使用 mock provider，让本地开发保持可预测。

## 通过工具层读取文件

```powershell
cargo run -p unio -- tool read --args path=README.md
```

## 检查最近工作

```powershell
cargo run -p unio -- resume --limit 10
cargo run -p unio -- trace <trace_id> --run <run_id>
```

## 下一步

- 阅读 [CLI](cli.md) 了解命令参考。
- 在启用写入或进程执行前阅读 [工具与审批](tools-and-approvals.md)。
- 使用 [技能](skills.md) 添加仓库专属工作流。
