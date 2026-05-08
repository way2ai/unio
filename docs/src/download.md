# Download

Unio is currently in active development. Pre-built binaries are not yet
published. Build from source using the instructions below.

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
