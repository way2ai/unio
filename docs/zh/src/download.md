# 下载

## 预构建二进制

下载适合您平台的最新版本：

| 平台 | 架构 | 归档包 |
|---|---|---|
| Linux | x86\_64 | [unio-latest-x86\_64-unknown-linux-gnu.tar.gz](https://github.com/way2ai/unio/releases/latest/download/unio-latest-x86_64-unknown-linux-gnu.tar.gz) |
| Windows | x86\_64 | [unio-latest-x86\_64-pc-windows-msvc.zip](https://github.com/way2ai/unio/releases/latest/download/unio-latest-x86_64-pc-windows-msvc.zip) |
| macOS（Intel）| x86\_64 | [unio-latest-x86\_64-apple-darwin.tar.gz](https://github.com/way2ai/unio/releases/latest/download/unio-latest-x86_64-apple-darwin.tar.gz) |
| macOS（Apple Silicon）| aarch64 | [unio-latest-aarch64-apple-darwin.tar.gz](https://github.com/way2ai/unio/releases/latest/download/unio-latest-aarch64-apple-darwin.tar.gz) |

每个归档包含两个二进制文件：

- `unio` — 用户侧 CLI 和混合界面
- `unio-daemon` — 本地 daemon 运行时

访问 [Releases 页面](https://github.com/way2ai/unio/releases) 查看所有版本和更新日志。

> **注意：** 当版本标签（`v*`）被推送到仓库时，将发布预构建二进制文件。若暂无发布版本，请按下方说明从源码构建。

## 环境要求

- Rust stable 工具链（从 [rustup.rs](https://rustup.rs) 安装）
- Cargo（随 Rust 一起提供）
- Windows、macOS 或 Linux

## 从源码构建

```sh
git clone https://github.com/way2ai/unio.git
cd unio
cargo build --workspace --release
```

编译后的二进制文件位于 `target/release/` 目录下：

- `unio` — 用户侧 CLI 和混合界面
- `unio-daemon` — 本地 daemon 运行时

## 无需安装直接运行

开发期间，在仓库根目录使用 `cargo run`：

```powershell
cargo run -p unio
cargo run -p unio -- status
cargo run -p unio -- exec "hello"
```

## 运行测试

```powershell
cargo fmt --all
cargo test --workspace
```
