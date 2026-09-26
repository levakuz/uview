# uview

A terminal UI for [`uv`](https://github.com/astral-sh/uv), the Python package manager —
browse and manage a project's dependencies without memorising command-line syntax.

**Status: early development.** Not yet a daily driver. See the
[open issues](https://github.com/levakuz/uview/issues) and
[milestones](https://github.com/levakuz/uview/milestones) for what works today and what
is planned.

## Documentation

- **[Architecture](docs/ARCHITECTURE.md)** — how the code is built today, and the target
  design we are working toward. Read this before making a change.

## Building

```bash
cargo build
cargo test
```

Requires a Rust toolchain (edition 2024) and [`uv`](https://docs.astral.sh/uv/getting-started/installation/)
on `PATH`.
