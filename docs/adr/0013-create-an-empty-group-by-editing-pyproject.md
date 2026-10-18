# 0013. Create an empty dependency group by editing pyproject.toml

- **Status:** Accepted (2026-10-18)
- **Implemented by:** #109

## Context

`g` manages the project's dependency groups (#109): create, rename, delete. uv 0.8.9 has no
command for a group itself; `--group` exists on `uv add` and `uv remove`, one package at a
time. A group therefore comes into being only with a package in it, and ADR-0009 already lets
uview delete one left empty. uv accepts an empty group, `lint = []`, and locks it. It rejects a
name that does not start and end with a letter or digit or holds anything but those and `-`,
`_`, `.`, and compares names normalized.

## Decision

We will create an empty group ourselves: `name = []` in `[dependency-groups]`, written with
`toml_edit` through `edit_then_lock`, which relocks and puts the file back if the lock fails, as
ADR-0009 and ADR-0010 do. A name uv would refuse, or one taken as uv compares them, is refused
before anything is written. Everything else `g` does is a sequence of commands the runtime
already runs, one after another: `uv remove` per package and `DeleteGroup` to delete a group
with packages; `uv add` per package to the new name, then `uv remove` from the old, then
`DeleteGroup`, to rename one. A group that `default-groups` or `include-group` names is neither
deleted nor renamed.

## Consequences

- A third edit of pyproject.toml outside uv, again narrow, again followed by `uv lock`.
- Renaming an empty group is creating the new name and deleting the old.
- A rename or deletion runs one `uv add` or `uv remove` per package, each of which syncs, so a
  large group is slow; batching them would change the commands they reuse.
- When uv gains group commands, these become `UvClient` calls.

## Alternatives considered

- **Create a group only with its first package, through `uv add --group`.** Keeps to uv, but no
  empty group could be made, and an empty group could not be renamed.
- **Rename by editing the table's key.** One edit instead of a command per package, but it would
  duplicate what `uv add` and `uv remove` already do to the file, and the lock.
