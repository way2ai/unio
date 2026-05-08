# 站点运维

本页说明如何在本地及通过 GitHub Actions 构建、预览和部署 Unio 文档站点。

## 本地预览

安装 mdBook：

```sh
cargo install mdbook
```

在本地构建并提供服务：

```sh
mdbook serve docs
```

在浏览器中打开 `http://localhost:3000`。服务器在文件变更时自动重载。

仅构建不启动服务：

```sh
mdbook build docs
```

输出位于 `docs/book/`，该目录已通过 `.gitignore` 排除在版本控制之外。

构建中文站：

```sh
mdbook build docs/zh
```

中文输出位于 `docs/book/zh-CN/`。

## CI 工作流

`.github/workflows/deploy-site.yml` 工作流在每次推送到默认分支或触发 `workflow_dispatch` 时运行。

工作流执行步骤：

1. 检出仓库。
2. 设置 Rust stable。
3. 缓存 Cargo 注册表和构建产物。
4. 运行 `cargo test --workspace` 验证代码库。
5. 安装 `mdbook`。
6. 运行 `mdbook build docs` 生成 `docs/book/`（英文）。
7. 运行 `mdbook build docs/zh` 生成 `docs/book/zh-CN/`（中文）。
8. 将 `docs/book/` 上传为 GitHub Pages artifact。
9. 通过 `deploy-pages` action 发布到 GitHub Pages。

## GitHub Pages 配置

为仓库启用 Pages：

1. 前往 **Settings → Pages**。
2. 将 **Source** 设为 **GitHub Actions**。
3. 推送到默认分支或触发 `workflow_dispatch` 进行发布。

## 构建产物

工作流 artifact `github-pages` 从 `docs/book/` 上传，`deploy-pages` action 将其发布到仓库的 Pages URL 下。

## 故障排查

| 现象 | 检查项 |
|---|---|
| 工作流在 `cargo test` 失败 | 查看 Actions 运行日志中的测试输出。 |
| `mdbook build` 失败 | 检查 `SUMMARY.md` 中是否有链接但缺失的文件。 |
| Pages 推送后未更新 | 确认 Pages source 已设为 GitHub Actions。 |
| 本地 `mdbook serve` 找不到命令 | 重新运行 `cargo install mdbook`。 |
