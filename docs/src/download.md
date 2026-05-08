# Download

## Pre-built Binaries

Download the latest release for your platform:

| Platform | Architecture | Archive |
|---|---|---|
| Linux | x86\_64 | [unio-latest-x86\_64-unknown-linux-gnu.tar.gz](https://github.com/way2ai/unio/releases/latest/download/unio-latest-x86_64-unknown-linux-gnu.tar.gz) |
| Windows | x86\_64 | [unio-latest-x86\_64-pc-windows-msvc.zip](https://github.com/way2ai/unio/releases/latest/download/unio-latest-x86_64-pc-windows-msvc.zip) |
| macOS (Intel) | x86\_64 | [unio-latest-x86\_64-apple-darwin.tar.gz](https://github.com/way2ai/unio/releases/latest/download/unio-latest-x86_64-apple-darwin.tar.gz) |
| macOS (Apple Silicon) | aarch64 | [unio-latest-aarch64-apple-darwin.tar.gz](https://github.com/way2ai/unio/releases/latest/download/unio-latest-aarch64-apple-darwin.tar.gz) |

Each archive contains two binaries:

- `unio` — user-facing CLI and hybrid surface
- `unio-daemon` — local daemon runtime

For all releases and changelogs visit the
[Releases page](https://github.com/way2ai/unio/releases).

> **Note:** Pre-built binaries are published when a version tag (`v*`) is
> pushed to the repository. If no release exists yet, build from source using
> the instructions below.

## Requirements

- Rust stable toolchain (install from [rustup.rs](https://rustup.rs))
- Cargo (included with Rust)
- Windows, macOS, or Linux

## Build From Source

```sh
git clone https://github.com/way2ai/unio.git
cd unio
cargo build --workspace --release
```

The compiled binaries are placed under `target/release/`:

- `unio` — user-facing CLI and hybrid surface
- `unio-daemon` — local daemon runtime

## Run Without Installing

During development, use `cargo run` from the repository root:

```powershell
cargo run -p unio
cargo run -p unio -- status
cargo run -p unio -- exec "hello"
```

## Run Tests

```powershell
cargo fmt --all
cargo test --workspace
```
