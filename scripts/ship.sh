#!/bin/sh
# ABOUTME: Releases and publishes Cupel from the owner's Mac (ADR-238): `release` builds the five archives and creates the GitHub Release,
# ABOUTME: `publish` uploads one registry's package (npm, PyPI, the Homebrew tap), and `verify` checks every live route. The make targets run it.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
repo=tysont/cupel
tap=tysont/homebrew-cupel
tap_url=git@github.com:$tap.git
# The Python packaging tools the PyPI upload installs into a throwaway virtual environment.
build_version=1.6.1
twine_version=6.1.0

fail() {
	echo "ship.sh: $*" >&2
	exit 1
}

say() {
	echo "ship.sh: $*" >&2
}

usage() {
	cat >&2 <<'EOF'
usage: ship.sh [--dry-run] release VERSION            build every archive and create the GitHub Release vVERSION
       ship.sh [--dry-run] publish REGISTRY VERSION   publish VERSION to npm, pypi, or homebrew
       ship.sh verify VERSION                         check the release on every live route
With --dry-run, every local step runs and nothing is created, uploaded, or pushed.
EOF
	exit 2
}

dry=false
if [ "${1:-}" = --dry-run ]; then
	dry=true
	shift
fi

# Runs a command that changes something outside this machine; a dry run prints it instead.
remote() {
	if $dry; then
		say "dry run, not running: $*"
	else
		"$@"
	fi
}

check_version() {
	echo "$1" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' ||
		fail "\"$1\" is not a version: pass MAJOR.MINOR.PATCH, such as 0.3.0"
}

tag_commit() {
	git -C "$root" rev-parse --quiet --verify "refs/tags/v$1^{commit}" 2>/dev/null || true
}

need_tag() {
	[ -n "$(tag_commit "$1")" ] ||
		fail "there is no tag v$1; tag the release commit first: git tag -a v$1 -m \"Cupel $1\""
}

