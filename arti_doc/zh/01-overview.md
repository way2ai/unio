# 01 项目概览

## 定位

Unio 是一个基于 Rust 的本地智能体运行时，面向开发者终端工作流。它把 CLI、本地 daemon、模型提供方抽象、工具执行、审批策略、技能、存储和观测事件整合在同一个 workspace 中。

项目当前更接近“可运行的本地 agent runtime 原型和基础平台”，而不是单一脚本工具。用户通过 `unio` 命令发起任务，CLI 将会话、执行、工具和审批请求交给本地 daemon，daemon 负责状态、持久化和安全边界。

## 当前能力

- 通过 `unio exec` 创建或恢复 workspace 关联会话。
- 通过 TUI 或直接命令提交 prompt。
- 支持 `/model` 配置持久模型设置，并允许环境变量覆盖配置文件。
- 支持 mock provider，便于无真实模型凭据时本地开发和测试。
- 支持 OpenAI-compatible 与 Anthropic 风格的模型 provider。
- 支持内置工具：文件搜索、读取、写入、编辑、受限命令执行、可信 URL fetch、计划生成、技能调用。
- 工具执行进入 `crates/security` 决策，可能直接允许、拒绝或进入审批队列。
- 支持审批列表、审批通过、审批拒绝和审批历史记录。
- 支持从 workspace 或用户目录发现 `.unio/skills/<name>/SKILL.md` 技能。
- 使用 SQLite 保存 session、run 和审批授权记录。
- 使用 JSONL 保存 transcript 和 trace 事件。
- 提供英文和简体中文 mdBook 文档站。

## 主要使用场景

- 在一个仓库中启动本地智能体会话，持续追踪 prompt、回答、工具调用和 trace。
- 以受控方式让模型请求读取、写入、编辑文件或运行命令。
- 在默认模式下对有副作用的工具调用进行人工审批。
- 用 mock provider 稳定测试 agent、工具、审批、存储和 trace 行为。
- 用技能机制把仓库级或用户级工作流暴露给 agent。

## 项目形态

Unio 是一个 Rust workspace，顶层包含两个应用和九个功能 crate：

- `apps/cli`: 用户入口命令。
- `apps/daemon`: 本地运行时和 HTTP 服务。
- `crates/core`: ID、路径和通用结构。
- `crates/protocol`: CLI、daemon、agent 之间共享的协议类型。
- `crates/agent`: root agent、计划、sub-agent 和 skill-agent 契约。
- `crates/model`: 模型 provider 抽象和配置解析。
- `crates/tools`: 工具注册表和工具执行。
- `crates/security`: 权限模式和审批策略。
- `crates/skills`: 技能发现和技能执行。
- `crates/storage`: SQLite 和 JSONL 持久化。
- `crates/observability`: trace 和上下文预算事件。

## 现状判断

当前实现已经具备完整的本地 runtime 闭环：CLI 可以连接 daemon，daemon 可以管理 session/run，agent 可以调用模型或工具，工具执行受安全策略约束，执行结果可以进入 transcript 和 trace。近期待加强方向主要是更完整的测试覆盖、模型错误报告、密钥处理、公开安装说明和更多生产级运行细节。
