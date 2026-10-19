#!/bin/sh
# ABOUTME: Renders the Homebrew formula for one release from its SHA256SUMS (ADR-125) and prints it; the
# ABOUTME: release workflow commits the output as Formula/cupel.rb in the tysont/homebrew-cupel tap.
#
#   sh dist/homebrew/render.sh <version> <SHA256SUMS> > cupel.rb
set -eu

if [ $# -ne 2 ]; then
	echo "usage: render.sh <version> <SHA256SUMS>" >&2
	exit 2
fi
version=${1#v}
sums=$2
template="$(dirname "$0")/cupel.rb"

# The template's two ABOUTME lines give way to a note saying where the formula comes from.
script="1,2d;s/@VERSION@/$version/g"
# Every target the template takes a digest for: one word each.
targets=$(sed -n 's/.*@SHA256:\([^@]*\)@.*/\1/p' "$template")
for target in $targets; do
	archive="cupel-$version-$target.tar.gz"
	line=$(grep -F "  $archive" "$sums") || {
		echo "render.sh: $sums lists no $archive" >&2
		exit 1
	}
	script="$script;s/@SHA256:$target@/${line%% *}/"
done
echo "# Rendered for v$version by dist/homebrew/render.sh in https://github.com/tysont/cupel; edit it there."
sed "$script" "$template"
