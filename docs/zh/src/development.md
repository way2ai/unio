# 开发

## 命令

在仓库根目录运行：

```powershell
cargo fmt --all
cargo test --workspace
cargo build --workspace
```

运行 CLI：

```powershell
cargo run -p unio -- status
cargo run -p unio -- exec "hello"
```

运行直接工具命令：

```powershell
cargo run -p unio -- tool read --args path=README.md
```

## 仓库约定

- 文档和实现保持同步。
- 行为、架构、工作流或文档有意义变化时更新 `CHANGE.md`。
- 设置、用法或用户可见行为变化时更新 `README.md`。
- 架构、协议、安全、存储、工具或智能体行为变化时更新 `docs/`。

## 测试

测试与实现放在各 crate 内的 `#[cfg(test)]` 模块中。优先在行为所属代码旁添加聚焦的单元测试。
