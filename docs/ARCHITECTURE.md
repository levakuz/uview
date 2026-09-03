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

The design follows the Elm architecture (TEA): a single `Action` type describes
everything that can happen, components translate input into actions, and controllers
perform side effects and report back as further actions.

## 2. Layers

There are two layers, plus a thin application shell.

| Layer | Location | Responsibility | Must not |
|---|---|---|---|
| **App shell** | `src/app.rs` | Owns the terminal, the event loop, the action channel and the active page | Contain feature logic |
| **UI** | `src/ui/` | Render frames, translate input into actions, hold view state | Spawn `uv`, read the filesystem |
| **Controllers** | `src/controllers/` | Invoke `uv`, parse its output, read `pyproject.toml` | Render, hold UI state, know about ratatui |

The rule that matters: **controllers never import ratatui, and UI code never invokes
`uv` directly.** The one place these meet is a component spawning a controller call onto
tokio and forwarding the result over the action channel.

> **Current deviation.** `DependenciesTablesTabs` (a widget) both spawns controller calls
> and owns the sender. That keeps `uv` invocation out of the render path but pushes
> orchestration down into a widget. See §9.

## 3. Data flow

```
                    ┌──────────────────────────────────────────────┐
                    │              App::run  (app.rs)              │
                    │                                              │
   key press ──────▶│  crossterm::event::poll / read               │
                    │        │                                     │
                    │        ▼                                     │
                    │  Action::ButtonPressed(KeyEvent)             │
                    │        │                                     │
                    │        ▼                                     │
                    │  App::dispatch(action) ◀──────────┐          │
                    │        │                          │          │
                    │        ▼                          │ recursion│
                    │  page.handle_event(&action)       │          │
                    │        │                          │          │
                    │        └── Option<Action> ────────┘          │
                    │                                              │
                    │  rx.try_recv()  ◀── mpsc ── spawned tasks    │
                    │        │                          ▲          │
                    │        ▼                          │          │
                    │  App::dispatch(action)            │          │
                    │                                   │          │
                    │  terminal.draw(page.render)       │          │
                    └───────────────────────────────────┼──────────┘
                                                        │
                    ┌───────────────────────────────────┴──────────┐
                    │      controllers::*  (tokio::process)        │
                    │      uv add / remove / lock / sync / …       │
                    └──────────────────────────────────────────────┘
```

The concrete path for a key press, using "pin a version" as the example:

1. `App::run` polls crossterm and wraps the key as `Action::ButtonPressed(KeyEvent)`.
2. `App::dispatch` forwards it to `DependenciesPage::handle_event`.
3. The page maps `KeyCode` to a semantic action — `Enter` becomes `Action::ConfirmInput`.
4. The action reaches `DependenciesTablesTabs`, which routes it to whichever popup is
   open and editing.
5. `PinVersionPopup` takes the input value and returns `Action::PinVersion(String)`.
6. That returned action is dispatched recursively (§4), lands back on the widget, which
   spawns `controllers::dependencies::pin_dep_version` onto tokio.
7. The spawned task sends `Action::UpdatedEnvDep(EnvDependency)` through the `mpsc`
   sender it was given.
8. The next loop iteration drains the receiver, dispatches that action, and the widget
   updates the table row.
9. The following `terminal.draw` renders the new state.

## 4. Actions and dispatch

`Action` (`src/events.rs`) is a single flat enum covering three distinct kinds of thing:

- **Raw input** — `ButtonPressed(KeyEvent)`
- **Semantic intent** — `MoveUp`, `SwitchTable`, `OpenPopup`, `RemovePackage`, `PinVersion(String)`
- **Results from controllers** — `LoadedPyprojectDeps(..)`, `LoadedEnvDeps(..)`, `UpdatedEnvDep(..)`, `Error(String)`

Actions enter the system through two doors, and both converge on `App::dispatch`:

