# 0006. Cancelling a job interrupts `uv` and waits for it to exit

- **Status:** Accepted (2026-10-04)
- **Implemented by:** #13

## Context

ADR-0003 asks that cancellation reach the child process rather than abandon the task. The
simplest way is to drop the job's future and let tokio kill the child (`kill_on_drop`).

`uv add` writes the requirement into pyproject.toml before it resolves, and resolving is the
slow part, so that is when a user cancels. Killed, `uv add` leaves the requirement declared but
neither locked nor installed. On SIGINT, which `uv` takes as Ctrl+C, it puts pyproject.toml
back and exits. `uv remove` and `uv sync` have no such handler: SIGINT ends them at once, as
a kill would, and `uv remove` leaves its edit behind. All three were checked against uv 0.8.9
with an index that never answers.

`uv add` needs a moment to restore the file. The runner reloads the tables after every
mutation, and a reload that starts as the signal is sent reads the file before `uv` has
restored it. The table then lists a package that is not declared.

## Decision

We will interrupt `uv` the way Ctrl+C does, and end the job only once `uv` has exited.

- `UvClient` gains `run_cancellable(args, &CancellationToken)` beside `run`. Its default
  drops the call when the token fires, so a fake needs nothing new.
- `RealUvClient` sends SIGINT on Unix and waits up to two seconds for `uv` to exit, then
  kills it. Windows cannot send a child Ctrl+C, so there it kills `uv` at once.
- The runner hands each job a client whose `run` is `run_cancellable` with the job's token.
  Controllers keep calling `run` and know nothing of cancellation.
- Quitting cancels every job and waits for them, so it leaves the project as cancelling does.

## Consequences

- On Unix, an add or a pin cancelled while `uv add` resolves leaves pyproject.toml as it
  was, and the reload after it shows that. `uv add` sets its handler just after it writes the
  file, so a cancel in the few milliseconds between kills it and leaves the edit behind.
- Cancelled at its sync, an add or a pin is declared and locked but not installed. A
  cancelled remove can leave the package removed from pyproject.toml but not from the
  lockfile, and on Windows any cancel can leave a change behind. The reload shows what is
  on disk.
- A `uv` that ignored SIGINT would hold the job, and quitting, for two seconds before it is
  killed.
- `UvClient` has two methods. ADR-0005 still holds: argv in, stdout or an error out. It
  already foresaw a second method, for streaming (#65).
- Cancellation is checked at each `uv` call. A step that runs no `uv`, such as reading
  pyproject.toml, runs to its end.

## Alternatives considered

- **Kill through `kill_on_drop` alone.** Simple and the same on every platform, but every
  cancelled add leaves pyproject.toml changed.
- **SIGINT as the future is dropped.** `uv add` restores the file, but the job ends at once
  and the reload races the restore. This was the first implementation; the race showed up
  end to end.
- **A token argument on every controller function.** Each operation would pass it through
  without using it.
- **A task-local token that `RealUvClient` reads.** No signature changes, but nothing at a
  call site would show that it can be cancelled, and a fake would have to read it too.
