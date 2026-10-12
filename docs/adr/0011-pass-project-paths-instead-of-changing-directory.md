# 0011. Pass the project's paths instead of changing the working directory

- **Status:** Accepted (2026-10-12)
- **Implemented by:** #104

## Context

`main.rs` made the project root the process's working directory with `set_current_dir`. From
then on pyproject.toml was read as `./pyproject.toml`, from a literal in one module and a
constant in another, and every `uv` subprocess ran wherever the process was. The environment
path had already been made explicit, in #81, so that tests could choose it; pyproject.toml had
not. The working directory is state shared by every task, set once, and invisible at the
places that depend on it: tests had to split parsing out to reach it (#25), reload tests read a
file that was not there, and opening a second project in one run (#26) would move it under
running jobs.

## Decision

We will resolve the project's paths once, in `main.rs`, into `ProjectPaths { root, pyproject,
env }`, and hand them to the `App` and the `JobRunner`, which pass each command the path it
needs. `RealUvClient` holds the directory it runs `uv` in and sets it on the child process.
uview's own working directory stays where it was started. `tests/architecture.rs` forbids
`set_current_dir` and `"./` paths in `src/`.

## Consequences

- No path in `src/` depends on a call made elsewhere; a test hands the runner a fixture
  pyproject.toml.
- Controllers that touch pyproject.toml take its path, so their signatures say so.
- A second project would need only another `ProjectPaths` and `RealUvClient`.
- `main.rs` still reads the directory uview was started in, once, to find the project.

## Alternatives considered

- **Keep `set_current_dir`, with one constant for the path.** Removes the duplication, but not
  the hidden dependency or its cost to tests.
- **Pass `--directory` to every `uv` call.** Puts the root into each argv instead of one place,
  and changes how uv resolves relative paths in the user's files.
