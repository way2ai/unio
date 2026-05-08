# 站点运维

项目站点是发布到 GitHub Pages 的 mdBook。英文内容位于 `docs/src/`，简体中文内容位于 `docs/zh/src/`。

## 本地预览

安装 mdBook：

```powershell
cargo install mdbook
```

预览英文站点：

```powershell
mdbook serve docs
```

预览中文站点：

```powershell
mdbook serve docs/zh
```

## 本地构建

```powershell
mdbook build docs
mdbook build docs/zh
```

英文输出写入 `docs/book/`。中文输出写入 `docs/book/zh-CN/`。

## GitHub Pages

`.github/workflows/deploy-site.yml` 会在推送到 `main` 和手动触发时运行。它会：

1. 检出仓库。
2. 设置 Rust stable。
3. 运行 `cargo test --workspace`。
4. 安装 mdBook。
5. 构建英文和中文 mdBook。
6. 上传 `docs/book/` 作为 Pages artifact。
7. 通过 GitHub Pages 部署 artifact。

仓库 Pages 来源应设置为 **GitHub Actions**。
