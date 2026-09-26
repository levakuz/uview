# 0003. Mutating `uv` operations are serialized; reads are not

- **Status:** Accepted (2026-09-03)
- **Implemented by:** #13

## Context

`uv` takes a lock on the project. Of two concurrent mutations such as `uv add` and
`uv remove`, one fails. Reads (`uv pip list`, `uv tree`, an index search) take no such
lock (`ARCHITECTURE.md` §12, F3).

Operations are slow: a resolve can take ten seconds. The user needs to see progress and
be able to cancel (F4).

Today every operation is spawned independently, so two quick mutations race and one
fails with a lock error.

## Decision

The runtime (ADR-0001) will route commands by kind:

- **Read-only** commands are spawned freely and run concurrently.
- **Mutating** commands go through a single-consumer queue and run strictly one at a time.

Every job carries an id, a human-readable label, a cancellation token and a status.
Cancellation reaches the child process instead of merely abandoning the task.

A completed mutation emits an invalidation event, and views react to it by reloading.
They no longer each remember to refresh.

## Consequences

- No more lock-contention failures between our own operations.
- The progress indicator (#13), cancellation and invalidation all come from one component.
- Mutations can never overlap. That costs nothing, because `uv` forbids overlap anyway.
- A queued mutation waits visibly, so the UI has to show a queued state as well as a
  running one.

## Alternatives considered

- **Let `uv`'s lock arbitrate and retry on failure.** This surfaces spurious errors and
  spreads retry logic across every call site.
- **Serialize everything, reads included.** This adds needless latency to search and to
  the dependency tree.
- **A mutex around mutating calls.** It serializes just as well, but gives nothing to
  show: no queue, no job ids, no progress.
