# 0009. Delete an empty dependency group by editing pyproject.toml

- **Status:** Accepted (2026-10-05)
- **Implemented by:** #20

## Context

`uv remove pkg --group dev` removes the last package from a group and leaves `dev = []` in
pyproject.toml; so does a move out of it. uv 0.8.9 has no command that deletes a group. Every
other change uview makes goes through the `uv` binary, and the user wants an emptied group
offered for deletion.

Deleting the key is not enough on its own. uv.lock lists the project's groups, so it is stale
until relocked. And a group that `tool.uv.default-groups` or another group's
`{ include-group = … }` names must stay: `uv sync` refuses a default group that is not defined.

## Decision

We will delete an empty group ourselves, in `controllers/groups.rs`, and only there. The edit
uses `toml_edit`, so the rest of the file keeps its formatting and comments. It runs only on a
group that is empty and that nothing else names, and is followed by `uv lock`; if the lock
fails, the file is put back as it was. The user is asked first, once, right after a removal or
move of theirs empties the group.

## Consequences

- uview writes pyproject.toml directly in one narrow case. Everything else still goes through
  `uv`, and the lock still comes from uv.
- A new dependency, `toml_edit`.
- What counts as "named elsewhere" is ours to keep in step with uv: default groups and
  `include-group` today. A reference we miss is caught only if `uv lock` rejects it.
- When uv gains a command for this, this module becomes one `UvClient` call.

## Alternatives considered

- **Tell the user to delete it by hand.** Keeps the principle, but the empty group stays in
  the group lists until they do.
- **Leave empty groups alone.** What uv does; it is what the user asked to change.
