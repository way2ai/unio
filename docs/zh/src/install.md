# 安装

## 环境要求

- Rust stable toolchain
- Cargo
- Windows、macOS 或 Linux

检查本地工具链：

```powershell
rustc --version
cargo --version
```

## 发布包

版本标签会触发预构建归档发布。

| 平台 | 架构 | 归档 |
|---|---|---|
| Linux | x86_64 | `unio-<version>-x86_64-unknown-linux-gnu.tar.gz` |
| Windows | x86_64 | `unio-<version>-x86_64-pc-windows-msvc.zip` |
| macOS | Apple Silicon | `unio-<version>-aarch64-apple-darwin.tar.gz` |

每个归档包含：

- `unio`：用户使用的 CLI。
- `unio-daemon`：本地运行时进程。

发布页面：`https://github.com/way2ai/unio/releases`。

## 从源码构建

```powershell
git clone https://github.com/way2ai/unio.git
cd unio
cargo build --workspace --release
```

构建产物位于 `target/release/`。

## 从源码运行

开发时可以通过 Cargo 运行：

```powershell
cargo run -p unio -- status
cargo run -p unio -- exec "hello unio"
```
