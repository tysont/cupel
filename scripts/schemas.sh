#!/bin/sh
# ABOUTME: Writes every JSON Schema the cupel binary publishes as a GitHub Pages site (ADR-127): each schema at the path
# ABOUTME: its `$id` names under the site, and an index.html linking them. The pages workflow deploys what it writes.
set -eu

# The GitHub Pages site of this repository. Every `$id` lies under it (BASE_URL in cupel-core's json_schema.rs).
site=https://tysont.github.io/cupel/

fail() {
	echo "schemas.sh: $*" >&2
	exit 1
}

if [ $# -ne 2 ]; then
	echo "usage: schemas.sh BINARY OUTDIR" >&2
	exit 2
fi
binary=$1
out=$2
[ ! -e "$out" ] || fail "$out already exists"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# Writes each schema in FILE (one compact JSON document per line) to the path its `$id` names under the site.
write() {
	while IFS= read -r schema; do
		id=$(printf '%s\n' "$schema" | jq -r '."$id"')
		case "$id" in
		"$site"*) ;;
		*) fail "$id is not under $site" ;;
		esac
		path=${id#"$site"}
		mkdir -p "$out/$(dirname "$path")"
		printf '%s\n' "$schema" | jq . >"$out/$path"
		echo "$path" >>"$tmp/paths"
	done <"$1"
}

mkdir -p "$out"

# The top-level schemas, as `describe --all` lists them.
"$binary" describe --all --format json >"$tmp/all.json"
jq -c '.description.schemas[].schema' "$tmp/all.json" >"$tmp/root.jsonl"
write "$tmp/root.jsonl"

# One schema per fact kind. `describe fact` without a kind is a usage error (exit 2) whose evidence lists every kind.
status=0
"$binary" describe fact --format json >"$tmp/kinds.json" || status=$?
[ "$status" -eq 2 ] || fail "describe fact exited $status, not 2 with the list of fact kinds"
for kind in $(jq -r '.findings[0].evidence.valid[]' "$tmp/kinds.json"); do
	"$binary" describe fact "$kind" --format json | jq -c '.description.schema' >>"$tmp/facts.jsonl"
done
write "$tmp/facts.jsonl"

{
	cat <<'EOF'
<!doctype html>
<html lang="en">
<meta charset="utf-8">
<title>Cupel JSON Schemas</title>
<h1>Cupel JSON Schemas</h1>
<p>The JSON Schemas (draft 2020-12) of what <a href="https://github.com/tysont/cupel">cupel</a> emits and reads.
<code>cupel describe schema NAME</code> and <code>cupel describe fact KIND</code> print the same documents.</p>
<ul>
EOF
	while IFS= read -r path; do
		echo "<li><a href=\"$path\">$site$path</a></li>"
	done <"$tmp/paths"
	echo "</ul>"
} >"$out/index.html"
