# uview

A terminal UI for [uv](https://github.com/astral-sh/uv), the Python package manager.

uview shows two views of a project:

- the dependencies declared in `pyproject.toml`;
- the packages actually installed in its environment.

From either view you can pin, update or remove a package without remembering uv's
command-line syntax. uview has no packaging logic of its own: every action runs the `uv`
binary.

![uview showing a project's pyproject.toml dependencies, just after pinning httpx](docs/screenshot.svg)

**Status: early development**, not yet a daily driver.

## Requirements

- [uv](https://docs.astral.sh/uv/getting-started/installation/) on `PATH`. uview is
  tested with uv 0.8.
- Rust, installed through [rustup](https://rustup.rs). The toolchain is pinned in
  `rust-toolchain.toml`, and rustup installs it on the first build.

## Install

```bash
git clone https://github.com/levakuz/uview.git
cd uview
cargo install --path . --locked
```

This puts the `uview` binary in `~/.cargo/bin`.

## Usage

Run `uview` anywhere inside a uv project:

```bash
cd path/to/your-project
uview
```

**How it finds the project.** uview walks up from the working directory to the nearest
`pyproject.toml`, as uv does, and runs every uv command from that directory. When
`UV_PROJECT` is set, it starts from there instead.

It prints why and exits instead of starting when:

- there is no `pyproject.toml` in the directory or any parent;
- the `pyproject.toml` has neither a `[project]` table nor `[tool.uv.workspace]`;
- the `pyproject.toml` sets `[tool.uv] managed = false`.

**The two tables.**

- **pyproject.toml** lists the declared dependencies. For each one it shows the group it
  belongs to (`main`, a dependency group, or `extra:<name>`) and its version constraint.
- **Environment** lists what is installed in the project's environment. That is `.venv` at
  the project root, or the path in `UV_PROJECT_ENVIRONMENT`, not whichever virtualenv is
  activated. If the environment does not exist yet, run `uv sync` once.

**The bottom line** shows the latest error or notice on the left and the project root on
the right.

### Actions

Press `Space` or `Enter` on a package to open its actions:

| Action | What it does |
|---|---|
| Pin to version | Asks for a constraint, then writes it to `pyproject.toml` with `uv add` in every group the package is declared in, and syncs. `1.2.3` becomes `==1.2.3`; `>=2.0` and `<3` are kept as typed. A package that is installed but not declared is added as a main dependency. |
| Update | Runs `uv sync --upgrade-package`: the newest version the constraints allow. |
| Remove | **pyproject.toml table:** runs `uv remove` for every group and extra the package is in, then syncs.<br>**Environment table:** runs `uv pip uninstall` on the environment only. `pyproject.toml` and `uv.lock` are untouched, so the next `uv sync` brings a declared package back. |
| Cancel | Closes the list. |

To add a package, press `p`, type a requirement such as `requests` or `httpx>=0.27`, and
press `Enter`. uview adds it to `[project.dependencies]` with `uv add` and syncs.

Syncs run `uv sync --inexact`, which keeps installed packages that are not in the lockfile.

**Progress.** While an operation runs, the bottom line names it after a spinner, with how
many wait behind it. uv locks the project, so uview runs one change at a time, in the order
you asked for them. After each, both tables are read again.

**Cancelling.** `Ctrl+C` cancels the running operation and those waiting. On macOS and
Linux, uview stops `uv` as Ctrl+C would in a shell: an add or a pin cancelled while `uv`
resolves it leaves `pyproject.toml` as it was. Cancelled at another step, or on Windows, an
operation may already have changed the project. Either way, the tables then show what is
there. Quitting cancels the same way.

### Keybindings

| Key | Action |
|---|---|
| `↑` / `k`, `↓` / `j` | Move the selection. Table selections wrap around at the ends. |
| `Tab`, `[`, `]` | Switch between the pyproject.toml and Environment tables. |
| `Space`, `Enter` | Open the actions for the selected package. In the actions list, run the highlighted one. |
| `Enter` | In an input, confirm what you typed: a version for **Pin to version**, or a package to add. |
| `Esc` | Close a popup or cancel input, and dismiss the message on the bottom line. |
| `p` | Add a package. |
| `Ctrl+C` | Cancel the running operation and those waiting behind it, in an input too. |
| `q` | Quit, cancelling whatever is running first. |

While you type into an input, every key except `Enter`, `Esc` and `Ctrl+C` goes into the
field.
