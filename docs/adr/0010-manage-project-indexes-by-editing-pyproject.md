# 0010. Manage the project's indexes by editing pyproject.toml

- **Status:** Accepted (2026-10-08)
- **Implemented by:** #95

## Context

Users who install from a private or mirror index want to see and change the project's
`[[tool.uv.index]]` entries. uv 0.8.9 has no command for them: there is no `uv index`, and
`--index` / `--default-index` exist on `uv add` and its kin only for that one invocation, `uv add`
needing a package besides. ADR-0009 already made one exception to delegating every operation to
uv, for an emptied group, with a pattern: edit, relock, put the file back if the lock fails.

uv checks less than it might: it accepts two `default = true` indexes without saying which it
uses, and it takes a mistyped URL for a path. It does refuse a duplicate or malformed name, and
fails to lock when `[tool.uv.sources]` pins a package to an index that is gone.

## Decision

We will add and remove `[[tool.uv.index]]` entries ourselves, in `controllers/indexes.rs`, with
`toml_edit` and the edit-then-lock helper ADR-0009's code now shares
(`controllers/pyproject.rs`). Before writing, uview refuses what uv would refuse or miss: a
taken or malformed name, a URL that is not http(s), a URL holding a user or token, and removing
an index a package is pinned to. Adding a default index clears `default` from the previous one.
Credentials stay out of scope (#38): uview never writes one.

## Consequences

- A second narrow case where uview writes pyproject.toml. Every package operation still goes
  through `uv`, and the lock always comes from uv.
- Validation that duplicates uv's is ours to keep in step; `uv lock` after each edit, with the
  file restored on failure, catches what we miss.
- A comment right above a removed index goes with it, as `toml_edit` attaches it to the table.
- When uv gains index commands, this module becomes `UvClient` calls.

## Alternatives considered

- **`uv add <pkg> --index …` only.** Needs a package and cannot remove an index.
- **Leave indexes to hand edits.** Keeps the principle; it is what the issue asks to change.
- **Write `uv.toml` instead.** Out of the project file, so not shared with the team.