# Each release file's name: every target's archive, as dist/targets.toml names it, then SHA256SUMS.
release_files() {
	awk -v v="$1" '
		/^triple = / { gsub(/"/, "", $3); triple = $3 }
		/^format = / { gsub(/"/, "", $3); print "cupel-" v "-" triple "." $3 }
		END { print "SHA256SUMS" }
	' "$root/dist/targets.toml"
}

triples() {
	sed -n 's/^triple = "\(.*\)"$/\1/p' "$root/dist/targets.toml"
}

# The names of the GitHub Release's assets, or nothing and a failure when there is no such release.
release_assets() {
	gh release view "v$1" --repo "$repo" --json assets --jq '.assets[].name' 2>/dev/null
}

# A scratch directory, removed on exit, holding the tag's copy of dist/ under $work/dist.
tag_tree() {
	work=$(mktemp -d)
	trap 'rm -rf "$work"' EXIT
	git -C "$root" archive "v$1" dist scripts/release.sh Cargo.toml | tar -x -C "$work"
}

release() {
	v=$1
	check_version "$v"
	changes=$(git -C "$root" status --porcelain)
	if [ -n "$changes" ]; then
		printf 'ship.sh: the tree has changes; release from a clean checkout of v%s:\n%s\n' "$v" "$changes" >&2
		exit 1
	fi
	need_tag "$v"
	tagged=$(tag_commit "$v")
	head=$(git -C "$root" rev-parse HEAD)
	[ "$tagged" = "$head" ] || fail "v$v is $tagged, not the checked-out commit $head; check out the tag"
	sh "$root/scripts/release.sh" version "v$v" >/dev/null
	[ "$(uname -s)" = Darwin ] || fail "a release builds both macOS archives with Apple's toolchain, so it runs on a Mac"

	pushed=$(gh api "repos/$repo/git/ref/tags/v$v" --jq .object.sha 2>/dev/null || true)
	[ "$pushed" = "$(git -C "$root" rev-parse "refs/tags/v$v")" ] ||
		fail "GitHub does not have this v$v; push it first: git push origin v$v"

	existing=$(release_assets "$v") && exists=true || exists=false
	if $exists; then
		missing=$(release_files "$v" | while read -r file; do
			printf '%s\n' "$existing" | grep -qx "$file" || echo "$file"
		done)
		if [ -z "$missing" ]; then
			say "v$v is already released with every archive and SHA256SUMS"
			return 0
		fi
		say "v$v is released without: $(echo "$missing" | tr '\n' ' ')"
	fi

	out=$root/target/release-$v
	rm -rf "$out"
	mkdir -p "$out"
	for triple in $(triples); do
		say "building $triple"
		make -C "$root" --no-print-directory dist TARGET="$triple" DIST_DIR="$out" >&2
		case $triple in
		*-linux-*) sh "$root/scripts/release.sh" static "$root/target/$triple/release/cupel" >&2 ;;
		esac
	done
	reports "$v" arch -arm64 "$root/target/aarch64-apple-darwin/release/cupel" --version >&2
	reports "$v" arch -x86_64 "$root/target/x86_64-apple-darwin/release/cupel" --version >&2
	sh "$root/scripts/release.sh" sums "$out"
	cat "$out/SHA256SUMS"
	if $exists; then
		remote gh release upload "v$v" --repo "$repo" --clobber "$out"/*
	else
		remote gh release create "v$v" --repo "$repo" --verify-tag --generate-notes --title "cupel $v" "$out"/*
	fi
}

# Fails unless the GitHub Release the wrappers download from is complete.
need_release() {
	assets=$(release_assets "$1") || fail "there is no GitHub Release v$1; run make release V=$1 first"
	printf '%s\n' "$assets" | grep -qx SHA256SUMS || fail "the GitHub Release v$1 has no SHA256SUMS; run make release V=$1"
}

# The token PyPI's upload reads from the environment, which `op run` fills from 1Password.
need_token() {
	name=$1 registry=$2 v=$3
	if ! $dry && [ -z "$(eval "printf '%s' \"\${$name:-}\"")" ]; then
		fail "$name is not set; run it with the token from 1Password: op run --env-file=release.env.tpl -- make publish-$registry V=$v"
	fi
}

# npm is signed in to through the browser for each publish, where the owner confirms with their 2FA (ADR-238).
# The session's token goes into a config file in this step's scratch directory, never ~/.npmrc, and the exit
# trap revokes it with `npm logout` however the step ends, then removes the directory.
publish_npm() {
	v=$1
	# Both wait on the terminal for the owner's confirmation; without one npm falls back to a username
	# prompt nothing can answer.
	if ! $dry && ! [ -t 0 ]; then
		fail "npm's browser sign-in and its publish each wait on a terminal for your confirmation; run make publish-npm V=$v in a terminal of your own"
	fi
	need_tag "$v"
	need_release "$v"
	tag_tree "$v"
	if published=$(npm view "cupel-cli@$v" version 2>"$work/npm.err"); then
		if [ "$published" = "$v" ]; then
			say "cupel-cli $v is already on npm"
			return 0
		fi
	else
		grep -q E404 "$work/npm.err" || { cat "$work/npm.err" >&2; fail "npm view cupel-cli@$v failed"; }
	fi
	package=$(node -p "require('$work/dist/npm/package.json').version")
	[ "$package" = "$v" ] || fail "v$v's dist/npm/package.json is version $package"
	: >"$work/npmrc"
	if $dry; then
		npm publish "$work/dist/npm" --access public --userconfig "$work/npmrc" --dry-run
		return 0
	fi
	trap 'npm logout --userconfig "$work/npmrc" >/dev/null 2>&1 || say "npm logout failed; revoke the session at https://www.npmjs.com/settings/~/tokens"; rm -rf "$work"' EXIT
	say "signing in to npm: confirm in the browser with your 2FA"
	npm login --auth-type=web --userconfig "$work/npmrc"
	npm publish "$work/dist/npm" --access public --userconfig "$work/npmrc"
}

publish_pypi() {
	v=$1
	need_token PYPI_TOKEN pypi "$v"
	need_tag "$v"
	need_release "$v"
	status=$(curl -s -o /dev/null -w '%{http_code}' "https://pypi.org/pypi/cupel-cli/$v/json")
	case $status in
	200)
		say "cupel-cli $v is already on PyPI"
		return 0
		;;
	404) ;;
	*) fail "PyPI answered $status for cupel-cli $v" ;;
	esac
	tag_tree "$v"
	python3 -m venv "$work/venv"
	# A pip configuration with `user = true` cannot install into a virtual environment, this one or the
	# isolated one `build` makes.
	export PIP_USER=0
	"$work/venv/bin/pip" install --quiet --disable-pip-version-check "build==$build_version" "twine==$twine_version"
	"$work/venv/bin/python" -m build --outdir "$work/pypi" "$work/dist/pipx" >&2
	[ -f "$work/pypi/cupel_cli-$v-py3-none-any.whl" ] || fail "v$v's dist/pipx did not build cupel_cli-$v-py3-none-any.whl"
	"$work/venv/bin/twine" check "$work/pypi"/*
	if $dry; then
		say "dry run, not uploading: $(cd "$work/pypi" && echo *)"
	else
		TWINE_USERNAME=__token__ TWINE_PASSWORD=${PYPI_TOKEN} \
			"$work/venv/bin/twine" upload --non-interactive "$work/pypi"/*
	fi
}

# The tap's formula is rendered from the release's SHA256SUMS by this checkout's dist/homebrew, and
# pushed over the SSH key of whoever runs it.
publish_homebrew() {
	v=$1
	need_tag "$v"
	need_release "$v"
	work=$(mktemp -d)
	trap 'rm -rf "$work"' EXIT
	gh release download "v$v" --repo "$repo" --pattern SHA256SUMS --dir "$work"
	sh "$root/dist/homebrew/render.sh" "$v" "$work/SHA256SUMS" >"$work/cupel.rb"
	current=$(gh api "repos/$tap/contents/Formula/cupel.rb" --jq .content 2>/dev/null | base64 -d 2>/dev/null || true)
	if [ "$current" = "$(cat "$work/cupel.rb")" ]; then
		say "the tap's Formula/cupel.rb is already $v"
		return 0
	fi
	if $dry; then
		say "dry run, not pushing Formula/cupel.rb for $v to $tap:"
		cat "$work/cupel.rb"
	else
		sh "$root/dist/homebrew/publish.sh" "v$v" "$work/SHA256SUMS" "$tap_url"
	fi
}

publish() {
	registry=$1 v=$2
	case $registry in
	npm | pypi | homebrew) ;;
	*)
		echo "ship.sh: no registry \"$registry\": publish to npm, pypi, or homebrew (crates.io stays unpublished, ADR-146)" >&2
		exit 2
		;;
	esac
	check_version "$v"
	"publish_$registry" "$v"
}

# Prints a step's heading.
step() {
	echo "== $*"
}

# Fails unless the command's output is "cupel VERSION"; prints the output.
reports() {
	expected=$1
	shift
	reported=$("$@" | tail -1)
	echo "$reported"
	[ "$reported" = "cupel $expected" ] || fail "$* reports \"$reported\", not \"cupel $expected\""
}

# Every route the release is published on, from what a user downloads. The archives are checked against
# the tag's own release script and install script.
verify() {
	v=$1
	check_version "$v"
	need_tag "$v"
	tag_tree "$v"
	mkdir -p "$work/assets" "$work/mac" "$work/install"
	(cd "$work/assets" && gh release download "v$v" --repo "$repo")
	step "release assets"
	ls -1 "$work/assets"
	step checksums
	(cd "$work/assets" && shasum -a 256 -c SHA256SUMS)
	step layout
	for archive in "$work/assets"/cupel-*; do
		sh "$work/scripts/release.sh" verify "$archive"
		echo "ok $(basename "$archive")"
	done
	step "macOS binaries"
	for target in aarch64-apple-darwin x86_64-apple-darwin; do
		tar -xzf "$work/assets/cupel-$v-$target.tar.gz" -C "$work/mac"
	done
	reports "$v" "$work/mac/cupel-$v-aarch64-apple-darwin/cupel" --version
	reports "$v" arch -x86_64 "$work/mac/cupel-$v-x86_64-apple-darwin/cupel" --version
	step "install.sh on the Mac"
	CUPEL_VERSION=$v CUPEL_INSTALL_DIR="$work/install" sh "$work/dist/install.sh" >&2
	reports "$v" "$work/install/cupel" --version
	step "install.sh in ubuntu:24.04 x86_64"
	reports "$v" docker run --rm --platform linux/amd64 -v "$work/dist/install.sh:/install.sh:ro" -e CUPEL_VERSION="$v" ubuntu:24.04 \
		sh -c 'apt-get update -qq >/dev/null && apt-get install -y -qq curl ca-certificates >/dev/null 2>&1 && sh /install.sh >&2 && ~/.local/bin/cupel --version'
	step "install.sh in alpine:3.20 arm64"
	reports "$v" docker run --rm --platform linux/arm64 -v "$work/dist/install.sh:/install.sh:ro" -e CUPEL_VERSION="$v" alpine:3.20 \
		sh -c 'apk add -q curl >/dev/null && sh /install.sh >&2 && ~/.local/bin/cupel --version'
	step "cargo binstall in rust:1.92-bookworm"
	reports "$v" docker run --rm rust:1.92-bookworm sh -c 'curl -L --proto =https --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash >/dev/null 2>&1 && cargo binstall -y --git https://github.com/tysont/cupel cupel-cli >&2 && cupel --version'
	step "npm cupel-cli@$v"
	npm view "cupel-cli@$v" version
	reports "$v" env CUPEL_CACHE_DIR="$work/npx-cache" npx -y "cupel-cli@$v" --version
	step "PyPI cupel-cli $v"
	curl -sf "https://pypi.org/pypi/cupel-cli/$v/json" | python3 -c 'import json, sys; print(json.load(sys.stdin)["info"]["version"])'
	reports "$v" env CUPEL_CACHE_DIR="$work/pipx-cache" pipx run --no-cache --spec "cupel-cli==$v" cupel --version
	# The formula is installed in Homebrew on Linux, in Docker, so the check neither depends on nor changes
	# this Mac's Homebrew; on the Mac, `brew fetch` downloads the macOS archive and checks the formula's digest.
	step "Homebrew tap"
	gh api "repos/$tap/commits/main" --jq .commit.message
	HOMEBREW_NO_AUTO_UPDATE=1 brew fetch --formula tysont/cupel/cupel >&2
	# shellcheck disable=SC2016 # expanded by the container's shell
	reports "$v" docker run --rm --platform linux/amd64 homebrew/brew:latest sh -c \
		'HOMEBREW_NO_AUTO_UPDATE=1 brew install tysont/cupel/cupel >&2 && "$(brew --prefix)/bin/cupel" --version'
	echo "verify=ok"
}

[ $# -gt 0 ] || usage
command=$1
shift
case $command in
release) [ $# -eq 1 ] || usage; release "$1" ;;
publish) [ $# -eq 2 ] || usage; publish "$1" "$2" ;;
verify) [ $# -eq 1 ] || usage; verify "$1" ;;
*) usage ;;
esac
