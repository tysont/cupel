#!/usr/bin/env bash
# ABOUTME: Rebuilds the vendored tree-sitter grammars (ADR-161): each upstream tag, plus its patch from vendor/patches,
# ABOUTME: regenerated with the tree-sitter CLI the release used, copied into vendor/<crate>. Needs git, node, and network.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# clone NAME TAG: the upstream repository at TAG, with vendor/patches/NAME-TAG.patch applied.
clone() {
	local name="$1" tag="$2"
	git -c advice.detachedHead=false clone --quiet --depth 1 --branch "v$tag" \
		"https://github.com/tree-sitter/$name" "$work/$name-$tag"
	git -C "$work/$name-$tag" apply "$root/vendor/patches/$name-$tag.patch"
}

# cli VERSION: the path of a tree-sitter CLI of VERSION, installed once into the work directory.
cli() {
	local version="$1"
	if [ ! -x "$work/cli-$version/node_modules/.bin/tree-sitter" ]; then
		mkdir -p "$work/cli-$version"
		npm install --silent --prefix "$work/cli-$version" "tree-sitter-cli@$version" >/dev/null
	fi
	echo "$work/cli-$version/node_modules/.bin/tree-sitter"
}

# publish SOURCE DEST PATH...: replaces DEST with SOURCE's Cargo.toml, LICENSE, and the crate's PATHs.
publish() {
	local source="$1" dest="$2"
	shift 2
	rm -rf "$dest"
	mkdir -p "$dest"
	cp "$source/Cargo.toml" "$source/LICENSE" "$dest/"
	for path in "$@"; do
		mkdir -p "$dest/$(dirname "$path")"
		cp -R "$source/$path" "$dest/$path"
	done
}

# TypeScript and TSX: tree-sitter-typescript 0.23.2 builds on tree-sitter-javascript 0.23.1's grammar.
clone tree-sitter-javascript 0.23.1
clone tree-sitter-typescript 0.23.2
ts="$work/tree-sitter-typescript-0.23.2"
mkdir -p "$ts/node_modules"
ln -s "$work/tree-sitter-javascript-0.23.1" "$ts/node_modules/tree-sitter-javascript"
generate="$(cli 0.24.4)"
(cd "$ts/tsx" && "$generate" generate)
(cd "$ts/typescript" && "$generate" generate)
publish "$ts" "$root/vendor/tree-sitter-typescript" \
	common bindings/rust typescript/grammar.js typescript/src tsx/grammar.js tsx/src queries

# JavaScript and JSX.
clone tree-sitter-javascript 0.25.0
js="$work/tree-sitter-javascript-0.25.0"
generate="$(cli 0.25.8)"
(cd "$js" && "$generate" generate)
publish "$js" "$root/vendor/tree-sitter-javascript" bindings/rust grammar.js queries src tree-sitter.json

echo "vendor/tree-sitter-typescript and vendor/tree-sitter-javascript rebuilt; git diff vendor shows any change"
