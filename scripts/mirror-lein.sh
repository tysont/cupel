#!/bin/sh
# ABOUTME: Copies one Leiningen release from Codeberg to the GitHub Release tools-lein-VERSION on tysont/cupel (ADR-240), the only
# ABOUTME: place `make dev-setup` and CI install Leiningen from, and prints the sha256 of each file for them to pin.
#
# usage: scripts/mirror-lein.sh VERSION
#
# The standalone jar is checked against the sha256 its launcher names (LEIN_CHECKSUM) before anything
# is uploaded. The release is a prerelease and never the latest, so install.sh's releases/latest still
# resolves to Cupel's own. Updating Leiningen is this script, then the pins in scripts/dev-setup.sh and
# .github/actions/tools/action.yml.
set -eu

repo=tysont/cupel
codeberg=https://codeberg.org/leiningen/leiningen

fail() {
	echo "mirror-lein.sh: $*" >&2
	exit 1
}

[ $# -eq 1 ] || fail "usage: mirror-lein.sh VERSION"
v=$1
printf '%s\n' "$v" | grep -Eqx '[0-9]+\.[0-9]+\.[0-9]+' || fail "$v is not a version like 2.12.0"
tag=tools-lein-$v
jar=leiningen-$v-standalone.jar
! gh release view "$tag" --repo "$repo" >/dev/null 2>&1 || fail "the GitHub Release $tag already exists"

work=$(mktemp -d "${TMPDIR:-/tmp}/mirror-lein.XXXXXX")
trap 'rm -rf "$work"' EXIT
curl -sSfL -o "$work/lein" "$codeberg/raw/tag/$v/bin/lein"
curl -sSfL -o "$work/$jar" "$codeberg/releases/download/$v/$jar"
named=$(sed -n "s/^export LEIN_CHECKSUM='\([0-9a-f]*\)'\$/\1/p" "$work/lein")
jar_sum=$(shasum -a 256 "$work/$jar" | cut -d' ' -f1)
[ -n "$named" ] && [ "$jar_sum" = "$named" ] || fail "$jar does not match the checksum its launcher names"
lein_sum=$(shasum -a 256 "$work/lein" | cut -d' ' -f1)

cat >"$work/notes" <<EOF
Leiningen $v, copied unchanged from $codeberg/releases/tag/$v for Cupel's pinned dev tools and CI (ADR-240). Not a Cupel release.

Leiningen is distributed under the Eclipse Public License 1.0; its source is at $codeberg.

sha256:
- lein: $lein_sum
- $jar: $jar_sum
EOF
gh release create "$tag" --repo "$repo" --target main --prerelease --latest=false \
	--title "Leiningen $v (dev tool)" --notes-file "$work/notes" "$work/lein" "$work/$jar"
echo "lein_launcher_sha256=$lein_sum"
echo "lein_jar_sha256=$jar_sum"
