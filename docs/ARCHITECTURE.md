# Architecture

This document has two parts, and the split is the point.

- **Part I — Current state** describes how `uview` is put together *as it exists today*,
  at commit `f903967`. Nothing in it describes an aspiration as if it were code.
- **Part II — Target state** describes the architecture we are building toward, the
  forces that make it the right shape for this kind of application, and which issue moves
  us each step closer.

Read Part I to understand the code you are about to change. Read Part II before deciding
*how* to change it: a pull request that moves away from the target state is allowed, but
it has to say so and say why.

This document says what the architecture is. Why each major choice was made — the
context, the alternatives and the cost — lives in the decision records in
[`docs/adr/`](adr/).

If you change the layering, the action flow or the module tree, update this file in the
same pull request.

---

# Part I — Current state

## 1. Overview

`uview` is a terminal UI for the [`uv`](https://github.com/astral-sh/uv) Python package
manager, built on [ratatui](https://ratatui.rs) and [crossterm](https://github.com/crossterm-rs/crossterm),
with [tokio](https://tokio.rs) providing the async runtime.

It owns no Python or packaging logic of its own. Every operation is delegated to the
`uv` binary as a subprocess, and `uview` is responsible only for presenting state and
turning key presses into `uv` invocations.

The design follows the Elm architecture (TEA), with messages split by direction of travel
(§4): a page's keymap turns input into actions, components turn actions into commands, and
the runtime performs the commands and reports what came of them as events.

## 2. Layers

There are four layers, plus a thin application shell.

| Layer | Location | Responsibility | Must not |
|---|---|---|---|
| **App shell** | `src/app.rs` | Owns the terminal, the event loop, the event channel and the active page; hands commands to the runtime | Contain feature logic |
| **UI** | `src/ui/` | Render frames, turn input into actions and actions into commands, hold view state | Spawn tasks, call controllers, read the filesystem |
| **Runtime** | `src/runtime/` | Run each command as a task through the controllers, report its outcome as events | Render, hold UI state, know about ratatui |
| **Controllers** | `src/controllers/` | Run `uv` operations through a `UvClient`, read `pyproject.toml` | Render, hold UI state, know about ratatui, start a process |
| **uv client** | `src/uv/` | Start the `uv` binary, build its arguments, parse its output | Know about the runtime, controllers, the UI or ratatui |

The rule that matters: **controllers never import ratatui, UI code never invokes `uv` or a
controller, and only `src/uv/` starts a process.** The UI and the controllers never meet: a
component returns commands, and the runtime runs them. The runtime is the only code that
spawns tasks.

`tests/architecture.rs` enforces every part of the rule (§10), with no exceptions.

## 3. Data flow

```
                    ┌──────────────────────────────────────────────┐
                    │              App::run  (app.rs)              │
                    │                                              │
   key press ──────▶│  crossterm::event::poll / read               │
                    │        │                                     │
                    │        ▼                                     │
                    │  page.action_for(&Input)          keymap     │
                    │        │                                     │
                    │        ▼                                     │
                    │  App::dispatch(action)                       │
                    │        │                                     │
                    │        ▼                                     │
                    │  page.update(&action) ── Vec<Command> ──┐    │
                    │                                         │    │
                    │  events_rx.try_recv() ◀── Event ──┐     │    │
                    │        │                          │     │    │
                    │        ▼                          │     │    │
                    │  status line, or page.apply       │     │    │
                    │                                   │     │    │
                    │  terminal.draw(page.render)       │     │    │
                    └───────────────────────────────────┼─────┼────┘
                                                        │     ▼
                    ┌───────────────────────────────────┴──────────┐
                    │  runtime::execute: one task per command      │
                    │  controllers::* → uv add / remove / sync / … │
                    └──────────────────────────────────────────────┘
```

The concrete path for a key press, using "pin a version" as the example:

1. `App::run` polls crossterm and wraps the key as `Input::Key(KeyEvent)`.
2. `DependenciesPage::action_for`, the page's keymap, turns it into an action: inside an
   input, `Enter` becomes `Action::Nav(NavAction::Confirm)`.
3. `App::dispatch` hands the action to `DependenciesPage::update`, which passes it to
   `DependenciesTablesTabs`.
4. The widget sends it to the input that is open, the pin input (an `InputPopup`), which
   returns the version typed.
5. The widget returns `Command::Pin` for the package the actions list was opened for.
6. `App::dispatch` hands the command to `runtime::execute`, which spawns a task running
   `controllers::dependencies::pin_dep_version`.
7. The task sends `Event::Notice`, then reloads both tables and sends
   `Event::PyprojectLoaded` and `Event::EnvLoaded` through the event channel.
8. The next loop iteration drains the channel: `App` shows the notice and hands the loaded
   rows to `DependenciesPage::apply`.
9. The following `terminal.draw` renders the new state.

## 4. Messages and dispatch

Messages are split by direction of travel (`src/message.rs`, ADR-0001):

- **`Input`** — what the terminal sends: `Key(KeyEvent)`.
- **`Action`** — what the user means, nested per domain: `Quit`, `Nav(NavAction)` for
  moving, choosing and typing, and `Deps(DepsAction)` for the dependencies page.
- **`Command`** — an effect a component asks for: `Reload`, `Add`, `RemoveFromProject`,
  `Uninstall`, `Upgrade`, `Pin`.
- **`Event`** — what a command produced: the loaded rows, `Notice(String)` and
  `Error(String)`.

The page's keymap, `DependenciesPage::action_for`, turns an `Input` into an `Action`. While
an input is open, every key but Enter and Esc becomes `NavAction::Type`. Outside an input,
Alt+key means nothing, because Esc read together with the next key arrives as Alt+key.

`App::dispatch` handles `Quit` itself and hands every other action to the page's `update`,
which returns `Vec<Command>`. Each command goes to `runtime::execute`. Esc, as
`NavAction::Back`, also dismisses the status message. Components return commands, never
actions, so nothing comes back into `dispatch` and it cannot recurse. ADR-0001 planned an
explicit work queue against that recursion; with no action coming back, there is nothing to
queue.

Inside the page, a popup reports to the widget that owns it with a plain return value: the
actions list returns the option chosen, and the pin input the version typed. The widget
turns that into a command.

The runtime sends `Event`s through one `mpsc::UnboundedSender<Event>`. `App` drains the
receiver once per loop iteration. `Notice` and `Error` set the one-line status message it
renders below the page, beside the project root. Every other event goes to
`DependenciesPage::apply`, which puts the rows into the tables whatever has focus: until
`AppState` exists (#43), the tables are where the data lives.

## 5. Module map

```
src/
├── main.rs                 # tokio entry point; finds the project root and makes it the cwd
├── lib.rs                  # public surface, so tests/ can reach everything but app.rs
├── app.rs                  # App: event loop, event channel, dispatch, status line, active page
├── message.rs              # Input / Action / Command / Event
├── domain.rs               # PyprojectDependency, EnvDependency, Membership
├── runtime/
│   └── mod.rs              # runs each command as a task and reports events; the only spawn
├── controllers/
│   ├── dependencies.rs     # operations: list, add, pin, update, remove, uninstall; reads pyproject.toml
│   └── project.rs          # startup: the project root, or why uv would refuse it
├── uv/
│   ├── mod.rs              # UvClient trait; RealUvClient, the only process spawn; UvError
│   ├── args.rs             # the argv of each operation, pure
│   └── parse.rs            # uv output into domain types, pure
└── ui/
    ├── mod.rs              # Component trait
    ├── pages/
    │   └── dependencies.rs # DependenciesPage — keymap and top-level layout
    └── components/
        ├── shared/         # generic, feature-agnostic
        │   ├── table.rs    #   SharedTable<T: TableRow>
        │   ├── input.rs    #   UviewInput (wraps tui-input)
        │   ├── popup.rs    #   Popup — centering and Clear only
        │   └── status_bar.rs # the bottom line owned by App: error/notice + project root
        ├── features/       # concrete, tied to this domain
        │   ├── dependencies_table.rs      # TableRow impls for the two dep types
        │   ├── input_popup.rs             # a popup with one input: the add and pin inputs
        │   └── package_control_popup.rs   # the actions list
        └── widgets/
            └── dependencies_tables_tabs.rs # composes tables + popups, turns their results into commands
```

**Planned, not yet written.** `controllers/python.rs`,
`ui/pages/home.rs`, `ui/pages/python.rs`. An earlier revision of the agent instructions
listed these as if they existed; they do not. This document is now the single description
of the module tree, and `AGENTS.md` points here rather than repeating it.

## 6. UI component taxonomy

Four kinds of UI code, in descending order of generality:

- **`pages/`** — a full screen. Owns the top-level layout and the keymap that turns
  `KeyCode` into semantic actions. A page is the only UI type that implements
  `Component`, because it is the only one `App` holds.
- **`components/widgets/`** — a composite that assembles several components into a
  working feature area and holds the state coordinating them (which tab is active, which
  popup is open). It holds the largest component in the project and was missing from the
  earlier agent instructions entirely.
- **`components/features/`** — concrete, domain-aware pieces. They know about
  `PyprojectDependency`, about pinning versions, about package groups.
- **`components/shared/`** — generic and reusable. `SharedTable<T>` handles rendering,
  selection and wrap-around scrolling for any `T: TableRow`; the feature layer supplies
  the `TableRow` impl that formats the domain type. `Popup` and `UviewInput` are
  similarly domain-free.

**Deciding between `shared` and `features`:** if the type mentions a domain type, a `uv`
concept or a specific screen, it belongs in `features`. If it is parameterised over a
trait and would work unchanged in a different TUI, it belongs in `shared`. The reverse
direction is enforced by `tests/architecture.rs` (`shared-no-domain`): nothing in `shared/`
may name a domain type or a controller.

## 7. The `Component` trait

```rust
pub trait Component {
    fn render(&mut self, frame: &mut Frame, area: Rect);
    fn update(&mut self, action: &Action) -> Vec<Command>;
}
```

`&mut self` in `render` is required because ratatui's `TableState` and `ListState` are
mutated during rendering. `update` takes no `&mut AppState` yet: `AppState` arrives with
#43, which adds the parameter together with the data it holds.

**Its real scope today is pages only.** `DependenciesPage` is the sole implementor. The
widget below it has an inherent `update` of the same shape. The pieces below the widget
report to it with plain return values instead of commands (§4). In three cases the
render shape itself differs:

- `Popup::render` returns `Rect` — it clears and centres an area, then hands it back for
  the caller to draw into. It cannot satisfy the trait as written.
- `UviewInput::render` takes `&self`, not `&mut self`.
- `SharedTable` has no `update` at all; navigation is driven by the owning widget
  calling `next()` / `previous()`.

This was a real design question, not an oversight to be silently fixed. `App` only ever
needs the trait in order to hold pages behind a `Box<dyn Component>`, which it does not
yet do because there is only one page.

**Resolved in §13.2.** The trait becomes universal once `render` receives `&AppState`,
because the reason the other components diverged was that each owned its own data.
`Popup` stays outside the trait — its `render` returns a `Rect` because it is a layout
helper, not a component, and forcing it to fit would be the wrong repair.

## 8. Controller contract

A controller function:

- takes plain data (`&str`, `Vec<String>`), plus a `&impl UvClient` if it runs `uv`, and
  returns `Result<T, E>`;
- runs `uv` only through that client, with argv from `uv::args` and output read by
  `uv::parse`, or reads `pyproject.toml` directly;
- returns the types in `domain.rs`;
- **never** renders, holds UI state, or imports ratatui.

`project.rs` is the exception. `main.rs` calls it once, synchronously, before the terminal
is taken over, and it returns plain values rather than a `Result`; `main.rs` prints a
failure to stderr and exits. `main.rs` starts from `UV_PROJECT` when set, else the working
directory. From there `find_project_root` walks up to the nearest `pyproject.toml`, as
`uv` does; an I/O error while looking counts as "no file", as it does for `uv`. `refusal`
then rejects a project `uv` would refuse: no `[project]` table and no workspace, or
`managed = false`.

Output is read in machine-readable form: `uv pip list --format json` is deserialized
with serde into `Vec<EnvDependency>`, and a single package's installed version is found
by filtering that list with PEP 503 name normalization rather than by scraping
`uv pip show`.

`pyproject.toml` is read and parsed directly with the `toml` crate rather than through
`uv`, because the three dependency locations — `[project.dependencies]`,
`[dependency-groups.*]` and `[project.optional-dependencies.*]` — need to be
distinguished, and a single package may appear in several of them. `load_pyproject_deps`
therefore deduplicates by name and accumulates the groups.

Every `uv` invocation goes through `UvClient::run`, which returns stdout or a
`UvError` — `Spawn` (could not start `uv`), `Failed` (non-zero exit, with the command and
stderr) or `Parse` (output unreadable). The runtime formats the error into `Event::Error`.
`RealUvClient` is the only implementation in `src/` and the only code that starts a
process (ADR-0002). `uv` runs in the working directory, which `main.rs` has made the project root, without
`UV_PROJECT`, which that root already accounts for.
`panic!`, `unwrap` and `expect` are denied crate-wide by `[lints.clippy]` in `Cargo.toml`.

## 9. Concurrency

There is one tokio runtime, entered via `#[tokio::main]`, and one `mpsc::UnboundedChannel<Event>`.

- The **event loop is synchronous** and blocking. `App::run` is `async` but never awaits
  inside the loop; `event::poll(50ms)` blocks the thread.
- **Only the runtime spawns.** `runtime::execute` runs each command in a task of its own,
  never awaited in the loop, so a slow `uv` invocation cannot freeze rendering. The task
  reports through a clone of the event sender.
- At startup, before the loop begins, `App` hands the runtime `Command::Reload`, which
  reads `pyproject.toml` and then lists the environment, sending each result or an
  `Event::Error`.
- After a mutation the runtime reloads both tables itself, after a failure too, since a
  failed sequence may already have changed `pyproject.toml`. Update only refreshes its row.

**The redraw is unconditional.** Every iteration calls `terminal.draw` whether or not
anything changed, giving a floor of roughly 20 fps of wasted work when idle. The draw
also happens *before* input is processed, so a key press is reflected one iteration
later. Both are acceptable at the current scale and are addressed by issue #32.

**Spawned handles are dropped.** `runtime::execute` keeps no handle to the task it spawns,
so a task that panics reports nothing. Every outcome a task returns goes through the
channel, failures included.

## 10. Testing

Per repository convention:

- Tests live in `tests/` as integration tests, **not** inline in `src/`.
- `src/lib.rs` exposes the crate so tests can reach items as `uview::...`.
- Anything to be tested must therefore be `pub`.

The consequence for design: **logic worth testing must be separable from process
execution.** Argument construction (`uv::args`) and output parsing (`uv::parse`,
`parse_dep_string`) are pure and tested directly. Operations take a `&impl UvClient`, and
so does `runtime::run`, so `tests/dependencies.rs` runs them against `FakeUvClient`. The fake returns canned stdout
and records the argv it receives, and a test asserts on exactly what would have run. It
lives in `tests/` rather than `src/` because tests are integration tests. Two tests still
run the real `uv` through `RealUvClient`, as ADR-0002 asks, because a fake can drift from
real behaviour.

**Architecture fitness tests.** `tests/architecture.rs` turns the structural rules of
this document into a failing test, using only the standard library:

| Rule | Checks |
|---|---|
| `controllers-no-ratatui` | nothing under `src/controllers/` references ratatui (§2) |
| `controllers-no-process` | nothing under `src/controllers/` starts a process (ADR-0002) |
| `uv-bottom-layer` | nothing under `src/uv/` references ratatui, a controller, the runtime, the messages or the UI (§2, ADR-0002) |
| `runtime-no-ui` | nothing under `src/runtime/` references ratatui or the UI (§2, ADR-0001) |
| `shared-no-domain` | nothing under `src/ui/components/shared/` names a domain type or a controller (§6) |
| `ui-no-spawn` | nothing under `src/ui/` spawns a task or a process, calls a controller or names the uv layer (§2, ADR-0001, ADR-0002) |
| `no-tests-in-src` | no `#[test]` or `#[cfg(test)]` under `src/` (this section) |

Known violations sit in an allowlist, each with the issue that removes it; it is empty
today. The list is a ratchet: the test fails on any new violation, and also when an allowlisted file stops
violating, so the entry must be deleted. Each rule's directory must exist, so moving a
module — `controllers/` becoming `runtime/` in the migration path — fails the
test until its rules are updated in the same PR. The matching is textual, and its limits
are noted in the file.

Unit coverage beyond that is still thin: parsing, argument and requirement construction,
`RealUvClient` in both directions, two operations and one runtime command against the fake,
and the dependencies page driven key by key. The page's `update` returns commands, so those
tests run no runtime and no subprocess. Issue #25 covers expanding it.

## 11. Known deviations and open questions

| # | Item | Tracked |
|---|---|---|
| 1 | ~~Agent instructions describe a module tree that does not exist~~ Resolved: `AGENTS.md` points here instead of restating | #40 |
| 2 | `Component` is implemented by pages only; three components have incompatible signatures | §13.2, during #26 |
| 3 | ~~Controllers `panic!` instead of returning errors~~ Resolved: `UvError` + clippy lints | #1 |
| 4 | ~~`Action::Error` is not shown while the app runs~~ Resolved: a status line at the bottom, dismissed with Esc | #2 |
| 5 | ~~A widget owns controller orchestration and the channel sender~~ Resolved: components return commands, and the runtime spawns | #42 |
| 6 | Unconditional redraw; input processed one frame behind the draw | #32 |
| 7 | Spawned task handles are dropped, so failures vanish | #6 |
| 8 | ~~`uv pip show` output is parsed by line prefix~~ Resolved: filtered from `uv pip list --format json` | #1 |
| 9 | `App` holds a concrete page type, so routing is impossible without a refactor | #26 |
| 10 | ~~Filename `add_packaage_popup.rs` is misspelled~~ Resolved: the add and pin inputs are one `InputPopup` | #8 |

**Open design questions — now answered in Part II**

These were genuinely open when Part I was written. Part II resolves each one; they are
kept here so the reasoning behind the answer is traceable from the code it concerns.

- *Should `Component` become the universal UI contract?* **Yes** — once `render` takes
  `&AppState`, the obstacle disappears. `Popup` is the exception that proves it: its
  `render` returns a `Rect` because it is a layout helper, not a component. See §13.2.
- *Where should controller orchestration live once there are several pages?* **In a
  dedicated runtime below the pages**, not in a widget and not in the page. Components
  return `Command`s and never spawn. See §13.3.
- *Should `Action` stay one flat enum?* **No** — split by direction of travel into
  `Input` / `Action` / `Command` / `Event`, with `Action` nested per domain. See §13.1.

---

# Part II — Target state

## 12. Design forces

Before choosing a shape, be clear about what actually constrains a TUI like this one.
The forces are not the ones a web service faces, and importing that playbook — caching
tiers, sharding, replicas — would be answering the wrong question.

**F1 — The render loop must never block.** Rendering is immediate-mode: the whole frame
is rebuilt from state on every draw. Any blocking call on that thread freezes the entire
application, including the key that would cancel it. This single force does more to
determine the architecture than anything else.

**F2 — Every capability is an external process.** `uview` has no in-process API to call.
There is no library binding for `uv`; there is a binary, argv, exit codes, stdout and
stderr. All I/O is spawn-and-parse, which means the boundary to the outside world is
narrow, uniform, and — importantly — easy to fake in tests.

**F3 — Mutations are serialized by the tool, not by us.** `uv` takes a lock on the
project. Two concurrent `uv add` invocations do not interleave; one fails. Read-only
queries (`pip list`, `tree`, an index search) have no such constraint. So the concurrency
model is fixed for us: **mutations are a queue of one, reads are free.**

**F4 — Operations are slow, and must be observable and interruptible.** A dependency
resolve can take ten seconds. The user needs to see that something is happening, see what
it is doing, and be able to abandon it. `.output()` — which buffers until the process
exits — cannot deliver any of that.

**F5 — Input is modal, and modality is the real complexity.** There is one user and one
key stream, so none of the usual concurrency problems apply to input. What does apply is
that the same key means different things in a table, in a text field, and in a confirm
dialog. Most of the branching in the current code is this, and it is where a TUI's
complexity concentrates.

**F6 — State is tiny but must stay coherent across screens.** Hundreds of rows, never
millions. Scale is a non-problem. Coherence is the problem: after `uv add`, both the
pyproject view and the environment view are stale, and a dependency tree on another page
is stale too.

**F7 — The terminal is a shared and hostile resource.** Raw mode and the alternate screen
are global process state. A panic inside a spawned task fires ratatui's panic hook,
which restores the terminal — and then the app keeps running and drawing into a terminal
that is no longer in raw mode. A child process that writes to the inherited stdout
corrupts the frame the same way.

**What the forces imply.** F1 and F4 mean side effects cannot be executed where they are
decided. F3 means there must be a component that owns scheduling. F2 means there should
be exactly one seam to the outside, and that seam should be a trait. F5 means input
routing deserves a first-class model instead of nested `if is_editing()`. F6 means domain
data belongs in one place, not inside the widgets that happen to display it. F7 means no
`panic!` on any reachable path, plus a hook for the unreachable ones.

## 13. Target design

### 13.1 Four message types, not one enum

The current flat `Action` mixes raw input, user intent and effect results. Separate them
by direction of travel:

```rust
enum Input   { Key(KeyEvent), Resize(u16, u16), Paste(String) }  // from the terminal
enum Action  { Deps(DepsAction), Project(ProjectAction), Nav(NavAction), .. }  // intent, downward
enum Command { LoadEnvDeps, Add { req: Requirement, group: Option<String> }, .. }  // requested effect, upward
enum Event   { EnvDepsLoaded(Vec<EnvDependency>), JobOutput(JobId, String), JobFailed(JobId, UvError), .. }
```

`Input` is translated to `Action` at the edge by a keymap. `Action` flows down the
component tree. `Command` flows back up as a *request* — the component decides what
should happen and never performs it. `Event` re-enters as new input.

Nesting `Action` per domain is what keeps this from becoming a 90-variant enum by
Sprint 6.

### 13.2 Components return commands; they do not spawn

```rust
pub trait Component {
    fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState);
    fn update(&mut self, action: &Action, state: &mut AppState) -> Vec<Command>;
}
```

Two changes carry most of the value:

- **`&AppState` is passed in.** Domain data moves out of widgets, so a widget holds only
  view state — selection index, scroll offset, whether its input is focused. `&mut self`
  survives only because `TableState` and `ListState` are mutated during rendering, which
  is a ratatui fact and not a design choice.
- **`Vec<Command>` replaces `tokio::spawn`.** No component owns an `mpsc` sender, and no
  component knows tokio exists. This makes `update` testable by calling it and asserting
  on the commands it returns — no runtime, no subprocess, no terminal.

This also settles the open question from §7. `Popup` fails the trait today because
`render` returns a `Rect`, but that is the correct signal: `Popup` is not a component at
all, it is a layout helper (`centered_rect` plus a `Clear`). Reclassify it as one, and
the trait fits everything that remains.

### 13.3 A runtime that executes commands

One place converts `Command` into work and work into `Event`:

```
Input ─▶ keymap ─▶ Action ─▶ page.update() ─▶ Vec<Command> ─▶ Runtime
                      ▲                                          │
                      └────────────── Event ◀────────────────────┘
```

The runtime owns the `UvClient`, the job queue and the cancellation tokens. It is the
only module that spawns tasks. Everything above it is synchronous and testable.

> Decision record for §13.1–13.3: [ADR-0001](adr/0001-components-return-commands.md).

### 13.4 `UvClient`: one seam to the outside

```rust
pub trait UvClient: Send + Sync {
    fn run(&self, args: &[&str]) -> impl Future<Output = Result<String, UvError>> + Send;
}
```

The seam sits at the narrowest point there is: argv in, stdout or a `UvError` out.
`RealUvClient` spawns the binary. `FakeUvClient` returns canned stdout and records the argv
it received, so a test asserts on exactly what would have run. Operations take the client
as `&impl UvClient`: static dispatch, with no `async_trait`.

Argument construction must be **pure and separate** from execution:

```rust
pub fn membership_args<'a>(verb: &'a str, target: &'a str, membership: &'a Membership) -> Vec<&'a str>;
```

so the interesting logic can be asserted without spawning anything — which is exactly
what the repository's testing convention (integration tests in `tests/`, public items via
`src/lib.rs`) requires.

Prefer `uv`'s machine-readable output wherever it exists (`--format json`,
`--output-format json`). Where line-prefix scraping is ever unavoidable, it is a
liability against future `uv` releases and should be isolated in `uv/parse.rs` so a
single module absorbs the breakage.

> Decision records: [ADR-0002](adr/0002-uvclient-is-the-only-seam-to-uv.md) for the seam,
> [ADR-0005](adr/0005-uvclient-takes-argv.md) for its shape.

### 13.5 Job runner: a queue of one for mutations

Direct consequence of **F3**:

```
Commands ─┬─▶ read-only  ─▶ spawned freely, concurrent
          └─▶ mutating   ─▶ single-consumer queue, strictly serialized
```

Every job carries an id, a human label, a `CancellationToken` and a status. This one
component supplies four things the roadmap needs separately: the progress indicator
(#13), cancellation, the guarantee that two mutations never overlap, and one honest place
to put retries if they are ever wanted.

A mutating job completing emits an invalidation event, which is how **F6** is answered:
views do not each remember to refresh, they react to `Event::ProjectMutated` and reload.

> Decision record: [ADR-0003](adr/0003-serialize-mutating-uv-operations.md).

### 13.6 Stream output, do not buffer it

Replace `Command::output()` — which waits for exit and returns everything at once — with
a spawned child, piped stdout/stderr, and a reader task emitting
`Event::JobOutput(JobId, String)` per line.

Buffering is the reason progress reporting is impossible today. `uv build` (#30),
`uv sync` (#10) and long resolves are unusable without this.

### 13.7 State ownership

```rust
pub struct AppState {
    pub project:  ProjectState,   // name, version, root path
    pub deps:     DepsState,      // pyproject + environment
    pub python:   PythonState,
    pub jobs:     JobsState,      // running and recent
    pub messages: MessageLog,     // errors and notices
}
```

Domain data lives here. View state stays in components.

**What this does and does not buy.** It does not guarantee consistency. The source of
truth is the project on disk and the virtualenv; `uv` is the only writer, and everything
`uview` holds in memory is a cache of it. `AppState` does not make the cache correct — it
reduces the number of caches from one-per-widget to one, so invalidation becomes a single
operation instead of a fan-out that each new widget must remember to join. Correctness
comes from the invalidation discipline in §13.5, not from the struct.

What it does buy is that domain data outlives the widget displaying it. Leaving a page and
returning must not drop loaded data, and two views — the pyproject table and the dependency
tree — may legitimately show the same underlying facts.

**Load state is not a `Vec`.** An empty `Vec<T>` cannot distinguish "not loaded yet" from
"loaded and genuinely empty" from "the load failed", and today all three render as an
identical empty table. Make it explicit:

```rust
pub enum Loadable<T> {
    NotLoaded,
    Loading { since: Instant },
    Loaded { value: T, generation: u64 },
    Failed(UvError),
}
```

`generation` is not decoration. Reloads are spawned, so they can return out of order: a
reload issued before a mutation can land *after* one issued in response to it, overwriting
fresh data with stale. One cache instead of five does not fix that race. Discard any
result whose generation is older than the current one.

**Keep it behaviour-free.** `AppState` is a data holder; it gets no `impl` block carrying
business logic. Guard the god-object failure mode with per-domain sub-structs that own
their own behaviour, and by never letting a component reach across domains — a dependency
widget touches `state.deps`, not `state.python`.

> Decision record, including why this lands in Sprint 1 rather than Sprint 4:
> [ADR-0004](adr/0004-introduce-appstate-ahead-of-need.md).

### 13.8 Navigation as a stack

```rust
pages: Vec<Box<dyn Component>>,   // the last one is active
```

A stack rather than a single field, because "go back" is then free and modals compose
naturally with pages. Global keys — quit, help, cancel job — are handled above the stack
so every page inherits them without repeating the match.

### 13.9 Errors

```rust
pub enum UvError {
    Spawn(std::io::Error),                                   // uv not installed, not executable
    Failed { command: String, status: ExitStatus, stderr: String }, // uv ran and refused
    Parse(String),                                           // output we could not read
    Cancelled,                                               // the user abandoned it — arrives with #13
}
```

Rules, in force everywhere: **no `panic!`, `unwrap` or `expect` on any path reachable
from a user action.** Failures become `Event::JobFailed` and are shown in the message
area. As defence in depth against **F7**, a panic hook restores the terminal before
unwinding. `ratatui::init()` installs it, so it must be called after
`color_eyre::install()` or the eyre hook replaces it.

### 13.10 Render only when something changed

Target loop:

```rust
loop {
    tokio::select! {
        Some(input) = input_rx.recv() => { dispatch(keymap(input)); }
        Some(event) = event_rx.recv() => { dispatch(event.into()); }
        _ = tick.tick()               => { /* animate spinners only */ }
    }
    if state.dirty { terminal.draw(..)?; state.dirty = false; }
}
```

Input arrives on its own thread and is received rather than polled, so nothing blocks
the loop (**F1**), input is no longer processed a frame behind the draw, and an idle
application costs nothing instead of redrawing twenty times a second.

## 14. Target module tree

```
src/
├── main.rs
├── lib.rs
├── app/
│   ├── mod.rs           # App: the select! loop, the page stack
│   ├── state.rs         # AppState and its per-domain sub-structs
│   └── dispatch.rs      # Action routing; no recursion, an explicit work queue
├── message.rs           # Input / Action / Command / Event
├── runtime/
│   ├── mod.rs           # Command → work → Event; the only module that spawns
│   ├── jobs.rs          # job queue, ids, cancellation, serialization of mutations
│   └── stream.rs        # line-wise child output
├── uv/
│   ├── mod.rs           # UvClient trait; RealUvClient, the only process spawn
│   ├── args.rs          # pure argument construction
│   └── parse.rs         # pure output parsing
├── domain/
│   ├── dependency.rs    # Requirement, extras, constraints (PEP 508)
│   ├── project.rs
│   └── python.rs
└── ui/
    ├── mod.rs           # Component trait
    ├── keymap.rs        # Input × Mode → Action, one table, no nested ifs
    ├── pages/
    └── components/{shared,features,widgets}/
```

Two things worth naming. `controllers/` becomes `uv/` plus `runtime/`, because the old
name described a layer that did two unrelated jobs: talking to `uv`, and deciding when.
And `domain/` appears because `Requirement` — name, extras, constraint, source — is a real
type that `domain.rs` currently flattens into two `String`s, which is why extras (#18)
cannot be represented today.

The fake `UvClient` is missing from the tree on purpose. Tests are integration tests, so
it lives in `tests/`.

## 15. Migration path

Nothing here requires a rewrite. Each step is a normal pull request, and each is already
an issue.

| Order | Step | Unlocks | Issues |
|---|---|---|---|
| 1 | `UvError`, no panics, panic hook | Everything below is safe to build on | #1 |
| 2 | Message area; errors become visible | Honest failure reporting | #2 |
| 3 | Extract `uv/args.rs` + `uv/parse.rs` as pure functions | Tests without subprocesses | #41, #25 |
| 4 | Introduce `UvClient` + `FakeUvClient` | The whole uv layer becomes testable | **#41** |
| 5 | `Command`/`Event` split; components stop spawning | Removes the widget-owns-the-sender deviation | **#42** |
| 6 | `runtime/` + job queue + cancellation | Progress, interruption, safe mutation | #13, #10 |
| 7 | Streamed child output | Build and sync become usable | #30, #10 |
| 8 | `AppState` extracted from widgets | Coherent data across pages | **#43** |
| 9 | Page stack + `keymap.rs` | Multi-page app; modal input stops sprawling | #26, #14 |
| 10 | `domain::Requirement` | Extras, sources, groups | #18, #19, #17 |
| 11 | Dirty-flag rendering, input thread | Idle cost, input latency | #32 |

Steps 1–4 are Sprint 0 work and should land in that order; nothing after step 4 is worth
starting before the seam exists.

Steps 7, 9, 10 and 11 have no issue of their own: they are acceptance criteria inside the
feature issues that need them (#30 requires streaming to be usable, #26 requires the page
stack, #18 requires a real requirement type, #32 requires dirty-flag rendering). That is
deliberate — they are not separable pieces of work. Steps 4, 5 and 8 do have their own
issues, because each is a refactor no single feature justifies on its own yet every later
feature depends on.

## 16. Trade-offs

**What this costs.** The `Command`/`Event` split adds a layer of indirection: a component
can no longer just do the thing. For a single-screen app that is pure overhead, and if
`uview` were staying at one page the current direct-spawn design would be the right
answer. It stops being overhead at the point where two pages need the same data and two
operations can be in flight, which the sprint plan reaches in Sprint 1.

**Where it can go wrong.** `AppState` is a god object waiting to happen; the mitigation is
per-domain sub-structs and a rule that components stay in their own. A trait for
`UvClient` costs a generic parameter on every operation and a second implementation to
keep honest — negligible against being able to test at all. Serializing mutations gives up parallelism we never
had, since `uv`'s project lock forbids it anyway.

**What we are deliberately not doing.** No plugin system, no configuration language, no
persistence layer, no daemon. `uview` holds no state that outlives the process; the
project on disk is the source of truth and `uv` is the only writer. Keeping it that way
is what makes the design small.

**What to revisit.** If `uv` ever ships a stable machine-readable interface or a library,
`RealUvClient` should collapse onto it and `parse.rs` should mostly disappear. If pages grow
past roughly a dozen, the single `AppState` should be split per page with explicit
sharing. And if `Action` passes about fifty variants even when nested, that is the signal
to give each page its own message type with a translation at the boundary, rather than
continuing to grow one tree.
