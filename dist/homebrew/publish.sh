#!/bin/sh
# ABOUTME: Commits the Homebrew formula rendered for one release to the tap's main branch and pushes it (ADR-235),
# ABOUTME: creating main and a README on a tap with no commits; the release workflow runs it with the tap's deploy key.
#
#   sh dist/homebrew/publish.sh <tag> <SHA256SUMS> <tap-git-url>
set -eu

if [ $# -ne 3 ]; then
	echo "usage: publish.sh <tag> <SHA256SUMS> <tap-git-url>" >&2
	exit 2
fi
tag=$1
sums=$2
tap_url=$3
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# Rendered before cloning, so a SHA256SUMS missing a target pushes nothing.
sh "$here/render.sh" "$tag" "$sums" > "$work/cupel.rb"

# A tap with no commits clones with an unborn branch; `HEAD:main` below creates main.
# Its "cloned an empty repository" warning is left out; any other failure is shown.
git clone --quiet "$tap_url" "$work/tap" 2> "$work/clone.err" || {
	cat "$work/clone.err" >&2
	exit 1
}
cd "$work/tap"
# The clone checks out the branch the tap's HEAD names, which need not be main; the formula
# goes on top of main wherever it has commits.
if git rev-parse --quiet --verify refs/remotes/origin/main > /dev/null; then
	git checkout --quiet -B main origin/main
fi
mkdir -p Formula
mv "$work/cupel.rb" Formula/cupel.rb
if [ ! -e README.md ]; then
	cat > README.md <<'EOF'
# homebrew-cupel

The Homebrew tap for [Cupel](https://github.com/tysont/cupel).

    brew install tysont/cupel/cupel

`Formula/cupel.rb` is written by Cupel's release workflow on every release; change it in
https://github.com/tysont/cupel (`dist/homebrew/`), not here.
EOF
fi
git add Formula/cupel.rb README.md
if git diff --cached --quiet; then
	echo "publish.sh: Formula/cupel.rb is already ${tag#v}"
	exit 0
fi
git -c user.name="github-actions[bot]" \
	-c user.email="41898282+github-actions[bot]@users.noreply.github.com" \
	commit --quiet -m "cupel ${tag#v}"
git push --quiet origin HEAD:main
echo "publish.sh: pushed Formula/cupel.rb for ${tag#v}"
