# 0007. Components read `AppState`; only the app writes it

- **Status:** Accepted (2026-10-04)
- **Implemented by:** #43

## Context

ADR-0001 gave components `update(&mut self, &Action, &mut AppState) -> Vec<Command>`, and
ADR-0004 moved domain data into `AppState`. When #43 did that move, no component had anything
to write there. What a component changes is its own view state: which tab, row or popup is
showing. The data changes only when the runtime reports an event: rows loaded, a job's
progress, a notice or an error.

## Decision

We will pass components `&AppState`, in `render` and in `update` alike. `App` is the only
writer: it puts each runtime event into the state, and dismisses the message on Esc. A
component that wants the data changed returns a command, as for everything else.

## Consequences

- Every change to what the app knows is in one function, `App::receive`, plus the Esc above.
- A component cannot leave the state half-updated or out of step with the project on disk.
- `update` can be tested with a state built in the test, as before with no runtime.
- When a component does need to write, such as a page that edits something the app knows,
  this becomes `&mut AppState` in the same change. The decision is cheap to reverse.

## Alternatives considered

- **`&mut AppState`, as ADR-0001 says.** It would give every component write access that
  none uses, and the single writer would rest on convention instead of the type.
- **Components own their data, as before #43.** ADR-0004 rejects this.
