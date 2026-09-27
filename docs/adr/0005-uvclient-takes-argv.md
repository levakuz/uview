# 0005. `UvClient` takes argv, not one method per operation

- **Status:** Accepted (2026-09-27)
- **Implemented by:** #41

## Context

ADR-0002 puts a `UvClient` trait across every `uv` call. It says the fake records the
argv it received, and that argument construction stays outside the trait. It does not
spell out the trait's shape.

Issue #41 sketched a different one: a typed method per operation (`pip_list`, `add`,
`remove`, `sync`) behind `#[async_trait]`.

Each operation `uview` performs is a sequence of `uv` invocations. Removing a package, for
example, is one `uv remove` per group or extra it is declared in, then one
`uv sync --inexact`. The two shapes differ in what a fake can observe of that sequence,
and in how much code each new operation costs.

## Decision

We will give `UvClient` a single method:

```rust
fn run(&self, args: &[&str]) -> impl Future<Output = Result<String, UvError>> + Send;
```

The contract is argv in, and stdout or a `UvError` out.

Operations stay in `controllers/` as functions that take `&impl UvClient`. They build
argv with `uv::args` and read output with `uv::parse`. Those functions are the typed API
that `uview` uses; the trait is not.

## Consequences

- The fake records exactly the argv that would run. A test of an operation therefore
  checks its sequencing and its argument construction together.
- Each operation is written once. A new operation changes neither the trait, the real
  client nor the fake.
- The trait does not list the operations that exist; `controllers/` and `uv::args` do.
- Only buffered output is possible. Streaming (`ARCHITECTURE.md` §13.6; #13, #30) needs a
  second method beside `run`, and the fake must implement it too.
- Tests assert on argv, so reordering flags breaks them even where `uv` would not mind.
  That is acceptable: the argv is exactly what they are meant to pin down.

## Alternatives considered

- **One method per operation, as #41 sketched.** Every operation would be written twice,
  once in `RealUvClient` and once in the fake. The fake would record which method was
  called, not what would run. The argv would stay untested unless the fake rebuilt it
  with the same `uv::args` functions, and that duplicates the real client.
- **`dyn UvClient` behind `#[async_trait]`.** ADR-0002 already rejects this: it needs
  boxing or a new dependency, and nothing in `uview` needs to choose an implementation at
  runtime.
