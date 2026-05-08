# Site Operations

This page describes how to build, preview, and deploy the Unio documentation
site locally and through GitHub Actions.

## Local Preview

Install mdBook:

```sh
cargo install mdbook
```

Build and serve the site locally:

```sh
mdbook serve docs
```

Open `http://localhost:3000` in a browser. The server reloads on file changes.

To only build without serving:

```sh
mdbook build docs
```

Output is placed in `docs/book/`. This directory is excluded from version
control via `.gitignore`.

## CI Workflow

The `.github/workflows/deploy-site.yml` workflow runs on every push to the
default branch and on `workflow_dispatch`.

Steps performed by the workflow:

1. Check out the repository.
2. Set up Rust stable.
3. Cache Cargo registry and build artifacts.
4. Run `cargo test --workspace` to verify the codebase before publishing.
5. Install `mdbook` via `cargo install`.
6. Run `mdbook build docs` to produce `docs/book/`.
7. Upload `docs/book/` as a GitHub Pages artifact.
8. Deploy to GitHub Pages using the official `deploy-pages` action.

## GitHub Pages Setup

Enable Pages for the repository:

1. Go to **Settings → Pages**.
2. Set **Source** to **GitHub Actions**.
3. Push to the default branch or trigger `workflow_dispatch` to publish.

## Build Artifacts

The workflow artifact is `github-pages` uploaded from `docs/book/`. The
`deploy-pages` action publishes it under the repository Pages URL.

## Troubleshooting

| Symptom | Check |
|---|---|
| Workflow fails on `cargo test` | Review test output in the Actions run log. |
| `mdbook build` fails | Check `docs/src/SUMMARY.md` for missing linked files. |
| Pages not updated after push | Confirm Pages source is set to GitHub Actions. |
| Local `mdbook serve` not found | Run `cargo install mdbook` again. |
