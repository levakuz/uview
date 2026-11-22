# 0019. Read uv's index settings from wherever uv reads them

- **Status:** Accepted (2026-11-22)
- **Implemented by:** #125

## Context

Pin, Bump and `/` list a package's versions from the index uv would take it from (ADR-0012).
uview read the indexes from pyproject.toml alone, so behind a mirror set in `uv.toml` or in
`UV_DEFAULT_INDEX` it asked pypi.org: a timeout, or versions uv will not find. uv has no
command that says which indexes it uses; its hidden `--show-settings` prints Rust debug
output, which is no interface.

uv takes indexes from the environment (`UV_INDEX`, `UV_DEFAULT_INDEX` and the legacy
`UV_INDEX_URL`, `UV_EXTRA_INDEX_URL`) and from settings files: the project's, found from the
workspace root upward, a `uv.toml` winning over a `pyproject.toml` beside it, which counts only
with a `[tool.uv]`; the user's `uv.toml`; the system's. `UV_CONFIG_FILE` stands for all of
them, `UV_NO_CONFIG` turns them off. Lists from several files are joined, the project's first;
the environment comes before any file, and the default index is always asked last.

## Decision

We will read those settings as uv documents them, in `controllers/uv_settings.rs`: the
runtime reads the variables and the files when a list is asked for, and a pure function orders
the indexes from them. The user's directory is the one uview's own settings use, as uv's
(ADR-0016). Index URLs are drawn with any credentials as `****`, in the list, its errors, the
list of indexes and the publish targets.

## Consequences

- A mirror or private index configured for uv anywhere is asked first, as uv asks it.
- uview now repeats a part of uv's settings logic. When uv changes it, the list may ask
  another index than uv until uview follows. `UV_ISOLATED`, `UV_NO_SYSTEM_CONFIG` and
  `no-index` are not read yet. The list's From row says which index answered, so a mismatch
  shows.
- Reading a few small files per list costs nothing next to the request.

## Alternatives considered

- **The registry uv.lock records for the package.** It is where uv took it from, whatever the
  settings, but a package not yet locked, the one `/` looks up, still needs the settings. The
  user chose the full reading.
- **pyproject.toml only, and say which index was asked.** The From row already said so; behind
  a mirror the list still timed out.
