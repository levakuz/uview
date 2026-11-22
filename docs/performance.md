# Performance

The targets, from the project's requirements (3.1.2): under 100 MB of memory, startup under 2 seconds, and efficient handling of
100 or more dependencies. Measured for #32 on 2026-10-24: a release build on an arm64
Mac, against the 212 pinned dependencies of `tests/fixtures/bench/pyproject.toml` (540 MB on disk, 29k
files in `.venv`), the page drawn at 110×30.

| | Before #32 | After #32 | Target |
|---|---|---|---|
| First frame | at once, before any `uv` call | same | < 2 s |
| Environment table loaded | 350 ms warm | 12–43 ms | |
| Everything but the size loaded | 0.40 s warm, 1.17 s cold, the size included | 70–110 ms | |
| The environment's size in the tile | with the rest | 0.39–0.65 s | |
| A frame, 200 rows | 0.22 ms | same | |
| A frame, 1000 rows | 0.77 ms | same | |
| CPU while idle | 1.7% of a core | 0 | |
| Memory (RSS) | ~13 MB | ~9 MB | < 100 MB |

## What changed

- **Drawing only on a change.** The loop drew 20 frames a second whatever happened. It now
  draws after a key, a resize or an event from the runtime, and, while a job runs, every 80 ms
  for the spinner.
- **The environment's size last.** Measuring the size walks every file of the environment,
  ~300 ms warm for 29k files. It ran in the middle of the reload, and the Environment table,
  the version, the lock state and the tree waited for it. It now runs last.

## Revised: rows outside the viewport

#32 asked that rows outside the viewport not be built, or be cached. They are still built: a
frame builds every row, 0.22 ms for 200 and 3.7 ms for 5000. Frames are now drawn per key or
event, not 20 times a second, so neither number is anywhere near visible. Building only the
visible rows would mean `SharedTable` keeping its own scroll offset, for no gain a user can
see. Reconsider if a frame of a realistic project ever nears 16 ms.

## Running the benchmark

The benchmark is in `tests/performance.rs`, ignored by a plain `cargo test`. It prints what it
measures and asserts nothing.

1. Sync the fixture in a directory of its own:

   ```bash
   mkdir -p /tmp/uview-bench && cp tests/fixtures/bench/pyproject.toml /tmp/uview-bench/
   (cd /tmp/uview-bench && uv sync)
   ```

2. Run the benchmark on a release build. `frame_time` draws the page with 200 and 1000 rows;
   `reload_time` reloads the project `UVIEW_BENCH_PROJECT` names three times, the first one
   cold, and prints when each part arrived:

   ```bash
   UVIEW_BENCH_PROJECT=/tmp/uview-bench cargo test --release --test performance -- --ignored --nocapture
   ```

3. Memory and idle CPU are the running app's: start `target/release/uview` in
   `/tmp/uview-bench`, and read `ps -o rss,time -p <pid>` five seconds in and again ten
   seconds later.
