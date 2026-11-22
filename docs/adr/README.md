# Architecture decisions

Each records one decision and why it was made; `docs/ARCHITECTURE.md` describes what came
of them. An accepted one is not edited: a later one supersedes it. `template.md` starts a new
one, and `AGENTS.md` says when one is due.

| | Decision |
|---|---|
| [0001](0001-components-return-commands.md) | Components return commands; a runtime executes them |
| [0002](0002-uvclient-is-the-only-seam-to-uv.md) | `UvClient` is the only seam to `uv` |
| [0003](0003-serialize-mutating-uv-operations.md) | Mutating `uv` operations are serialized; reads are not |
| [0004](0004-introduce-appstate-ahead-of-need.md) | `AppState` is introduced ahead of the need |
| [0005](0005-uvclient-takes-argv.md) | `UvClient` takes argv, not one method per operation |
| [0006](0006-cancelling-interrupts-uv-and-waits.md) | Cancelling a job interrupts `uv` and waits for it to exit |
| [0007](0007-components-read-appstate-only-the-app-writes-it.md) | Components read `AppState`; only the app writes it |
| [0008](0008-find-the-workspace-root-as-uv-does.md) | uview finds a project's workspace root as uv does |
| [0009](0009-delete-an-empty-group-by-editing-pyproject.md) | Delete an empty dependency group by editing pyproject.toml |
| [0010](0010-manage-project-indexes-by-editing-pyproject.md) | Manage the project's indexes by editing pyproject.toml |
| [0011](0011-pass-project-paths-instead-of-changing-directory.md) | Pass the project's paths instead of changing the working directory |
| [0012](0012-read-package-indexes-over-http.md) | Read package indexes over HTTP for what uv cannot list |
| [0013](0013-create-an-empty-group-by-editing-pyproject.md) | Create an empty dependency group by editing pyproject.toml |
| [0014](0014-one-page-views-are-tabs-and-tiles.md) | One page: new views are tabs and tiles, not pages |
| [0015](0015-delete-the-environment-directory.md) | Delete the project's environment by removing its directory |
| [0016](0016-a-settings-file-where-uv-keeps-its-own.md) | Keep the user's settings in a TOML file where uv keeps its own |
| [0017](0017-ship-binary-wheels-built-by-maturin.md) | Ship uview as binary wheels built by maturin, released from a tag |
| [0018](0018-rename-a-group-by-editing-pyproject.md) | Rename a dependency group by editing pyproject.toml |
| [0019](0019-read-uv-s-index-settings-as-uv-does.md) | Read uv's index settings from wherever uv reads them |
