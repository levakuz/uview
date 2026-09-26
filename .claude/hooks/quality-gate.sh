#!/bin/sh
# Claude Code Stop hook: the agent may not finish while Rust changes fail the
# Definition of done in AGENTS.md. `cargo test` builds every target, so it is also
# the build check. Exit 2 blocks the stop and hands stderr back to the agent.

input=$(cat)
cd "$CLAUDE_PROJECT_DIR" || exit 0

# Gate only when Rust sources or manifests differ from main, committed or not.
git status --porcelain -- '*.rs' Cargo.toml Cargo.lock | grep -q . ||
  ! git diff --quiet main...HEAD -- '*.rs' Cargo.toml Cargo.lock 2>/dev/null ||
  exit 0

# Run all three even after a failure: the agent gets one retry, so it must see everything.
out=$(
  fail=0
  cargo fmt --check 2>&1 || fail=1
  cargo clippy --quiet -- -D warnings 2>&1 || fail=1
  cargo test --quiet 2>&1 || fail=1
  exit "$fail"
) && exit 0

# Already blocked once and still failing: stop looping, tell the user instead.
if printf '%s' "$input" | grep -qE '"stop_hook_active"[[:space:]]*:[[:space:]]*true'; then
  echo '{"systemMessage":"Quality gate still failing (fmt, clippy or tests). See .claude/hooks/quality-gate.sh"}'
  exit 0
fi

printf 'Quality gate failed. Fix before finishing (AGENTS.md, Definition of done):\n\n%s\n' \
  "$(printf '%s\n' "$out" | head -n 120)" >&2
exit 2
