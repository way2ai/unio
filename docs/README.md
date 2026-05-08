# Unio Documentation

This directory contains the mdBook source for the Unio project site.

## Books

- English: `docs/src/`
- Simplified Chinese: `docs/zh/src/`
- Shared theme customizations: `docs/theme/`

## Local Preview

```powershell
cargo install mdbook
mdbook serve docs
mdbook serve docs/zh
```

## Local Build

```powershell
mdbook build docs
mdbook build docs/zh
```

English output is written to `docs/book/`. The Chinese build writes to
`docs/book/zh-CN/` so both languages can be published as one GitHub Pages
artifact.

## Documentation Policy

Docs are the product and contributor reference for Unio. Keep them synchronized
with behavior, commands, release targets, security policy, storage contracts,
and agent workflows.
