# Plan: Wire the migration pathway into the rust-bucket CLI

- **Slug:** `wire-migration-path`
- **Branch:** `danver/wire-migration-path`
- **Status:** Requirements draft (implementation plan intentionally deferred)
- **Date:** 2026-06-24

> Scope note: this document covers **Overview** and **Requirements / Expected Behavior** only. The implementation plan, phases, and testing strategy are deliberately out of scope and will be added in a later pass.

---

## Overview

- The migration machinery already exists but is **dead code**: the embedded `migrations/*.md` guides, `migrations::migrations_between`, and `upgrade::run_upgrade` are all implemented and unit-tested, yet nothing is reachable from the CLI.
- Today there is no migration subcommand, `main.rs` never surfaces migrations, the wired `apply` update path bumps the version stamp while ignoring migrations entirely, and `migration.instructions` is never displayed anywhere.
- **Goal:** surface migration guidance whenever a managed repo moves *forward* in version, so a human — or an LLM agent — can act on it.
- **Two entry points:** automatically during `apply`, and on demand via a new `rust-bucket show-migration` command.
- **Display-only:** print the relevant guides verbatim and automate nothing. Output is meant to be read or piped to an LLM agent that performs the steps.
- **Forward-only becomes an enforced product principle:** rust-bucket only moves forward; downgrades are unsupported. This is enforced in `apply` and documented in a new `DESIGN.md`.
- **Consistency over branching:** prefer one unified output path (e.g. `apply` always prints a migration footer) over special-casing empty results.

### Non-goals (this feature)

- No automated application of migration steps — guidance is informational text only.
- No structured/JSON output mode — markdown only.
- No implementation decisions here (e.g. whether the new command reuses `run_upgrade`, and how it relates to `apply`'s regeneration path) — deferred to the implementation pass.
- Bringing `DESIGN.md` fully up to date with all design decisions is explicitly deferred to a later effort.

---

## Requirements / Expected Behavior

### `show-migration` command (on-demand)

- Add a new `rust-bucket show-migration` subcommand.
- Takes optional `--from` / `--to`:
  - `--from` defaults to the version recorded in `rust-bucket.toml`.
  - `--to` defaults to the running binary version.
  - So a bare `show-migration` answers "what migrations are pending for this repo?".
- With **both** flags supplied, it works **anywhere** — no repo or `rust-bucket.toml` needed (a pure version-range query).
- A **bare** invocation (flags omitted) **outside** an initialized repo (no `rust-bucket.toml`) is an **error** (exit non-zero) — there is no `--from` to resolve.
- Versions must be full `X.Y.Z` semver. Partial or prefixed input (`1`, `v5`, `0.9`) is an unparseable-version **error**.
- Range semantics are `(from, to]` — exclusive of `from`, inclusive of `to`. A multi-version jump (e.g. moving across several releases) shows **every intermediate** guide in range.
- Output is **display-only**: the matching guides are printed verbatim as concatenated raw markdown, each under a version header, to **stdout**.
- The command only displays — it does **not** regenerate managed files or run verification.

### `apply` integration (automatic)

- `apply` **always** regenerates managed files and runs verification, regardless of any pending migrations.
- Migrations are **purely informational**: surfacing them **never changes the exit code**, which continues to reflect verification results only.
- **Every** `apply` run prints a migration footer **after** the verification summary — one unified path, no special-casing:
  - Forward bump with matching guides → the full guide markdown.
  - Otherwise (no-op, first-time init, or a forward bump with no matching guide files) → a "No upgrade instructions" message.
  - The footer ends with a re-view hint pointing at `show-migration --from <old-version>`.
- The `rust-bucket.toml` version stamp **always advances** on a forward `apply`. Migrations are therefore shown **once** at the moment of the bump; `show-migration` is the mechanism to re-view them afterward.

### Forward-only enforcement

- rust-bucket only moves **forward**.
- When the running binary is **older** than the version recorded in `rust-bucket.toml`, `apply` **refuses**: it errors out and exits non-zero **before touching anything** — it never lowers the stamp and never regenerates files.

### Empty results vs. errors — exit code & stream contract

- An **empty result is a success** (exit 0) accompanied by an explicit **"No upgrade instructions"** message on **stdout**. This covers:
  - the repo is already current,
  - a no-op or first-time-init `apply`,
  - `--from == --to`,
  - no matching guide files in the resolved range.
- Both the guide markdown and the "No upgrade instructions" result are written to **stdout** (the "No upgrade instructions" line is a normal result, not a diagnostic).
- **stderr** is reserved for true **errors**, which all exit non-zero:
  - `--from > --to` (a strictly backwards window),
  - an unparseable / non-`X.Y.Z` version,
  - a bare `show-migration` outside an initialized repo,
  - the forward-only refusal in `apply` (binary older than the recorded version).
- Rule of thumb: **never silent** — if a migration lookup happens and finds nothing, say so on stdout; only genuine misuse goes to stderr and fails.

### Documentation (`DESIGN.md`)

- Create a new `DESIGN.md` at the repo root stating the **forward-only** design principle.
- `DESIGN.md` is currently listed as required reading in `AGENTS.md` / `CLAUDE.md` but **does not exist** — this feature creates it.
- This documentation change is folded into the feature's work and tracked via a bead (per the repo's coordinator policy).
- **Out of scope:** populating `DESIGN.md` with the full set of design decisions — deferred to a later effort.

---

## Open Questions

- **First-time-init re-view hint:** on a first-time `init` there is no prior version, so the `show-migration --from <old-version>` hint has no meaningful `<old-version>`. Decide whether to omit the hint on init or point it at the current version. (Implementation-level; can be settled during the build.)
- **Implementation approach (deferred by design):** how the new `show-migration` command, the existing `upgrade::run_upgrade`, and `apply`'s regeneration path relate — including whether `run_upgrade` is reused, extended, or retired — is intentionally left to the implementation-planning pass.
