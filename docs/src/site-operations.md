# Site Operations

The project site is an mdBook published to GitHub Pages. English content lives
under `docs/src/`; Simplified Chinese content lives under `docs/zh/src/`.

## Local Preview

Install mdBook:

```powershell
cargo install mdbook
```

Serve the English site:

```powershell
mdbook serve docs
```

Serve the Chinese site:

```powershell
mdbook serve docs/zh
```

## Local Build

```powershell
mdbook build docs
mdbook build docs/zh
```

English output is written to `docs/book/`. Chinese output is written to
`docs/book/zh-CN/`.

## GitHub Pages

The `.github/workflows/deploy-site.yml` workflow runs on pushes to `main` and
on manual dispatch. It:

1. Checks out the repository.
2. Sets up Rust stable.
3. Runs `cargo test --workspace`.
4. Installs mdBook.
5. Builds English and Chinese mdBooks.
6. Uploads `docs/book/` as the Pages artifact.
7. Deploys the artifact through GitHub Pages.

Repository Pages should use **GitHub Actions** as the source.
