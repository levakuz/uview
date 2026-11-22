# AGENTS.md

Working agreement for this repository — for humans and coding agents alike.
Claude Code reads this through the `@AGENTS.md` import in `CLAUDE.md`.

**When requirements are unclear, ask before writing code.**

## Project

`uview` is a terminal UI for [`uv`](https://github.com/astral-sh/uv), the Python package
manager, written in Rust (edition 2024) on ratatui. It owns no packaging logic of its
own — every operation is delegated to the `uv` binary as a subprocess. The exceptions,
changes uv has no command for, are deleting an emptied dependency group (ADR-0009),
managing the project's package indexes (ADR-0010), creating an empty dependency
group (ADR-0013) and deleting the project's environment (ADR-0015). What uv cannot list,
a package's versions, uview reads from the index over HTTP, read only (ADR-0012).

Architecture decisions live in `docs/adr/`. Tasks and their status live in
[GitHub issues](https://github.com/levakuz/uview/issues).

## Commands

| | |
|---|---|
| Build | `cargo build` |
| Run | `cargo run` |
| Test | `cargo test` |
| Single test | `cargo test <test_name>` |
| Lint | `cargo clippy -- -D warnings` |
| Format | `cargo fmt` |
| Release | `cargo build --release` |

## Architecture

See **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** — Part I is the system as it is,
Part II the target we build toward. Read it before changing anything structural.

Do not restate the architecture here or in code comments. It drifted from the code once
already; one owner per fact.

The short version:

- **Three layers.** `src/ui/` renders and turns input into actions; `src/controllers/`
  runs `uv` operations through the `UvClient` in `src/uv/`, which alone starts the `uv`
  process. Controllers never import ratatui; UI code never invokes `uv` directly.
- **UI is layered** `pages/` › `components/widgets/` › `components/features/` ›
  `components/shared/`. Parameterised over a trait and portable to another TUI → `shared`.
  Names a domain type or a `uv` concept → `features`.

## Architecture decisions

A decision that is hard to reverse, changes the structure, or will make someone ask
"why is it like this?" in six months gets an ADR in `docs/adr/`. Most PRs do not need one.

- Copy `docs/adr/template.md` to the next free number: `docs/adr/NNNN-kebab-title.md`.
- Put it in the same PR as the code that implements it, when there is one. Merged means
  accepted; the PR is the proposal.
- Never edit an accepted ADR except its Status line. To reverse a decision, write a new
  ADR and mark the old one `Superseded by ADR-NNNN`.
- `ARCHITECTURE.md` describes what the system is; an ADR records why. Link, don't restate.

## Code conventions

- **No `panic!`, `unwrap` or `expect` anywhere in the crate** — enforced by
  `[lints.clippy]` in `Cargo.toml`. The app owns a raw-mode terminal: a panic inside a
  spawned task fires ratatui's panic hook, which tears the terminal down while the app
  keeps drawing on top of it. Return a typed error and report it through the action
  channel.
- Prefer `uv`'s machine-readable output (`--format json`, `--output-format json`) over
  parsing human-readable lines. Where line parsing is unavoidable, isolate it so one
  module absorbs the breakage when `uv` changes.
- Keep argument construction pure and separate from process execution, so it can be
  tested without spawning anything.

## Testing

- Tests live in `tests/` as integration tests, not inline in `src/`.
- `src/lib.rs` exposes the crate, so tests reach items as `uview::...`. Anything to be
  tested must be `pub`.
- `tests/architecture.rs` enforces the layering rules of `docs/ARCHITECTURE.md`. A known
  violation goes on its allowlist only with the issue that removes it.
- Fixtures go in `tests/fixtures/`. The root `pyproject.toml` is not one: it packages
  uview as a wheel (ADR-0017). Try uview on a project outside the repository, not on it.

## Git workflow

Trunk-based. `main` is the only long-lived branch. No `develop`, no release branches — pre-1.0 project does not need them, and they cost more than they return.

**Never commit to `main` directly.**

**One branch per issue**, short-lived, named `<type>/<issue>-<slug>`:

```
fix/1-uv-error-types
feat/9-project-header
refactor/42-message-types
docs/40-architecture
```

Types: `feat`, `fix`, `refactor`, `perf`, `test`, `docs`, `chore`.

**Commits** are `type: imperative summary`, summary ≤ 72 characters. The body explains
*why*; the diff already shows what. Close the issue from the commit or the PR body:

```
fix: return typed errors from uv controllers

Closes #1.

Every uv invocation panicked on failure. Inside a spawned task that fired
ratatui's panic hook, which tore the terminal down while the app kept
drawing on top of it.
```

**Update a branch by rebasing**, not by merging `main` into it:

```bash
git fetch origin && git rebase origin/main
```

**One PR per issue.** Squash on merge, so `main` gets one commit per issue and its
history reads as the issue list.

## Definition of done

Before a PR is merged:

- Every acceptance criterion in the issue is met, or the PR says which is not and why.
- `cargo test`, `cargo clippy -- -D warnings` and `cargo fmt --check` all pass.
  `.claude/hooks/quality-gate.sh` runs exactly these as a Claude Code Stop hook, so an
  agent cannot finish a turn with Rust changes that fail them. CI
  (`.github/workflows/ci.yml`) runs the same on Linux, macOS and Windows for every pull
  request. A change to the packaging also builds and installs the wheels
  (`.github/workflows/release.yml`). The toolchain is pinned in `rust-toolchain.toml`.
- A change that alters the layering, the action flow or the module tree updates
  `docs/ARCHITECTURE.md` and the rules in `tests/architecture.rs` in the same PR.
- A change that moves *away* from the target state in Part II is allowed, but the PR
  says so and says why.