- **Synchronously**, from the crossterm poll.
- **Asynchronously**, through an `mpsc::UnboundedSender<Action>` cloned into components
  that need to report results.

`App::dispatch` handles `Quit` and `Error` itself and passes everything else to the
active page. If `handle_event` returns `Some(next)`, dispatch **calls itself** with that
action. This is how a component escalates: `PinVersionPopup` returns `Action::PinVersion`
and the widget above it receives it on the next pass.

> **Hazard.** The recursion has no depth limit and no cycle detection. A component that
> returns the same action it was handed will recurse until the stack overflows. Keep the
> rule: `handle_event` returns an action that is *different in kind* from the one it
> received, or returns `None`.

## 5. Module map

```
src/
├── main.rs                 # tokio entry point, color_eyre install
├── lib.rs                  # public surface, so tests/ can reach controllers and ui
├── app.rs                  # App: event loop, channel, dispatch, active page
├── events.rs               # Action enum + PyprojectDependency / EnvDependency
├── controllers/
│   └── dependencies.rs     # uv add/remove/lock/sync/pip, pyproject.toml parsing
└── ui/
    ├── mod.rs              # Component trait
    ├── pages/
    │   └── dependencies.rs # DependenciesPage — keymap and top-level layout
    └── components/
        ├── shared/         # generic, feature-agnostic
        │   ├── table.rs    #   SharedTable<T: TableRow>
        │   ├── input.rs    #   UviewInput (wraps tui-input)
        │   └── popup.rs    #   Popup — centering and Clear only
        ├── features/       # concrete, tied to this domain
        │   ├── dependencies_table.rs      # TableRow impls for the two dep types
        │   ├── add_packaage_popup.rs      # [sic] misspelled filename
        │   ├── package_control_popup.rs
        │   └── pin_version_popup.rs
        └── widgets/
            └── dependencies_tables_tabs.rs # composes tables + popups, owns the tx
```

**Planned, not yet written.** `controllers/project.rs`, `controllers/python.rs`,
`ui/pages/home.rs`, `ui/pages/python.rs`. `CLAUDE.md` lists these as if they exist; they
do not.

## 6. UI component taxonomy

Four kinds of UI code, in descending order of generality:

- **`pages/`** — a full screen. Owns the top-level layout and the keymap that turns
  `KeyCode` into semantic actions. A page is the only UI type that implements
  `Component`, because it is the only one `App` holds.
- **`components/widgets/`** — a composite that assembles several components into a
  working feature area and holds the state coordinating them (which tab is active, which
  popup is open). This directory is not mentioned in `CLAUDE.md` but holds the largest
  component in the project.
- **`components/features/`** — concrete, domain-aware pieces. They know about
  `PyprojectDependency`, about pinning versions, about package groups.
- **`components/shared/`** — generic and reusable. `SharedTable<T>` handles rendering,
  selection and wrap-around scrolling for any `T: TableRow`; the feature layer supplies
  the `TableRow` impl that formats the domain type. `Popup` and `UviewInput` are
  similarly domain-free.

**Deciding between `shared` and `features`:** if the type mentions a domain type, a `uv`
concept or a specific screen, it belongs in `features`. If it is parameterised over a
trait and would work unchanged in a different TUI, it belongs in `shared`.

## 7. The `Component` trait

```rust
pub trait Component {
    fn render(&mut self, frame: &mut Frame, area: Rect);
    fn handle_event(&mut self, action: &Action) -> Option<Action>;
}
```

`&mut self` in `render` is required because ratatui's `TableState` and `ListState` are
mutated during rendering.

**Its real scope today is pages only.** `DependenciesPage` is the sole implementor.
Every other component exposes inherent `render` / `handle_event` methods with the same
shape but no trait, and in three cases the shape itself differs:

- `Popup::render` returns `Rect` — it clears and centres an area, then hands it back for
  the caller to draw into. It cannot satisfy the trait as written.
