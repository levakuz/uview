# 0008. uview finds a project's workspace root as uv does

- **Status:** Accepted (2026-10-05)
- **Implemented by:** #58

## Context

The Environment table lists the environment uview hands `uv pip` with `--python`, and its
Remove uninstalls from it. That has to be the environment `uv sync` uses, or the table shows
one environment while every sync changes another. For a member of a uv workspace, `uv sync`
uses the environment at the workspace root: `.venv` there, or a relative
`UV_PROJECT_ENVIRONMENT` resolved from there. uview took `.venv` beside the project. A member
without one showed an error. A member with one, made on that error's advice, silently showed
the wrong environment (#58).

uv 0.8 has no command that names the workspace root or the project environment:

- `uv python find` returns the nearest `.venv` on disk, which can be the member's.
- `uv sync --dry-run --output-format json` reports both, but its schema is marked preview, and
  it resolves the project first, which can need the network.
- `uv workspace` does not exist in uv 0.8.

The rules are few. They were checked against uv 0.8.9:

- A project that declares `[tool.uv.workspace]` is its own root.
- Otherwise uv considers only the nearest pyproject.toml above the project. Its directory is
  the root if its `members` globs match the project and its `exclude` globs do not.
- Otherwise the project stands alone.

## Decision

We will find the workspace root in uview, by uv's rules, and resolve the environment against
it.

- `controllers::project::workspace_root` applies the rules to the project root. It matches the
  globs as uv does: with the `glob` crate's defaults, so `*` crosses `/`, and with the root
  escaped.
- `project_environment` joins `.venv`, or `UV_PROJECT_ENVIRONMENT`, to that root. uview works it
  out again for each `uv pip` call, from the working directory, as uv would.
- Everything else still runs in the project, so add, remove and the pyproject table act on the
  member's own pyproject.toml.

## Consequences

- A member's Environment table, and its Remove, act on the environment its syncs change. A
  project nested in a workspace but not a member of it keeps its own `.venv`, as it does with
  uv.
- uview now holds a piece of uv's logic, which a later uv can change. The tests pin the rules
  as uv 0.8.9 applies them. Once uv has a stable command that names the workspace root or the
  project environment, uview should ask it instead.
- A new dependency, `glob`, which is small and has no dependencies of its own.
- `uv pip list` now runs with `--quiet`. Given an environment other than the `.venv` beside it,
  uv names that environment on stderr, which would add a line to the uv log on every reload.

## Alternatives considered

- **Ask `uv sync --dry-run --output-format json`.** That is uv's own answer, but its schema is
  marked preview, and every reload would resolve the project, which can need the network.
- **`uv python find`.** It answers a different question: the nearest environment on disk.
- **A hand-written glob matcher.** It would avoid the dependency, but uv's matching is the
  `glob` crate's: `*` across `/`, character classes, escaping. A near match would put a project
  in the wrong environment.
