# Rust Bucket: A Rusty bucket to carry your slop

Rust-Bucket is a Rust-first project bootstrapper for AI-first engineering agentic coding workflows. It prepares an
**already-initialized** Rust repo (you have already run `git init` and `cargo init`) by installing a standard set of
tooling files, documentation, and conventions.

Rust-Bucket is designed to be:
- **Cross-platform**: Linux, macOS, and Windows.
- **Deterministic**: templates are **embedded in the Rust-Bucket binary**.
- **Strict**: agents get rigid structure and automated checks.

## What it does

### `rust-bucket apply`
Detects if this is being run for the first time by the presense of `rust-bucket.toml`

If the first time, sets up the current crate.

- Prompts you for a small set of decisions (interactive by default).
- Writes `rust-bucket.toml` into the current repo to persist those decisions.
- Generates the managed file set (docs, lint configs, testing configs, devcontainer stubs).
- **Refuses to run if any managed file already exists.**

If we have run this before, it updates the managed file set.

- Loads and preserves **all choices** from `rust-bucket.toml`.
- Prompts for any new questions, if necessary.
- Re-renders templates from the embedded template pack.
- **Overwrites all managed files** with the current template versions.
- Does not attempt to diff or dry-run in v1.

> Rust-Bucket never edits a target repo’s `README.md` or `ARCHITECTURE.md`

## Managed file set (v1)
The initial "managed" set is expected to include:
- `AGENTS.md`
- `CLAUDE.md`
- `TESTING.md`
- `.claude/agents/*.md` (coordinator, coding, judge, tidy, reflection)
- `.config/nextest.toml`
- `deny.toml` (if enabled)
- `rustfmt.toml` / clippy configuration (as needed)
- devcontainer stubs for Sculptor (placeholders acceptable)

Managed files are **overwritten on every apply**, so they should not be edited by hand.

## Seed file set
Seed files are written **only if absent** and are **never overwritten on re-apply**. After the first apply the project owns them and may customize them freely. The current seed files are:
- `ratchets.toml` — initial `imbue-ai/ratchets` config so the `ratchets check` gate has something to read.
- `STYLE_GUIDE.md` — starting project style guide.

## Template engine (v1)
Rust-Bucket uses the Liquid template language (via `cargo-generate` as a library) to render embedded templates into the target repo.

No hooks in v1.

## Self-hosting
Rust-Bucket is intended to be able to `rust-bucket apply` its own repository without special cases.
See `ARCHITECTURE.md`.