- `UviewInput::render` takes `&self`, not `&mut self`.
- `SharedTable` has no `handle_event` at all; navigation is driven by the owning widget
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

- takes plain data (`&str`, `Vec<String>`) and returns `Result<T, E>`;
- invokes `uv` through `tokio::process::Command`, or reads `pyproject.toml` directly;
- parses output into the types in `events.rs`;
- **never** renders, holds UI state, or imports ratatui.

Two ways of talking to `uv` are in use, and the distinction is deliberate:

- **Structured output where `uv` offers it** — `uv pip list --format json` is
  deserialized with serde into `Vec<EnvDependency>`. Prefer this.
- **Line parsing where it does not** — `uv pip show` output is scanned for `Name: ` and
  `Version: ` prefixes. This is brittle against `uv` releases and should be replaced
  whenever a JSON form becomes available.

`pyproject.toml` is read and parsed directly with the `toml` crate rather than through
`uv`, because the three dependency locations — `[project.dependencies]`,
`[dependency-groups.*]` and `[project.optional-dependencies.*]` — need to be
distinguished, and a single package may appear in several of them. `load_pyproject_deps`
therefore deduplicates by name and accumulates the groups.

> **Current deviation.** Controllers signal failure with `panic!`, which kills the
> process while the terminal is in raw mode and leaves the user's shell broken. The
> contract above is what controllers *should* do; see issue #1.

## 9. Concurrency

There is one tokio runtime, entered via `#[tokio::main]`, and one `mpsc::UnboundedChannel<Action>`.

- The **event loop is synchronous** and blocking. `App::run` is `async` but never awaits
  inside the loop; `event::poll(50ms)` blocks the thread.
- **Controller calls are spawned**, never awaited in the loop, so a slow `uv` invocation
  cannot freeze rendering.
- Two loads are spawned at startup, before the loop begins: `load_pyproject_deps` and
  `load_env_deps`, each sending its result or an `Action::Error`.
- Later calls are spawned from `DependenciesTablesTabs`, which holds a cloned sender for
  exactly this purpose.

**The redraw is unconditional.** Every iteration calls `terminal.draw` whether or not
anything changed, giving a floor of roughly 20 fps of wasted work when idle. The draw
also happens *before* input is processed, so a key press is reflected one iteration
later. Both are acceptable at the current scale and are addressed by issue #32.

**Spawned handles are dropped.** Several call sites do `let _ = handle.spawn(async { ... })`,
so a task that fails reports nothing — clippy flags this as `let_underscore_future`.
Anything spawned should report its outcome through the channel, including failure.

## 10. Testing

Per repository convention:

- Tests live in `tests/` as integration tests, **not** inline in `src/`.
- `src/lib.rs` exposes the crate so tests can reach items as `uview::...`.
- Anything to be tested must therefore be `pub`.

The consequence for design: **logic worth testing must be separable from process
execution.** Pure functions — `parse_dep_string` today, argument construction and output
parsing in future — are testable directly. Functions that spawn `uv` are not, so keep
the pure part extracted rather than embedding string building inside the spawn.

Current coverage is one test, for `parse_dep_string`. Issue #25 covers expanding it.

## 11. Known deviations and open questions

| # | Item | Tracked |
|---|---|---|
| 1 | `CLAUDE.md` lists modules that do not exist and omits `components/widgets/` | #40 |
| 2 | `Component` is implemented by pages only; three components have incompatible signatures | §13.2, during #26 |
| 3 | Controllers `panic!` instead of returning errors | #1 |
| 4 | `Action::Error` is printed with `eprintln!` over the live TUI | #2 |
| 5 | A widget owns controller orchestration and the channel sender | §13.3, during #8 / #26 |
| 6 | Unconditional redraw; input processed one frame behind the draw | #32 |
| 7 | Spawned task handles are dropped, so failures vanish | #6 |
| 8 | `uv pip show` output is parsed by line prefix | §8 |
| 9 | `App` holds a concrete page type, so routing is impossible without a refactor | #26 |
| 10 | Filename `add_packaage_popup.rs` is misspelled | #8 |

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
are global process state. A panic, or a child process that writes to the inherited
stdout, corrupts the user's shell after `uview` exits.

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

