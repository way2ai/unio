# Release

Release artifacts are built by `.github/workflows/release.yml` when a tag that
matches `v*` is pushed.

## Targets

| Platform | Target | Archive |
|---|---|---|
| Linux | `x86_64-unknown-linux-gnu` | `.tar.gz` |
| Windows | `x86_64-pc-windows-msvc` | `.zip` |
| macOS | `aarch64-apple-darwin` | `.tar.gz` |

The release workflow builds both `unio` and `unio-daemon`, packages them, and
attaches the archives to a GitHub Release.

## Create A Release

```powershell
git tag v0.1.0
git push origin v0.1.0
```

## Local Release Build

```powershell
cargo build -p unio -p unio-daemon --release
```
