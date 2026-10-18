# 0012. Read package indexes over HTTP for what uv cannot list

- **Status:** Accepted (2026-10-12)
- **Implemented by:** #23

## Context

Pinning a package should offer the versions it has, so the user need not know them (#23); a
bump to the next release (#102) and a package search (#22) need the same. uv 0.8.9 has no
command that lists a package's versions or searches an index, and parsing its resolver errors
for them would break with its wording. Every other operation goes through the `uv` binary
(ADR-0002), and only `src/uv/` starts a process.

The indexes a project uses speak the Simple API: PyPI answers JSON (PEP 691), the PyTorch index
only HTML (PEP 503). PyPI's JSON lists versions sorted as strings, not by PEP 440. The PyTorch
index answers 403, not 404, for a package it lacks; uv goes on past it, and past whatever an
index's `ignore-error-codes` lists.

## Decision

We will read index metadata ourselves, over HTTP, in a new bottom layer, `src/index/`, beside
`src/uv/`. An `IndexClient` trait is its seam, with `RealIndexClient` on `reqwest` (rustls, the
platform's certificate store and the environment's proxy) and fakes in tests. It asks for JSON
and takes HTML; pure parsers read both. Controllers choose the index as uv would, and order
versions with `pep440_rs`, Astral's PEP 440 crate. It only reads: nothing is uploaded or
written, and no credential is sent (#38). Only `src/index/` may use `reqwest`, which
`tests/architecture.rs` enforces.

## Consequences

- A second way out of the process besides `uv`, for reads alone. What uv would install is still
  uv's to decide; the list only offers versions.
- Our index choice follows uv's rules as far as pyproject.toml says them: pinned sources,
  explicit and default indexes, first index wins, and its ignored codes. Indexes from
  `uv.toml` or `UV_INDEX` are not read yet.
- New dependencies: `reqwest` and its TLS stack, and `pep440_rs`.
- A private index that wants credentials is reported as such until #38.
- When uv gains a command for this, `src/index/` gives way to a `UvClient` call.

## Alternatives considered

- **PyPI's JSON API only.** Simpler, but no project index from #95 would be listed.
- **Parse uv's resolver errors.** Version lists hidden in prose that changes with uv.
- **`ureq`.** Fewer dependencies, but blocking: a cancelled job's request would run on to its
  timeout instead of ending with the job.
