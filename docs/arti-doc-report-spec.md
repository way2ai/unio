# Arti Doc Project Report Specification

## Purpose

`arti_doc/` contains a bilingual technical review report for Unio. The report is
generated from the current repository state, existing mdBook documentation, and
the Rust workspace entry points.

## Audience

The primary audience is technical reviewers who need to understand what Unio is,
how the runtime is organized, which modules own which responsibilities, and how
to run or inspect the system locally.

## Output Structure

The report is organized as chapter-level bilingual Markdown:

- `arti_doc/README.md`: language index and chapter map.
- `arti_doc/zh/`: Simplified Chinese report chapters.
- `arti_doc/en/`: English report chapters.

Each language contains the same chapter set:

1. Overview
2. Technical Architecture
3. Functional Modules
4. Usage Guide
5. Technical Introduction

## Source Of Truth

The report should stay aligned with:

- `README.md`
- `PLAN.md`
- `docs/src/*.md`
- `Cargo.toml`
- `apps/*/src/lib.rs`
- `crates/*/src/lib.rs`

When Unio changes behavior, architecture, commands, storage, tools, approvals,
skills, or model configuration, update the relevant report chapters together
with the main documentation set.

## Scope

The report documents implemented behavior and near-term technical context. It
does not define a new product roadmap, introduce new APIs, or replace the
mdBook documentation.
