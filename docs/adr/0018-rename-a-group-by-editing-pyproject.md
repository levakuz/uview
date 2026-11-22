# 0018. Rename a dependency group by editing pyproject.toml

- **Status:** Accepted (2026-11-22)
- **Implemented by:** #120

## Context

uv has no command to rename a dependency group. uview renamed one with uv's own commands: a
`uv add --group new` for each package, a `uv remove --group old` for each, then deleting the
emptied group, each as a job of its own. The job queue runs every job whether or not the one
before it failed. With the index unreachable, every `uv add` failed and every `uv remove` then
ran: the packages, with their constraints, were in neither group. A package listed twice in
the group, or declared differently elsewhere, also lost its extras and markers on the way,
since the requirement was rebuilt from what the table shows.

## Decision

We will rename a group ourselves, in `controllers/groups.rs`: the key in `[dependency-groups]`
is replaced in place with `toml_edit`, so every requirement, comment and the group's position
stay as written, and `uv lock` follows. If uv cannot lock, the file is put back as it was. The
rename is refused when uv would refuse the name, a group has it already, or
`tool.uv.default-groups` or another group's `include-group` names the group.

## Consequences

- A rename is one job that changes one word of the file, or nothing.
- One more edit of pyproject.toml outside uv, beside ADR-0009, ADR-0010 and ADR-0013.
- The environment is not touched: its packages are the same under either name.
- A `[tool.uv.sources]` entry tied to the group by name is not followed; `uv lock` then fails
  and the file is put back.

## Alternatives considered

- **Keep uv's commands, in one job that stops at the first failure.** No package can end in
  neither group, but a failure part-way still leaves two half-groups, and extras and markers
  are still rebuilt rather than kept.