### 13.4 `UvClient`: one seam to the outside

```rust
#[async_trait]
pub trait UvClient: Send + Sync {
    async fn pip_list(&self) -> Result<Vec<EnvDependency>, UvError>;
    async fn add(&self, req: &Requirement, group: Option<&str>) -> Result<(), UvError>;
    async fn remove(&self, name: &str, group: Option<&str>) -> Result<(), UvError>;
    async fn sync(&self) -> Result<(), UvError>;
    // …
}
```

`RealUvClient` spawns the binary. `FakeUvClient` returns canned results and records the
calls it received. This is the change that makes the codebase testable at all: today
every function that touches `uv` is untestable because the subprocess call is inline.

Argument construction must be **pure and separate** from execution:

```rust
pub fn add_args(req: &Requirement, group: Option<&str>) -> Vec<String>;
```

so the interesting logic can be asserted without spawning anything — which is exactly
what the repository's testing convention (integration tests in `tests/`, public items via
`src/lib.rs`) requires.

Prefer `uv`'s machine-readable output wherever it exists (`--format json`,
`--output-format json`). Line-prefix scraping, as `uv pip show` needs today, is a
liability against future `uv` releases and should be isolated behind this trait so a
single module absorbs the breakage.

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

**Why this lands in Sprint 1 rather than Sprint 4.** The benefit — coherence across pages
— does not materialize until routing exists (#26, Sprint 4). The *cost* of the extraction,
however, scales with the number of widgets that own data, and that number only grows. Two
tables and three popups is the cheapest this refactor will ever be.

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
    Spawn(std::io::Error),                       // uv not installed, not executable
    Failed { code: Option<i32>, stderr: String },// uv ran and refused
    Parse(String),                               // output we could not read
    Cancelled,                                   // the user abandoned it
    NotFound(String),
}
```

Rules, in force everywhere: **no `panic!`, `unwrap` or `expect` on any path reachable
from a user action.** Failures become `Event::JobFailed` and are shown in the message
area. As defence in depth against **F7**, a panic hook restores the terminal before
unwinding, so even a genuine bug leaves a usable shell.

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
│   ├── client.rs        # UvClient trait
│   ├── real.rs          # subprocess implementation
│   ├── fake.rs          # #[cfg(test)] implementation
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
type that `events.rs` currently flattens into two `String`s, which is why extras (#18)
cannot be represented today.

## 15. Migration path

Nothing here requires a rewrite. Each step is a normal pull request, and each is already
an issue.

| Order | Step | Unlocks | Issues |
|---|---|---|---|
| 1 | `UvError`, no panics, panic hook | Everything below is safe to build on | #1 |
| 2 | Message area; errors become visible | Honest failure reporting | #2 |
| 3 | Extract `uv/args.rs` + `uv/parse.rs` as pure functions | Tests without subprocesses | #25 |
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
`UvClient` costs dynamic dispatch and a second implementation to keep honest — negligible
against being able to test at all. Serializing mutations gives up parallelism we never
had, since `uv`'s project lock forbids it anyway.

**What we are deliberately not doing.** No plugin system, no configuration language, no
persistence layer, no daemon. `uview` holds no state that outlives the process; the
project on disk is the source of truth and `uv` is the only writer. Keeping it that way
is what makes the design small.

**What to revisit.** If `uv` ever ships a stable machine-readable interface or a library,
`uv/real.rs` should collapse onto it and `parse.rs` should mostly disappear. If pages grow
past roughly a dozen, the single `AppState` should be split per page with explicit
sharing. And if `Action` passes about fifty variants even when nested, that is the signal
to give each page its own message type with a translation at the boundary, rather than
continuing to grow one tree.
