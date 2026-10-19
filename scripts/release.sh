#!/bin/sh
# ABOUTME: Release packaging for the cupel binary (ADR-124): the crate version and tag check, one target's archive,
# ABOUTME: SHA256SUMS, the archive layout check, and the static-binary check. `make dist` and release.yml call it.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)

fail() {
	echo "release.sh: $*" >&2
	exit 1
}

usage() {
	cat >&2 <<'EOF'
usage: release.sh version [TAG]               print the crate version; with TAG, fail unless TAG is v<version>
       release.sh package BINARY TARGET OUTDIR  write OUTDIR/cupel-<version>-<TARGET>.tar.gz (.zip on Windows)
       release.sh sums DIR                      write DIR/SHA256SUMS over every archive in DIR
       release.sh verify ARCHIVE                fail unless ARCHIVE holds exactly the binary, LICENSE, and README.md
       release.sh static BINARY                 fail unless BINARY is a statically linked ELF executable
EOF
	exit 2
}

# The workspace version every crate inherits, which `cupel --version` and the envelope report.
crate_version() {
	sed -n '/^\[workspace\.package\]/,/^\[/s/^version = "\(.*\)"$/\1/p' "$root/Cargo.toml"
}

# The binary's file name inside an archive for TARGET.
binary_name() {
	case "$1" in
	*windows*) echo cupel.exe ;;
	*) echo cupel ;;
	esac
}

# The archive's file name, without its directory, for TARGET.
archive_name() {
	case "$1" in
	*windows*) echo "cupel-$(crate_version)-$1.zip" ;;
	*) echo "cupel-$(crate_version)-$1.tar.gz" ;;
	esac
}

sha256() {
	if command -v sha256sum >/dev/null 2>&1; then
		sha256sum "$@"
	else
		shasum -a 256 "$@"
	fi
}

version() {
	v=$(crate_version)
	[ -n "$v" ] || fail "no version in [workspace.package] of $root/Cargo.toml"
	if [ $# -gt 0 ] && [ "$1" != "v$v" ]; then
		fail "tag $1 does not match the crate version $v (expected v$v)"
	fi
	echo "$v"
}

package() {
	binary=$1 target=$2 out=$3
	[ -f "$binary" ] || fail "no binary at $binary"
	mkdir -p "$out"
	out=$(cd "$out" && pwd)
	name=$(archive_name "$target")
	top=${name%.tar.gz}
	top=${top%.zip}
	staging=$(mktemp -d)
	trap 'rm -rf "$staging"' EXIT
	mkdir "$staging/$top"
	cp "$binary" "$staging/$top/$(binary_name "$target")"
	chmod 755 "$staging/$top/$(binary_name "$target")"
	cp "$root/LICENSE" "$root/README.md" "$staging/$top/"
	rm -f "$out/$name"
	case "$name" in
	*.zip)
		if command -v zip >/dev/null 2>&1; then
			(cd "$staging" && zip -qr "$out/$name" "$top")
		else
			(cd "$staging" && 7z a -tzip -bd "$out/$name" "$top" >/dev/null)
		fi
		;;
	# COPYFILE_DISABLE keeps macOS tar from adding AppleDouble `._` entries for extended attributes.
	*) COPYFILE_DISABLE=1 tar -czf "$out/$name" -C "$staging" "$top" ;;
	esac
	echo "$out/$name"
}

sums() {
	cd "$1"
	# shellcheck disable=SC2046
	set -- $(ls cupel-*.tar.gz cupel-*.zip 2>/dev/null | LC_ALL=C sort)
	[ $# -gt 0 ] || fail "no archives in $(pwd)"
	sha256 "$@" >SHA256SUMS
}

# Every file in ARCHIVE, one per line, without directory entries.
entries() {
	case "$1" in
	*.zip)
		if command -v unzip >/dev/null 2>&1; then
			unzip -Z1 "$1"
		else
			7z l -ba -slt "$1" | sed -n 's/^Path = //p' | tr '\\' /
		fi
		;;
	*) tar -tzf "$1" ;;
	esac | grep -v '/$' | LC_ALL=C sort
}

verify() {
	archive=$1
	name=$(basename "$archive")
	case "$name" in
	cupel-*.tar.gz) top=${name%.tar.gz} ;;
	cupel-*.zip) top=${name%.zip} ;;
	*) fail "$name is not named cupel-<version>-<target>.tar.gz or .zip" ;;
	esac
	target=${top#cupel-"$(crate_version)"-}
	[ "$target" != "$top" ] || fail "$name does not carry the crate version $(crate_version)"
	expected=$(printf '%s\n' "$top/LICENSE" "$top/README.md" "$top/$(binary_name "$target")" | LC_ALL=C sort)
	actual=$(entries "$archive")
	if [ "$actual" != "$expected" ]; then
		printf 'release.sh: %s holds\n%s\nbut should hold exactly\n%s\n' "$name" "$actual" "$expected" >&2
		exit 1
	fi
	case "$name" in
	*.tar.gz)
		mode=$(tar -tvzf "$archive" "$top/cupel" | cut -c1-10)
		case "$mode" in
		-rwx*) ;;
		*) fail "$top/cupel in $name is not executable ($mode)" ;;
		esac
		;;
	esac
}

static() {
	description=$(file -b "$1")
	case "$description" in
	ELF*statically\ linked* | ELF*static-pie\ linked*) echo "$1: $description" ;;
	*) fail "$1 is not a statically linked executable: $description" ;;
	esac
}

[ $# -gt 0 ] || usage
command=$1
shift
case "$command" in
version) [ $# -le 1 ] || usage; version "$@" ;;
package) [ $# -eq 3 ] || usage; package "$@" ;;
sums) [ $# -eq 1 ] || usage; sums "$(cd "$1" && pwd)" ;;
verify) [ $# -eq 1 ] || usage; verify "$1" ;;
static) [ $# -eq 1 ] || usage; static "$1" ;;
*) usage ;;
esac
