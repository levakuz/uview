# 0017. Ship uview as binary wheels built by maturin, released from a tag

- **Status:** Accepted (2026-11-06)
- **Implemented by:** #64

## Context

uview's users are Python developers who already have uv, and they install tools with
`uv tool install`, `pipx` or `pip`. Building uview from source needs a Rust toolchain, which
most of them do not have. The release has to put a working `uview` on `PATH` from PyPI, on
Linux, macOS and Windows, with nothing published from a maintainer's machine and no token
stored anywhere.

uv itself and ruff ship this way: a wheel that carries the binary and nothing to import.

## Decision

We will publish the package `uview` on PyPI as wheels built by maturin in binary mode
(`bindings = "bin"`), from a root `pyproject.toml` whose version is Cargo.toml's: one version,
which `uview --version` and the help show too. The crate takes no PyO3 dependency.

`.github/workflows/release.yml` builds wheels for Linux (manylinux and musllinux, x86_64 and
aarch64), macOS (x86_64 and arm64) and Windows (x86_64), and an sdist; installs them with the
oldest pip supported, 21.2, and with `uv tool install` and `uvx`; and publishes them with
`uv publish` through trusted publishing. A tag `vX.Y.Z`, which must equal Cargo.toml's
version, publishes to PyPI and makes a GitHub Release with the size of each file. A run
started by hand publishes to TestPyPI, as a rehearsal.

## Consequences

- `uv tool install uview` and `uvx uview` work without Rust; the sdist builds anywhere else,
  with a Rust toolchain.
- The repository root holds the package's `pyproject.toml`. The Python project that was kept
  there, ignored, for trying uview by hand has to live elsewhere, and uview opened on the
  repository opens the uview package itself.
- Releasing needs the maintainer's one-time setup on pypi.org and test.pypi.org, and GitHub
  environments `pypi` and `testpypi` (docs/releasing.md).
- A version on PyPI cannot be uploaded twice: a mistake in a release costs a version number.
  The TestPyPI rehearsal is there to catch it first.

## Alternatives considered

- **`cargo install` only.** No packaging at all, but every user needs Rust.
- **A `python/` subdirectory with `manifest-path`.** Keeps the root free for the manual-testing
  project, but is not maturin's usual layout, and the sdist from it would need checking of its
  own.
- **Binaries on GitHub Releases alone.** No pip, no `uv tool install`, and an update by hand.
