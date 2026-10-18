# 0015. Delete the project's environment by removing its directory

- **Status:** Accepted (2026-10-18)
- **Implemented by:** #29

## Context

The environment popup offers to make the environment anew with another Python, to empty it,
and to delete it. The first two are uv commands: `uv sync --python <key>` and
`uv venv --clear <env>`. uv 0.8.9 has no command that deletes an environment; `uv venv`
without `--clear` keeps what is installed and only warns. Every other change uview makes goes
through the `uv` binary, and the user wants delete in the popup.

The directory comes from `UV_PROJECT_ENVIRONMENT` when it is set, so it can be anywhere,
including the project root or above it. A wrong path here deletes the user's work.

## Decision

We will delete the environment ourselves, in `controllers/environment.rs`, and only there. It
removes the directory `ProjectPaths::env` names, and only when that directory holds
`pyvenv.cfg` and neither is the project root nor lies above it. The user is asked first, every
time. Afterwards `s` makes it again.

## Consequences

- uview deletes a directory outside uv in one narrow case. The other two actions, and making
  the environment again, still go through `uv`.
- The checks are ours to keep: a directory that holds `pyvenv.cfg` and is not above the
  project root is assumed to be the environment uv made.
- When uv gains a command for this, the function becomes one `UvClient` call.

## Alternatives considered

- **Leave delete out.** Emptying covers most needs, but an environment made with the wrong
  Python or broken badly stays until the user removes it by hand.
- **Delete without the checks.** One line shorter; a mistyped `UV_PROJECT_ENVIRONMENT` would
  then take the project with it.
