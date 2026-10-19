#!/bin/sh
# ABOUTME: Installs the cupel release binary for this machine from GitHub Releases after checking its SHA-256
# ABOUTME: (ADR-125). CUPEL_VERSION picks a release (default the latest), CUPEL_INSTALL_DIR the directory.
#
#   curl -fsSL https://raw.githubusercontent.com/tysont/cupel/main/dist/install.sh | sh
#
# CUPEL_INSTALL_DIR defaults to ~/.local/bin. CUPEL_DOWNLOAD_URL replaces
# https://github.com/tysont/cupel/releases, for a mirror. Nothing runs until the last line
# calls `main`, so a truncated download of this script does nothing.
set -eu

releases="${CUPEL_DOWNLOAD_URL:-https://github.com/tysont/cupel/releases}"
install_dir="${CUPEL_INSTALL_DIR:-$HOME/.local/bin}"

fail() {
	echo "cupel install: $*" >&2
	exit 1
}

# The release target for this machine, as dist/targets.toml names it.
detect_target() {
	system=$(uname -s)
	machine=$(uname -m)
	case "$system-$machine" in
	Linux-x86_64 | Linux-amd64) echo x86_64-unknown-linux-musl ;;
	Linux-aarch64 | Linux-arm64) echo aarch64-unknown-linux-musl ;;
	Darwin-x86_64) echo x86_64-apple-darwin ;;
	Darwin-arm64 | Darwin-aarch64) echo aarch64-apple-darwin ;;
	*) fail "no release for $system $machine; see https://github.com/tysont/cupel#install" ;;
	esac
}

# fetch <url> <file>
fetch() {
	if command -v curl >/dev/null 2>&1; then
		curl -fsL -o "$2" "$1" || fail "could not download $1"
	elif command -v wget >/dev/null 2>&1; then
		wget -q -O "$2" "$1" || fail "could not download $1"
	else
		fail "downloading needs curl or wget"
	fi
}

# The SHA-256 of a file, in hex.
sha256() {
	if command -v sha256sum >/dev/null 2>&1; then
		set -- "$(sha256sum "$1")"
	elif command -v shasum >/dev/null 2>&1; then
		set -- "$(shasum -a 256 "$1")"
	else
		fail "checking the download needs sha256sum or shasum"
	fi
	echo "${1%% *}"
}

main() {
	target=$(detect_target)
	version="${CUPEL_VERSION:-}"
	if [ -n "$version" ]; then
		base="$releases/download/v${version#v}"
	else
		base="$releases/latest/download"
	fi
	tmp=$(mktemp -d)
	trap 'rm -rf "$tmp"' EXIT
	fetch "$base/SHA256SUMS" "$tmp/SHA256SUMS"
	# The archive's line, "<sha256>  cupel-<version>-<target>.tar.gz", also names the version.
	line=$(grep -E "[ *]cupel-[^ ]+-$target\.tar\.gz\$" "$tmp/SHA256SUMS") ||
		fail "$base/SHA256SUMS lists no archive for $target"
	expected=${line%% *}
	archive=${line##*[ *]}
	echo "cupel install: downloading $base/$archive" >&2
	fetch "$base/$archive" "$tmp/$archive"
	actual=$(sha256 "$tmp/$archive")
	[ "$actual" = "$expected" ] ||
		fail "checksum mismatch for $archive: SHA256SUMS has $expected, the download is $actual"
	tar -xzf "$tmp/$archive" -C "$tmp"
	mkdir -p "$install_dir"
	cp "$tmp/${archive%.tar.gz}/cupel" "$install_dir/.cupel.download"
	chmod 755 "$install_dir/.cupel.download"
	mv -f "$install_dir/.cupel.download" "$install_dir/cupel"
	echo "cupel install: installed $("$install_dir/cupel" --version) as $install_dir/cupel"
	case ":$PATH:" in
	*":$install_dir:"*) ;;
	*)
		echo "Add $install_dir to your PATH, for example in ~/.profile:"
		echo "  export PATH=\"$install_dir:\$PATH\""
		;;
	esac
}

main
