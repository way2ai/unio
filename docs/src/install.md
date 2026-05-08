# Install

## Requirements

- Rust stable toolchain
- Cargo
- Windows, macOS, or Linux

Check the local toolchain:

```powershell
rustc --version
cargo --version
```

## Releases

Pre-built archives are published from version tags.

| Platform | Architecture | Archive |
|---|---|---|
| Linux | x86_64 | `unio-<version>-x86_64-unknown-linux-gnu.tar.gz` |
| Windows | x86_64 | `unio-<version>-x86_64-pc-windows-msvc.zip` |
| macOS | Apple Silicon | `unio-<version>-aarch64-apple-darwin.tar.gz` |

Each archive contains:

- `unio`: user-facing CLI.
- `unio-daemon`: local runtime process.

See the project releases at
`https://github.com/way2ai/unio/releases`.

## Build From Source

```powershell
git clone https://github.com/way2ai/unio.git
cd unio
cargo build --workspace --release
```

Compiled binaries are written to `target/release/`.

## Run From Source

During development, run commands through Cargo:

```powershell
cargo run -p unio -- status
cargo run -p unio -- exec "hello unio"
```
