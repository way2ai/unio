# 当前状态

本轮从 0 开始重构，旧实现不再作为兼容约束。

旧目录仍保留在仓库中，作用是：

- 提供已有需求和交互行为参考。
- 帮助验证哪些能力已经被探索过。
- 作为后续人工对照材料。

旧实现不决定：

- 新 crate 名称和边界。
- 新协议 schema。
- 新 transcript、trace、session 存储格式。
- 新 skills 路径。
- 新权限策略枚举。

新实现以 `docs/architecture/README.md` 和 `docs/target-architecture/README.md` 为准。
