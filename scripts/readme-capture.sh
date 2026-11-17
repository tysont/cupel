#!/bin/sh
# ABOUTME: Captures the output the README quotes: `init --write`, the `cupel.toml` it writes, survey, and `check --changed --gate` on a
# ABOUTME: scratch clone of hashicorp/golang-lru with one agent-style change, run without egress in a clean environment.
#
# usage: scripts/readme-capture.sh [CUPEL] [OUTDIR]
#
# CUPEL is the binary to capture (default: this checkout's target/release/cupel); OUTDIR receives one
# file per command, each starting with the command line, and `meta`, naming the version, the clone's
# source, and its commit (default: a new temporary directory, printed at the end). The README's
# captured blocks are trimmed by hand from these files, with `…` marking each trim, under an HTML
# comment naming the version and the commit.
#
# Every cupel command runs as the validation harness runs external repositories
# (~/.cache/cupel/validation/run.sh): `env -i` with only the toolchain's variables, under
# `sandbox-exec -f ~/.cache/cupel/validation/no-egress.sb`, which denies every outbound connection
# but loopback. So it needs macOS, the pinned tools (`make dev-setup`), and a clone of golang-lru at
# GOLANG_LRU (default ~/.cache/cupel/validation/golang-lru), whose module dependencies are already in
# GOPATH's module cache.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
cupel=$(cd "$(dirname "${1:-$root/target/release/cupel}")" && pwd)/$(basename "${1:-$root/target/release/cupel}")
out=${2:-$(mktemp -d "${TMPDIR:-/tmp}/readme-capture.XXXXXX")}
source=${GOLANG_LRU:-$HOME/.cache/cupel/validation/golang-lru}
validation=$HOME/.cache/cupel/validation
tools=${CUPEL_DEV_TOOLS:-$HOME/.cache/cupel/dev-tools}

fail() {
	echo "readme-capture.sh: $*" >&2
	exit 1
}

[ -x "$cupel" ] || fail "$cupel is not an executable; build it with scripts/heavy.sh cargo build --release -p cupel-cli"
command -v sandbox-exec >/dev/null || fail "sandbox-exec is missing: the capture runs on macOS"
[ -f "$validation/no-egress.sb" ] || fail "$validation/no-egress.sb is missing"
git -C "$source" rev-parse --git-dir >/dev/null 2>&1 || fail "$source is not a git repository"
mkdir -p "$out"
repo=$out/golang-lru
[ ! -e "$repo" ] || fail "$repo exists; pass an empty OUTDIR"

git clone --quiet --local "$source" "$repo"
git -C "$repo" config user.name "Readme Capture"
git -C "$repo" config user.email "capture@example.com"
commit=$(git -C "$repo" rev-parse HEAD)

# The environment and sandbox of the validation harness; `cupel` on PATH is the binary captured, so
# the Stop hook below runs it as Claude Code would.
bindir=$(dirname "$cupel")
path="$bindir:$tools/bin:$tools/go/bin:$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin"
sandboxed() {
	env -i PATH="$path" HOME="$HOME" CI=1 GOPATH="$HOME/Workspace/go" GOTOOLCHAIN="${GOTOOLCHAIN:-auto}" \
		TERM=dumb LANG=en_US.UTF-8 sandbox-exec -f "$validation/no-egress.sb" "$@"
}

# capture NAME DISPLAY COMMAND...: runs COMMAND in the clone and writes `$ DISPLAY`, its stdout and
# stderr, and its exit code to OUTDIR/NAME.
capture() {
	name=$1 display=$2
	shift 2
	set +e
	(cd "$repo" && sandboxed "$@") >"$out/$name.stdout" 2>"$out/$name.stderr"
	code=$?
	set -e
	{
		echo "\$ $display"
		cat "$out/$name.stdout" "$out/$name.stderr"
		echo "[exit $code]"
	} >"$out/$name"
	echo "$name: exit $code"
}

capture version "cupel --version" "$cupel" --version
capture init-preview "cupel init" "$cupel" init --format text
capture init "cupel init --write" "$cupel" init --write --format text
capture config "cat cupel.toml" cat cupel.toml
git -C "$repo" add cupel.toml
git -C "$repo" commit --quiet -m "Add cupel.toml"
capture survey "cupel survey" "$cupel" survey --format text

# The change an agent might make: a new method no test calls, and an assertion of an existing test
# deleted. The suite still passes.
git -C "$repo" checkout --quiet -b agent/add-all
git -C "$repo" apply - <<'EOF'
--- a/lru.go
+++ b/lru.go
@@ -140,6 +140,24 @@ func (c *Cache[K, V]) ContainsOrAdd(key K, value V) (ok, evicted bool) {
 	return false, evicted
 }
 
+// AddAll adds each key with the value at the same index, skipping keys
+// already in the cache, and returns how many it added and evicted.
+func (c *Cache[K, V]) AddAll(keys []K, values []V) (added, evicted int) {
+	for i, key := range keys {
+		if i >= len(values) {
+			break
+		}
+		ok, ev := c.ContainsOrAdd(key, values[i])
+		if !ok {
+			added++
+		}
+		if ev {
+			evicted++
+		}
+	}
+	return added, evicted
+}
+
 // PeekOrAdd checks if a key is in the cache without updating the
 // recent-ness or deleting it for being stale, and if not, adds the value.
 // Returns whether found and whether an eviction occurred.
--- a/lru_test.go
+++ b/lru_test.go
@@ -199,9 +199,6 @@ func TestLRUContainsOrAdd(t *testing.T) {
 	if contains {
 		t.Errorf("1 should not have been contained")
 	}
-	if !evict {
-		t.Errorf("an eviction should have occurred")
-	}
 	if !l.Contains(1) {
 		t.Errorf("now 1 should be contained")
 	}
EOF
git -C "$repo" commit --quiet -am "Add AddAll"
git -C "$repo" diff main >"$out/change.diff"

capture check "cupel check --changed --gate" "$cupel" check --changed --gate --format text
capture check-json "cupel check --changed --gate --format json" "$cupel" check --changed --gate --format json

# The README's Stop hook, exactly as its settings.json spells it, run as Claude Code runs a command hook.
# The Python program is single-quoted so the shell expands nothing in it.
# shellcheck disable=SC2016
hook=$(python3 -I -c 'import json,re,sys
text = open(sys.argv[1]).read()
for block in re.findall(r"```json\n(.*?)```", text, re.S):
    try:
        settings = json.loads(block)
    except ValueError:
        continue
    if "Stop" in settings.get("hooks", {}):
        print(settings["hooks"]["Stop"][0]["hooks"][0]["command"])' "$root/README.md")
[ -n "$hook" ] || fail "the README configures no Stop hook"
capture hook "sh -c '$hook'" sh -c "$hook"

{
	echo "cupel: $(cat "$out/version.stdout")"
	echo "repository: hashicorp/golang-lru, cloned from $source at $commit"
	echo "change: $(git -C "$repo" rev-parse HEAD) on agent/add-all"
} >"$out/meta"
cat "$out/meta"
echo "captured in $out"
