# Releasing

uview is released as wheels on PyPI (ADR-0017), from a tag, by
`.github/workflows/release.yml`. Nothing is published from a maintainer's machine.

## Once

1. On [pypi.org](https://pypi.org/manage/account/publishing/) and on
   [test.pypi.org](https://test.pypi.org/manage/account/publishing/), add a pending trusted
   publisher for the project `uview`: owner `levakuz`, repository `uview`, workflow
   `release.yml`, environment `pypi` on PyPI and `testpypi` on TestPyPI.
2. In the repository's settings, under Environments, create `pypi` and `testpypi`. Requiring a
   reviewer on `pypi` makes every release wait for a click.

## Each release

1. On a branch, set `version` in `Cargo.toml` and run `cargo build` so that `Cargo.lock`
   follows. Merge it.
2. Rehearse: start **Release** by hand from the Actions tab on `main`, or
   `gh workflow run release.yml --ref main`. It builds every wheel, installs them, and
   publishes to TestPyPI. Try it:

   ```bash
   uvx --index https://test.pypi.org/simple/ --index-strategy unsafe-best-match uview --version
   ```

3. Tag `main` with the version, and push the tag:

   ```bash
   git tag v0.0.1 origin/main && git push origin v0.0.1
   ```

   The workflow checks that the tag is `Cargo.toml`'s version, publishes to PyPI, and makes a
   GitHub Release whose notes list the size of each file.

A version on PyPI cannot be uploaded again. A broken release is fixed by the next version.
