# uview

A terminal UI for [uv](https://github.com/astral-sh/uv), the Python package manager: see a
project's dependencies, environment and lockfile on one screen, and change them with a key.

![uview showing a project's pyproject.toml dependencies with httpx selected, beside the project tile and the uv log, just after pinning httpx](https://raw.githubusercontent.com/levakuz/uview/main/docs/screenshot.svg)

**Status: alpha (0.0.1).** uview changes `pyproject.toml`, `uv.lock` and the environment, almost
always by running uv. Expect rough edges, and please
[report what you find](https://github.com/levakuz/uview/issues).

## What it does

- **Three views of a project,** as tabs: what `pyproject.toml` declares, what is installed in
  the environment, and the dependency tree.
- **Packages:** add from PyPI, another index, git or a path; pin to a version chosen from the
  index's list; bump to the next patch, minor or major; update; move between groups; remove.
- **Dependency groups:** create, rename, delete.
- **Package indexes:** list, add, remove.
- **Lockfile and environment:** lock, upgrade the lock, sync; make the environment anew with
  another Python, empty it, delete it.
- **Project version:** bump or set it.
- **Build and publish** with `uv build` and `uv publish`.

uview has no packaging logic of its own: every action runs the `uv` binary, and the uv log on
the right shows what uv printed. The exceptions are a few changes uv has no command for, each
followed by `uv lock`: creating, renaming or deleting an empty dependency group, and adding or
removing a package index. Deleting the environment removes its directory. To list a package's
versions, uview reads the package index over HTTP, and only reads.

## Install

uview is on PyPI as a wheel that holds its binary, for Linux (glibc and musl, x86_64 and
aarch64), macOS (x86_64 and arm64) and Windows (x86_64). Any of these puts `uview` on `PATH`:

```bash
uv tool install uview
```

```bash
pipx install uview
```

```bash
pip install uview
```

or run it once without installing it:

```bash
uvx uview
```

**Requirements.** To run, uview needs
[uv](https://docs.astral.sh/uv/getting-started/installation/) 0.8.0 or newer on `PATH`; it says
so on the bottom line when uv is missing or too old. pip needs Python 3.8 or newer and pip 21.2
or newer; uview itself runs no Python. On any other platform pip builds it from the source
distribution, which needs a Rust toolchain ([rustup](https://rustup.rs)).

**From the repository,** with the toolchain pinned in `rust-toolchain.toml`, which rustup
installs on the first build:

```bash
git clone https://github.com/levakuz/uview.git
cd uview
cargo install --path . --locked
```

## Quick start

Run `uview` anywhere inside a uv project:

```bash
cd path/to/your-project
uview
```

Five keys get you going:

| Key | |
|---|---|
| `Tab` | Switch between pyproject.toml, Environment and Tree. |
| `Enter` | Open the actions for the selected package. |
| `p` | Add a package. |
| `s` | Sync the environment. |
| `?` | Show every key, by where it works. |

`q` quits. `uview --help` and `uview --version` print and exit.

## The screen

- **The tabs,** on the left:
  - **pyproject.toml** lists the declared dependencies, each with the group it belongs to
    (`main`, a dependency group, or `extra:<name>`) and its version constraint.
  - **Environment** lists what is installed in the environment uv uses for the project, not
    whichever virtualenv is activated.
  - **Tree** shows the dependency tree from `uv.lock` (see [Dependency tree](#dependency-tree)).

  Until a tab has loaded, it says so, and if it cannot load, it says why.
- **The project tile,** top right: the project's name and version, as `uv version` reports
  them, and its root. Under the root:
  - the environment, such as `.venv · Python 3.11.15 · 48 packages · 41.2 MiB`, or
    `no environment`;
  - whether `uv.lock` matches `pyproject.toml`: `uv.lock up to date`, or
    `uv.lock out of date — press l` and `no uv.lock — press l` when there is something to do.
- **The uv log,** below the tile: each operation's name, then the lines uv printed while it
  ran. `Shift+↑` and `Shift+↓`, or `PgUp` and `PgDn`, scroll it.
- **The bottom lines:** the first shows what is running and the latest error or notice; the
  last lists the most used keys for where you are. `?` lists all of them.

## Keys

In the tables and the tree:

| Key | Action |
|---|---|
| `↑` / `k`, `↓` / `j` | Move the selection. |
| `Tab` / `]`, `[` | The next tab, or the one before. |
| `Enter`, `Space` | Open the actions for the selected package. |
| `p`, `P`, `/` | [Add a package](#adding-packages): from the default index, from another source, or after looking it up. |
| `u`, `d`, `b` | Update, remove or bump the selected package, as its [actions](#package-actions) do. `d` removes at once. |
| `g` | Manage the [dependency groups](#dependency-groups). |
| `i` | Manage the [package indexes](#package-indexes). |
| `s`, `l`, `L` | [Sync, lock, or upgrade the lock](#lock-and-sync). |
| `r` | Read everything again, for changes made outside uview. |
| `e` | Manage the [environment](#environment). |
| `v` | Change the [project's version](#project-version). |
| `B`, `U` | [Build, and publish](#build-and-publish) the last build. |
| `C` | Write the settings file, to change the [colours](#colours). |
| `Shift+↑` / `Shift+↓`, `PgUp` / `PgDn` | Scroll the uv log. |
| `Esc` | Dismiss the message on the bottom line. |
| `?` | Show every key; `↑` / `↓` scroll the list, and `Esc`, `?` or `q` closes it. |
| `Ctrl+C` | [Cancel](#progress-and-cancelling) what uv is running. It does not quit. |
| `q` | Quit, cancelling whatever is running first. |

In the tree, also:

| Key | Action |
|---|---|
| `←`, `→` | Fold the package or go to its parent; unfold it. |
| `+`, `-` | Unfold everything; fold all but the direct dependencies. |
| `o` | Show or hide which packages are behind. |
| `w` | Show what pulls the selected package in, until `Esc`. |

In a list, `↑` / `↓` move, `Enter` chooses and `Esc` closes. In an input, `Enter` confirms,
`Esc` cancels, and every other key but `Ctrl+C` is typed.

## Features

### Adding packages

- **`p`** adds from the default index. Type a requirement:
  - a name alone, such as `flask[dotenv]`, then lists the package's versions, titled with PyPI's
    summary of it when it comes from PyPI. **Latest stable version**, highlighted, adds it with
    no version, so uv takes the newest stable release the project can use. Choosing a version
    asks how to pin it, as [Pin to version](#package-actions) does;
  - with a version, a marker or a URL, such as `httpx>=0.27`, it skips the list.

  Then choose where it goes: the main dependencies, a group, or a new group.
- **`P`** adds from another index (the package is pinned to it), a git repository
  (`URL@branch`, `@tag` or `@commit`) or a local path (editable unless you say otherwise), then
  asks for the group.
- **`/`** looks a package up by its exact name and shows its latest stable version, the index
  it comes from and, for a package from PyPI, its summary, required Python and home page.
  **Add …** goes on as `p` does. PyPI has no search by keyword, so the name must be exact; case
  and `-`/`_` do not matter.

### Package actions

`Enter` or `Space` on a package opens its actions.

- **Pin to version** lists the versions the package's index has, newest first. Pre-releases are
  marked `pre`, a release that needs a newer Python than the project supports is marked with
  what it needs, and yanked releases are left out. The index is the one uv would use: the
  package's pinned index; else those `UV_INDEX` names, then those of uv's settings files (the
  project's `uv.toml` or `pyproject.toml`, yours, the system's), then the default, which is
  `UV_DEFAULT_INDEX`, the one a settings file sets, or PyPI. The list says which index it asked.
  - Choosing a version asks how to pin it: `==` (highlighted), `>=`, `~=`, `==1.17.*`, `>`,
    `<=`, `<` or `!=`, each shown with what it means.
  - **Type a constraint…** takes one typed in: `1.2.3` becomes `==1.2.3`; `>=2.0` and `<3` are
    kept as typed.
  - The constraint is written with `uv add` in every group the package is declared in. A
    package that is installed but not declared is added as a main dependency.
- **Bump**, for a declared package, lists `patch → 0.25.2`, `minor → 0.28.1` and
  `major → 1.0.0`: the newest release of each above the installed version.
  - It keeps the declared operator: `==0.25.0` becomes `==0.28.1`, `>=0.27` becomes `>=0.28.1`.
  - If the new version falls outside a declared range, such as `>=1.15,<1.16`, uview shows the
    range it would write and writes it only once you confirm.
- **Update** takes the newest version the constraints allow, with `uv sync --upgrade-package`.
- **Move to group** and **Add to group**, for a package in one group, move it to another group
  or declare it in a second one. A package in several groups has **Add to group** and **Remove
  from group** instead. The requirement keeps its extras and marker.
- **Remove:**
  - in the pyproject.toml tab, it runs `uv remove` for every group and extra the package is in,
    then syncs: the package leaves the environment too, with any dependency nothing else needs;
  - in the Environment tab, it runs `uv pip uninstall` on the environment only. `pyproject.toml`
    and `uv.lock` are untouched, so a declared package comes back with the next sync.

### Dependency tree

The Tree tab shows the tree as `uv tree` draws it from `uv.lock`, fully unfolded, the direct
dependencies in bold, with why each package is there when it is an extra or a group.

- `o` marks each package that is behind with `latest: …`, which asks the index.
- `w` shows what pulls the selected package in, until `Esc`.
- `Enter`, `u`, `b` and `d` act on a package `pyproject.toml` declares as they do in the
  tables. On any other package only `u` works: it updates it within the constraints.

### Dependency groups

`g` lists the groups, each with its package count, under **New group…**. `Enter` on a group
shows its packages, with **Rename…** and **Delete group**. In the list, `n` adds a group, and
`r` and `d` rename and delete the selected one.

- **Deleting** an empty group happens at once. One with packages asks first, then removes them
  from that group only.
- **Renaming** changes the group's name in `pyproject.toml` in place.
- A group that `tool.uv.default-groups` or another group's `include-group` names is kept as it
  is.

uv has no command to create an empty group, rename a group or delete an emptied one, so uview
edits `pyproject.toml` for these, then runs `uv lock`. If uv cannot lock, the file is put back.

### Package indexes

`i` lists the project's indexes (`[[tool.uv.index]]`). Choose one, or press `d` on it, to remove
it; or add one: a name, a URL, and whether it is an extra index searched alongside PyPI or the
default that replaces it. uv has no command for this either: uview edits `pyproject.toml`, then
runs `uv lock`.

### Lock and sync

- **`s`** runs `uv sync --all-groups --all-extras`: the environment gets everything the
  pyproject.toml tab shows, and packages `uv.lock` does not list, such as ones installed with
  `uv pip install`, are uninstalled. Plain `uv sync` installs the default groups only, and
  would uninstall a package you had just added to another group or to an extra.
- **`l`** locks with `uv lock`.
- **`L`** runs `uv lock --upgrade`, which moves every package in `uv.lock` to the newest
  version the constraints allow.

If uv cannot resolve, the uv log says why. Update and Remove sync as `s` does; adding and
pinning install what they need and leave other packages alone.

### Environment

`e` manages the project's environment:

- **Make it anew with another Python…** lists every Python uv finds or can download, and runs
  `uv sync --python` with the one chosen. If the project has a `.python-version`, it is pinned
  there too, or the next sync would go back.
- **Empty it, then install the project again** runs `uv venv --clear`, then syncs.
- **Delete it** asks first, then removes the directory, since uv has no command for it. uview
  deletes it only if it holds `pyvenv.cfg` and is not the project root or above it. `s` makes
  it again.

### Project version

`v` lists what each bump would make, such as `minor → 0.5.0`, as `uv version --bump … --dry-run`
says; `Enter` writes it with `uv version --bump`. **Set version…** takes a version typed in.

### Build and publish

- **`B`** builds the sdist and wheel with `uv build`; at a workspace root without a `[project]`
  of its own, every member's. Once it succeeds, a list shows each file it wrote to `dist/`
  with its size, under **Publish…**. Older files in `dist/` are left as they are.
- **`U`**, or **Publish…** after a build, uploads the last build with `uv publish`. Choose
  PyPI, TestPyPI or a project index with a `publish-url`, then confirm: the confirmation names
  the index, its upload URL, each file and where uv will take credentials from.
  - **Publish is highlighted, and PyPI is first:** `U`, `Enter`, `Enter` uploads to PyPI, and
    a version on PyPI cannot be uploaded again. To rehearse, choose TestPyPI.
  - Only the files the last build wrote are uploaded, never the rest of `dist/`.
  - uview passes no credentials and asks for none. Set `UV_PUBLISH_TOKEN` (or
    `UV_PUBLISH_USERNAME` and `UV_PUBLISH_PASSWORD`, or `UV_KEYRING_PROVIDER=subprocess` for a
    keyring) before starting it. Their values are kept out of the uv log.

## How uview finds the project

uview walks up from the working directory to the nearest `pyproject.toml`, as uv does, and
runs every uv command from that directory. When `UV_PROJECT` is set, it starts from there
instead.

The environment is the one `uv sync` uses: `.venv`, or the path in `UV_PROJECT_ENVIRONMENT`, at
the project root, or at the workspace root for a member of a uv workspace.

With no `pyproject.toml` in the directory or any parent, uview offers to create a project
there with `uv init`. It asks first, then for the project's name, which defaults to the
directory's, and opens the new project:

```
uview: no pyproject.toml in "/home/jane/acme" or any parent directory.
Create a uv project in "/home/jane/acme"? [y/N] y
Project name [acme]:
Initialized project `acme` at `/home/jane/acme`
```

It prints why and exits when it has no terminal to ask on, and when the `pyproject.toml` has
neither a `[project]` table nor `[tool.uv.workspace]`, or sets `[tool.uv] managed = false`.

## Progress and cancelling

While an operation runs, the bottom line names it after a spinner, with how many wait behind
it, and the uv log shows what uv prints as it goes. uv locks the project, so uview runs one
change at a time, in the order you asked for them. After each, the views are read again.

`Ctrl+C` cancels the running operation and those waiting, and what uv started for it, such as a
build backend or git. On macOS and Linux, uview stops `uv` as Ctrl+C would in a shell: an add or
a pin cancelled while uv resolves it leaves `pyproject.toml` as it was. Cancelled at another
step, an operation may already have changed the project. On Windows, where a process cannot be
sent Ctrl+C, uv is killed: an add or a pin cancelled while uv resolves it leaves the new
requirement in `pyproject.toml` with `uv.lock` behind it, which `l` locks and `d` removes.
Either way, the views are then read again and show what is there. Quitting cancels the same way.

## Colours

uview comes with a dark and a light theme, both of the 16 ANSI colours, so your terminal's own
palette decides how they look and a terminal limited to 16 colours shows them all. It asks the
terminal for its background at startup and takes the theme that suits it; a terminal that does
not answer gets the dark one.

To choose for yourself, or to change a colour, edit `~/.config/uview/config.toml`
(`$XDG_CONFIG_HOME/uview/config.toml` if that is set, `%APPDATA%\uview\config.toml` on Windows,
or any file `UVIEW_CONFIG_FILE` names). Press `C` in uview to write it with every line
commented out and each colour as it is now; a file that is there is left alone. uview reads it
as it starts. Every line is optional:

```toml
theme = "light"        # or "dark"; set, uview does not ask the terminal

[colors]
text    = "reset"      # the project's name, the tabs, the keys: "reset" is the terminal's own
accent  = "blue"       # what has the focus, what is running
muted   = "darkgray"   # hints, placeholders, borders without the focus
warning = "magenta"    # a stale uv.lock, a package behind
error   = "red"
success = "#2e7d32"    # notices, the active tab
```

A colour is a name (`red`, `lightred`, `bright red`, `darkgray`), `reset`, a number from `0`
to `255`, or `#rrggbb`; the last two need a terminal that has those colours. If the file has a
mistake, the bottom line says what it is, and uview uses the theme's colour instead.

## Contributing

Bugs and ideas go to the [issues](https://github.com/levakuz/uview/issues).
[AGENTS.md](https://github.com/levakuz/uview/blob/main/AGENTS.md) is the working agreement
for the code, and
[docs/ARCHITECTURE.md](https://github.com/levakuz/uview/blob/main/docs/ARCHITECTURE.md)
describes how it is built.

## License

[MIT](https://github.com/levakuz/uview/blob/main/LICENSE).
