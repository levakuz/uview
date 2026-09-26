# 0004. `AppState` is introduced ahead of the need

- **Status:** Accepted (2026-09-03)
- **Implemented by:** #43, with #44 and #11

## Context

Domain data lives inside the widgets that display it: `SharedTable<T>` owns its
`Vec<T>`. Today there is one page and one copy of each list, so nothing can diverge.

That changes with routing (#26, Sprint 4). Data then dies with the widget when the user
navigates away. The dependency tree and the pyproject table show the same facts from two
places. Loads are asynchronous and can return out of order.

The source of truth is the project on disk and the virtualenv, and `uv` is the only
writer. Everything `uview` holds in memory is a cache of it.

## Decision

We will move domain data into an `AppState` made of per-domain sub-structs: project,
dependencies, jobs and messages. Components keep only view state and receive `&AppState`
when rendering.

`AppState` carries no behaviour. Each sub-struct owns its own logic, and a component
never reaches across domains.

Asynchronously loaded data is held as `Loadable<T>`, not a bare collection. A generation
counter makes out-of-order results discardable (#44, #11).

This lands in Sprint 1, not Sprint 4.

## Consequences

- Loaded data survives navigation, and invalidation has a single target.
- **This does not guarantee consistency.** It reduces the number of caches from one per
  widget to one, so invalidation becomes a single operation instead of a fan-out that
  every new widget must remember to join. Correctness comes from the invalidation
  discipline of ADR-0003 and from the generation counter.
- `AppState` is a god object waiting to happen. The rules against behaviour and against
  crossing domains are what hold it off.
- The refactor is paid for before its benefit is observable.

## Alternatives considered

- **Defer to Sprint 4, when routing needs it.** This was the original proposal. It was
  rejected because the cost of the extraction grows with every widget that owns data, and
  two tables and three popups is the cheapest it will ever be.
- **Keep data in widgets and reload on every page entry.** Simple and correct, but it
  costs a visible delay and loses scroll position on each navigation.
- **Share each domain through `Rc<RefCell<_>>`.** This fails with runtime borrow panics,
  which the crate's no-panic rule forbids.
