# 0002. `UvClient` is the only seam to `uv`

- **Status:** Accepted (2026-09-03)
- **Implemented by:** #41

## Context

`uview` has no in-process API to call. Every capability is the `uv` binary: argv in,
exit code and stdout out (`ARCHITECTURE.md` §12, F2).

Until #1, every function built and spawned its own subprocess inline, and the repository
had exactly one test. Since #1 every call goes through `run_uv`, which returns a typed
`UvError`. That gives a single path, but a concrete one: anything above it still needs a
real `uv`, a real project and possibly the network in order to be tested.

The boundary is narrow and uniform, so putting a trait across it is cheap.

## Decision

We will put a `UvClient` trait over every `uv` operation `uview` performs.

- `RealUvClient` is the only code that spawns `uv`.
- `FakeUvClient` returns canned results and records the argv it received.
- Argument construction and output parsing stay pure functions outside the trait, so they
  are testable without any client.
- Machine-readable output (`--format json`) is used wherever `uv` offers it.

Callers receive the client as a parameter, never through a global. Static dispatch
through generics is preferred: `dyn UvClient` with async methods would need boxing or an
`async_trait` dependency.

## Consequences

- The whole layer above `uv` becomes testable without `uv` installed.
- A breaking change in `uv`'s output lands in one module.
- There is a second implementation to keep honest. The fake can drift from real `uv`
  behaviour, for example in exit codes or output shape, so a few tests keep calling real
  `uv`, as `successful_uv_call_returns_stdout` does today.

## Alternatives considered

- **A fake `uv` executable on `PATH` in tests.** This exercises the real spawning, but it
  is slow, needs per-platform handling (`.exe` on Windows) and is awkward to script per
  test.
- **Test only the pure functions.** This leaves all orchestration untested.
- **Link against `uv` as a library.** Not possible: `uv` has no stable library API.
