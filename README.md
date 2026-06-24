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
- **Refuses to run if any managed file already exists** (override with `--force`).

If we have run this before, it updates the managed file set.

- Loads and preserves **all choices** from `rust-bucket.toml`.
- Prompts for any new questions, if necessary.
- Re-renders templates from the embedded template pack.
- **Overwrites all managed files** with the current template versions.
- Does not attempt to diff or dry-run in v1.

After verification, `apply` prints a migration footer: the embedded guides for the version range it just moved across (or `No upgrade instructions`), plus a hint to re-view them via `rust-bucket show-migration --from <old-version>`.

`apply` is **forward-only**: it refuses (errors, non-zero exit, **no changes**) when the running binary is older than the version recorded in `rust-bucket.toml`.

> Rust-Bucket never edits a target repo’s `README.md` or `ARCHITECTURE.md`

### `rust-bucket show-migration`
Prints embedded migration guides for a version range. Display-only — it performs no file generation.

- `--from` defaults to the version recorded in `rust-bucket.toml`; `--to` defaults to the running binary version.
- With both flags it works anywhere (no initialized repo required); a bare invocation outside an initialized repo is an error.
- Versions must be full `X.Y.Z` semver.
- Guide text and a `No upgrade instructions` message print to stdout (exit 0).
- Errors (`--from` greater than `--to`, an unparseable version, or not-initialized) go to stderr with a non-zero exit.

## Managed file set
The authoritative list of managed files is defined in code, as `managed_files()` in `src/templates.rs` — consult it there rather than maintaining a copy here that drifts. Each entry maps to a template under `templates/`.

Managed files are **overwritten on every apply**, so they must not be edited by hand; edit the corresponding template under `templates/` instead.

## Seed file set
Seed files are written **only if absent** and are **never overwritten on re-apply**, so the project can customize them freely. The authoritative list is defined in code, as `seed_files()` in `src/templates.rs`. It currently seeds:
- `ratchets.toml` — initial `imbue-ai/ratchets` config so the `ratchets check` gate has something to read.
- `STYLE_GUIDE.md` — starting project style guide.

## Template engine (v1)
Rust-Bucket uses the Liquid template language, via the `liquid` crate used as a library, to render embedded templates into the target repo.

No hooks in v1.

## Self-hosting
Rust-Bucket is intended to be able to `rust-bucket apply` its own repository without special cases.
See `ARCHITECTURE.md`.
