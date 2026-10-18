# uview

A terminal UI for [uv](https://github.com/astral-sh/uv), the Python package manager.

uview shows two views of a project:

- the dependencies declared in `pyproject.toml`;
- the packages actually installed in its environment.

From either view you can pin, update or remove a package without remembering uv's
command-line syntax. uview has no packaging logic of its own: every action runs the `uv`
binary, except deleting a group a removal left empty, which uv has no command for.

![uview showing a project's pyproject.toml dependencies beside the uv log, just after pinning httpx](docs/screenshot.svg)

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

**Outside a project**, with no `pyproject.toml` in the directory or any parent, uview offers
to create one there with `uv init`. It asks first, then for the project's name, which defaults
to the directory's, and shows what uv prints. It then opens the new project:

```
uview: no pyproject.toml in "/home/jane/acme" or any parent directory.
Create a uv project in "/home/jane/acme"? [y/N] y
Project name [acme]:
Initialized project `acme` at `/home/jane/acme`
```

Without a terminal to ask on, it prints why and exits instead, as it does when:

- the `pyproject.toml` has neither a `[project]` table nor `[tool.uv.workspace]`;
- the `pyproject.toml` sets `[tool.uv] managed = false`.

**The two tables.**

- **pyproject.toml** lists the declared dependencies. For each one it shows the group it
  belongs to (`main`, a dependency group, or `extra:<name>`) and its version constraint.
- **Environment** lists what is installed in the environment `uv sync` uses for the project,
  not whichever virtualenv is activated. That is `.venv`, or the path in
  `UV_PROJECT_ENVIRONMENT`, at the project root, or at the workspace root for a member of a uv
  workspace. If the environment does not exist yet, run `uv sync` once.

Until a table has loaded, it says so, and if it cannot load, it says why.

**The right column** shows the project at the top: its name and version, as `uv version`
reports them, in the tile's border, and its root. If uv cannot read the version, a dynamic
one for instance, the tile shows what uv said instead. Below it is the uv log: each
operation's name, then the lines uv printed while it ran.

**The bottom lines.** The first shows what is running and the latest error or notice. The
last lists the main keys that work where you are: in a table, in the actions list or in an
input. The arrows, `P`, `i` and `q` are left out for room. Press `?` for every key, by where it
works.

### Actions

Press `Space` or `Enter` on a package to open its actions:

| Action | What it does |
|---|---|
| Pin to version | Lists the versions the package's index has, newest first: pre-releases marked `pre`, a release that needs a newer Python than the project supports marked with what it needs, yanked releases left out. The index is the one uv would use: the package's pinned index, else the project's indexes, then PyPI. Choosing one then asks how to pin it: `==` (highlighted), `>=`, `~=` (compatible: `>=1.17.0, <1.18`), `==1.17.*` (that series), `>`, `<=`, `<` or `!=`, each shown with what it means; only those PEP 440 allows for that version are offered. **Type a constraint…**, first in the list, asks for a constraint instead, then writes it to `pyproject.toml` with `uv add` in every group the package is declared in, and syncs. `1.2.3` becomes `==1.2.3`; `>=2.0` and `<3` are kept as typed. A package that is installed but not declared is added as a main dependency. |
| Update | Runs `uv sync --upgrade-package`: the newest version the constraints allow. |
| Remove | **pyproject.toml table:** runs `uv remove` for every group and extra the package is in, which syncs: the package leaves the environment too, with any dependency nothing else needs.<br>**Environment table:** runs `uv pip uninstall` on the environment only. `pyproject.toml` and `uv.lock` are untouched, so a declared package comes back with the next sync, which every other action runs. |
| Cancel | Closes the list. |

To add a package, press `p`, type a requirement such as `requests` or `httpx>=0.27`, and
press `Enter`. uview adds it to `[project.dependencies]` with `uv add` and syncs.

To sync the environment, press `s`. uview runs plain `uv sync`.

uview runs the `uv` commands you would type yourself, and each action leaves the environment
as its command would. `uv add`, behind Add and Pin, installs what it needs and leaves other
packages alone. `uv remove`, behind Remove in the pyproject.toml table, and `uv sync`, behind
`s` and Update, make the environment match the lockfile exactly: they also uninstall packages
it does not list, such as ones installed with `uv pip install`.

**Progress.** While an operation runs, the bottom line names it after a spinner, with how
many wait behind it, and the uv log shows what uv prints as it goes. uv locks the project,
so uview runs one change at a time, in the order you asked for them. After each, both
tables are read again.

**Cancelling.** `Ctrl+C` cancels the running operation and those waiting. On macOS and
Linux, uview stops `uv` as Ctrl+C would in a shell: an add or a pin cancelled while `uv`
resolves it leaves `pyproject.toml` as it was. Cancelled at another step, or on Windows, an
operation may already have changed the project. Either way, the tables then show what is
there. Quitting cancels the same way.

### Keybindings

| Key | Action |
|---|---|
| `↑` / `k`, `↓` / `j` | Move the selection. Table selections wrap around at the ends, and stay on their package when the tables are read again. |
| `Tab`, `[`, `]` | Switch between the pyproject.toml and Environment tables. |
| `Space`, `Enter` | Open the actions for the selected package. In the actions list, run the highlighted one. |
| `Enter` | In an input, confirm what you typed: a version for **Pin to version**, or a package to add. |
| `Esc` | Close a popup or cancel input, and dismiss the message on the bottom line. |
| `p` | Add a package from the default index. Typed as a name alone, such as `flask[dotenv]`, it then lists the package's versions as **Pin to version** does: **Latest stable version**, highlighted, adds it with no version, so uv takes the newest stable release the project can use, and a version asks how to pin it. Typed with a version, a marker or a URL, it skips the list. Then choose its group. |
| `P` | Add a package from another index (the package is pinned to it), a git repository (`URL@branch`, `@tag` or `@commit`) or a local path (editable unless you say otherwise), then choose its group. |
| `u` | Update the selected package, as **Update** in its actions does. |
| `d` | Remove the selected package at once, as **Remove** in its actions does. |
| `s` | Sync the environment with `uv sync`. |
| `r` | Read both tables again, for changes made outside uview. |
| `v` | Change the project's version. The list shows what each bump makes, e.g. `minor → 0.5.0`, as `uv version --bump … --dry-run` says; Enter writes it with `uv version --bump`. **Set version…** takes a version typed in. |
| `i` | Show the project's package indexes (`[[tool.uv.index]]`). Pick one, or press `d` on it, to remove it, or add one: a name, a URL, and whether it is an extra index searched alongside PyPI or the default that replaces it. uview edits pyproject.toml for this, since uv has no command for it, then runs `uv lock`. |
| `?` | Show every key, by where it works; `↑` / `↓` scroll it when it does not fit. `Esc`, `?` or `q` closes the list. |
| `Ctrl+C` | Cancel the running operation and those waiting behind it, in an input too. |
| `q` | Quit, cancelling whatever is running first. |

While you type into an input, every key except `Enter`, `Esc` and `Ctrl+C` goes into the
field. While the actions list is open, only its keys work, with `?`, `q` and `Ctrl+C`.
