# Unio 文档

本文档描述从零重构后的 Unio 系统。

推荐阅读顺序：

- [快速开始](get-started.md)：构建、运行、使用混合 CLI、工具、审批、技能、恢复、追踪和测试。
- [架构](architecture.md)：运行时层次、主执行流、crate 边界和当前架构契约。
- [混合文件引用](hybrid-file-references.md)：交互式 `@path` 建议与工作区索引。
- [混合斜杠命令](hybrid-slash-commands.md)：交互式 `/` 命令建议与补全行为。
- [混合输入编辑](hybrid-input-editing.md)：光标移动、删除快捷键和多行提示行为。
- [目标架构](target-architecture.md)：长期的 agent、模型、工具、安全、存储、可观测性和 UX 目标。
- [下载](download.md)：如何获取和构建 Unio。
- [站点运维](site-operations.md)：本地预览、CI 和 GitHub Pages。
- [决策记录](decisions/0001-start-from-zero.md)：架构决策记录。

保持文档与实现同步。行为变更时更新 `PLAN.md`、`README.md`、`CHANGE.md` 以及相关的 `docs/` 文件。
