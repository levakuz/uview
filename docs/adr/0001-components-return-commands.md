# 0001. Components return commands; a runtime executes them

- **Status:** Accepted (2026-09-03)
- **Implemented by:** #42

## Context

One flat `Action` enum mixes three unrelated things: raw input (`ButtonPressed`), user
intent (`RemovePackage`) and effect results (`LoadedEnvDeps`, `Error`). It had 19
variants while serving a single page, and seven sprints of pages and operations will
land on it.

The widget `DependenciesTablesTabs` owns an `mpsc` sender and calls `tokio::spawn`
directly. It cannot be tested without a runtime, and orchestration sits at the wrong
level of the tree.

The render loop must never block, and `uv` operations are slow (`ARCHITECTURE.md` §12,
F1 and F4), so side effects cannot run where they are decided.

## Decision

We will split messages by direction of travel: `Input` from the terminal, `Action` as
intent nested per domain, `Command` as a requested effect, and `Event` as its result.

Components will implement `update(&mut self, &Action, &mut AppState) -> Vec<Command>`
and never spawn. A single runtime module executes commands and emits events. It is the
only module that spawns tasks.

`App::dispatch`'s unbounded recursion will be replaced by an explicit work queue.

## Consequences

- `update` becomes testable by calling it and asserting on the returned commands, with no
  runtime, subprocess or terminal.
- Spawning, cancellation and job tracking live in one place, which is what makes
  ADR-0003 possible.
- "`src/ui/` never spawns" becomes a rule that can be checked mechanically (#47).
- A component can no longer "just do the thing". For a single-screen app this indirection
  is pure overhead. It pays off once two pages need the same data and two operations can
  be in flight, which the plan reaches in Sprint 1.

## Alternatives considered

- **Keep spawning from widgets.** This is the right answer for one page. It does not
  survive routing and leaves every widget untestable.
- **Hand a sender to every component.** This still couples the UI to tokio and keeps
  side effects scattered.
- **A separate message type per page.** Deferred, not rejected: revisit if `Action`
  passes about fifty variants even when nested (`ARCHITECTURE.md` §16).
