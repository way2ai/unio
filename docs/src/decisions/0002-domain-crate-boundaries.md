# ADR 0002: 领域 crate 边界

## Status

Accepted

## Decision

新系统使用领域 crate，而不是按旧 engine 分组：

- `core` 放 ID、路径、时间等基础设施。
- `protocol` 放跨进程和跨模块协议类型。
- `agent` 放 root agent、planner、sub-agent 和 agent loop。
- `model` 放 provider 抽象。
- `tools` 放工具 registry 和工具 I/O。
- `security` 放权限策略、审批和 sandbox 决策。
- `skills` 放 skill discovery 与 tool 注入。
- `observability` 放 trace、token、统计事件。
- `storage` 放 SQLite 和 JSONL 持久化。

## Consequences

模块依赖方向必须保持单向：上层应用依赖领域 crate，领域 crate 之间通过协议和接口通信，不能让 CLI/TUI 直接依赖具体模型或工具执行细节。
