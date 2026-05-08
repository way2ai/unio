# 发布

当推送匹配 `v*` 的标签时，`.github/workflows/release.yml` 会构建发布产物。

## 目标

| 平台 | Target | 归档 |
|---|---|---|
| Linux | `x86_64-unknown-linux-gnu` | `.tar.gz` |
| Windows | `x86_64-pc-windows-msvc` | `.zip` |
| macOS | `aarch64-apple-darwin` | `.tar.gz` |

发布工作流会构建 `unio` 和 `unio-daemon`，打包后附加到 GitHub Release。

## 创建发布

```powershell
git tag v0.1.0
git push origin v0.1.0
```

## 本地发布构建

```powershell
cargo build -p unio -p unio-daemon --release
```
