#!/bin/sh
# ABOUTME: The P10 inner-loop measurement: a fresh clone of this checkout on a branch, a release binary, and
# ABOUTME: `cupel check --changed --depth standard` on one changed function, cold, warm, and unchanged, as one table.
#
# Usage: scripts/inner_loop.sh [LABEL] [BINARY]
#   LABEL   names the clone and the output files under $CUPEL_MEASURE_DIR (default: a directory under /tmp)
#   BINARY  the cupel to measure (default: this checkout's target/release/cupel, built first)
#
# The change is a comment inside one function of crates/cupel-checks/src/metrics.rs: `symbol_coverage` for the
# cold run (no cache, no slots), then `cyclomatic` for the warm run (the cache and slots of the cold run), then
# the warm commit again, unchanged. Every run goes through scripts/heavy.sh, with no daemon and --jobs 4, and
# prints its wall time, the exit code, the check table, and the store's durations over one second: the suite
# run is `tool.llvm-cov-nextest`, the engine `engine.mutation`. Report the three rows as wall / suite run / engine.
set -u
LABEL="${1:-inner-loop}"
HERE=$(cd "$(dirname "$0")/.." && pwd)
OUT="${CUPEL_MEASURE_DIR:-/tmp/cupel-inner-loop}"
mkdir -p "$OUT"
cd "$HERE" || exit 1
eval "$(make -s dev-env)"
if [ -n "${2:-}" ]; then BIN="$2"; else
  cargo build --quiet --release --package cupel-cli || exit 1
  BIN="$HERE/target/release/cupel"
fi
CLONE="$OUT/$LABEL"
rm -rf "$CLONE"
git clone --quiet "$HERE" "$CLONE" && cd "$CLONE" || exit 1
git config user.email inner-loop@cupel.invalid && git config user.name inner-loop
# `--changed` diffs against `main`: the checkout's tip, which a clone of a branch has only as `origin/<branch>`.
[ "$(git rev-parse --abbrev-ref HEAD)" = main ] || git branch --quiet --force main HEAD
git checkout --quiet -b inner-loop
export CUPEL_NO_DAEMON=1
run() {
  echo "== $1 start $(date -u +%FT%TZ) load $(uptime | sed 's/.*averages: //')"
  s=$(date +%s)
  "$HERE/scripts/heavy.sh" "$BIN" check --changed --depth standard --no-gate --jobs 4 --format text > "$OUT/$LABEL-$1.out" 2>&1
  echo "exit=$?"
  echo "== $1 wall $(( $(date +%s) - s )) s"
  grep -E "^scope|^  [a-z.-]+ +(pass|fail|error|skipped)" "$OUT/$LABEL-$1.out" | cut -c1-160
  sqlite3 .cupel/cupel.db "select kind, task, lang, class, duration_ms from durations where run_id = (select run_id from durations order by rowid desc limit 1) and duration_ms > 1000 order by duration_ms desc;" 2>/dev/null
}
"$BIN" --version
# Line 66 is inside symbol_coverage; after that insertion, line 81 is inside cyclomatic.
sed -i.bak '66i\
    // Inner-loop measurement: a one-function change.
' crates/cupel-checks/src/metrics.rs && rm -f crates/cupel-checks/src/metrics.rs.bak
git commit --quiet -am "inner loop: cold change in symbol_coverage"
run cold
sed -i.bak '81i\
    // Inner-loop measurement: a one-function change.
' crates/cupel-checks/src/metrics.rs && rm -f crates/cupel-checks/src/metrics.rs.bak
git commit --quiet -am "inner loop: warm change inside cyclomatic"
git diff -U0 HEAD~1 | grep "^@@"
run warm
run unchanged
echo "== outputs in $OUT ($LABEL-*.out)"
