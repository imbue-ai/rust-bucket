# Design

Detailed design decisions and known design gaps for Rust-Bucket. This file is
project-owned (not generated): Rust-Bucket never overwrites it. Agents read it
when present (see the canonical reading list in AGENTS.md "Hard requirements").

## Future improvements

### Persisting Reflection-agent edits back into the template pack

The Reflection Agent (`.claude/agents/reflection.md`) is permitted to edit the
`.claude/agents/*.md` files by hand so that workflow improvements take effect
**immediately** for the running agent fleet. That immediacy is intentional and
desirable.

The gap: those `.claude/agents/*.md` files are **managed files** (see
`managed_files()` in `src/templates.rs`) and are regenerated from
`templates/.claude/agents/*.md.liquid` on every `rust-bucket apply`. A
hand-edit applied directly to a generated file is therefore silently lost the
next time the templates are re-applied. We have no mechanism to fold a
Reflection improvement back into the corresponding `templates/*.liquid` source,
so an improvement is either ephemeral (in a downstream project) or has to be
manually mirrored into the template (in this self-hosting repo).

Possible directions (not yet decided):
- Have the Reflection Agent edit the `templates/*.liquid` source and re-render,
  so the change is both immediate and durable. Only works where the template
  pack is available in the working tree (i.e. self-hosting).
- A "fold-in" step that diffs a generated file against its rendered template
  and back-ports the delta into the `.liquid` source.
- Distinguish a project-local override layer from the managed template layer so
  downstream projects can persist their own agent-instruction tweaks across
  `apply` runs.

Until one of these lands, Reflection edits to generated agent files in this
repo must be mirrored by hand into the matching `templates/*.liquid` file to
survive the next apply.
