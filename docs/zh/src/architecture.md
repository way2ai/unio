# 架构

Unio 是围绕本地 daemon 组织的 Rust workspace。CLI 保持轻量：收集用户输入并把工作发送给 daemon。daemon 负责状态、会话和运行生命周期、审批、工具执行、存储与追踪事件。

## Workspace 布局

```text
apps/
  cli/       用户使用的 unio 命令
  daemon/    会话、运行、工具、存储和 trace 的本地运行时

crates/
  core/           ID、路径、元数据和共享工具
  protocol/       CLI、daemon 和 agent 的协议类型
  agent/          root agent、planner、sub-agent 和 skill-agent 合同
  model/          模型提供方抽象与 mock provider
  tools/          工具注册表与执行合同
  security/       审批策略与风险预检查
  skills/         技能发现与 skill-tool 执行
  storage/        SQLite 元数据与 JSONL transcript 存储
  observability/  trace 与上下文事件
```

## 运行流程

```text
user
  -> unio CLI
  -> daemon
  -> agent
  -> model provider or tool
  -> security approval when needed
  -> storage and trace records
  -> user-visible result
```

## 当前合同

- `apps/cli` 负责用户交互和命令解析。
- `apps/daemon` 负责运行时编排和持久化。
- `crates/protocol` 定义共享请求和响应类型。
- `crates/security` 返回允许、拒绝或需要审批的决策。
- `crates/tools` 只在预检查后执行已注册工具。
- `crates/storage` 持久化元数据、transcript 和 trace 数据。
- `crates/observability` 记录结构化运行事件。

## 设计原则

- 不把模型调用、工具执行和持久化放进 CLI。
- 所有有风险的操作都经过安全策略。
- 在 model crate 中从 `~/.unio/config.toml` 解析模型配置，并让环境变量作为更高优先级覆盖。
- 使用 mock provider 保证本地开发可运行。
- 为 session、run、agent 和 trace 使用显式 ID。
