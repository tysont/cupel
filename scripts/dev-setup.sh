#!/bin/sh
# ABOUTME: Installs or verifies every tool the merge procedure needs (ADR-139), at the versions ci.yml and release.yml pin,
# ABOUTME: under $CUPEL_DEV_TOOLS (default ~/.cache/cupel/dev-tools). `make dev-setup` runs it; it ends with each tool's version.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
tools=${CUPEL_DEV_TOOLS:-$HOME/.cache/cupel/dev-tools}

# The pins. Each mirrors ci.yml, or release.yml's build job for zig and cargo-zigbuild, and
# crates/cupel-cli/tests/dev_setup.rs fails while a version pinned there is missing here.
nightly="nightly-2026-03-26"
nextest_version=0.9.148
llvm_cov_version=0.9.1
cargo_fuzz_version=0.13.2
hyperfine_version=1.20.0
kani_version=0.68.0
zigbuild_version=0.23.4
zig_version=0.16.0
shellcheck_version=0.11.0
# Not in CI: it lints the workflows, and ShellCheck the shell in them, before a push.
actionlint_version=1.7.12
# ci.yml's go-version '1.23' resolves to 1.23.12, the final 1.23 release.
go_version=1.23.12
staticcheck_version=2024.1.1
python_version=3.12.10
# The python-build-standalone release that carries CPython 3.12.10.
python_build=20250409
python_packages="pytest==9.1.1 pytest-cov==7.1.0 hypothesis==6.170.1 pytest-benchmark==5.1.0 mypy==2.4.0 ruff==0.17.1 icontract==2.7.3 flit_core==3.12.0"
node_version=22.23.3
npm_packages="vitest@5.0.5 @vitest/coverage-v8@5.0.5 jest@30.5.2 fast-check@4.10.2 typescript@7.0.2 eslint@10.13.0"
jazzer_version=4.0.0
# ci.yml's java-version '21' with distribution temurin resolves to Temurin's latest 21 build, this one
# as of 2026-10-19.
jdk_release=21.0.12.1
jdk_build=1
maven_version=3.9.16
gradle_version=9.6.1
clojure_version=1.12.5.1664
tla_version=1.7.4
dafny_version=4.11.0

fail() {
	echo "dev-setup.sh: $*" >&2
	exit 1
}

say() {
	echo "dev-setup.sh: $*" >&2
}

# Each release asset's platform name, for the platforms CI runs on.
case "$(uname -s)-$(uname -m)" in
Darwin-arm64)
	shellcheck_platform=darwin.aarch64 actionlint_platform=darwin_arm64 zig_platform=aarch64-macos
	go_platform=darwin-arm64 python_platform=aarch64-apple-darwin node_platform=darwin-arm64
	jdk_platform=aarch64_mac dafny_platform=arm64-macos-13
	release_targets="x86_64-apple-darwin x86_64-unknown-linux-musl aarch64-unknown-linux-musl"
	;;
Darwin-x86_64)
	shellcheck_platform=darwin.x86_64 actionlint_platform=darwin_amd64 zig_platform=x86_64-macos
	go_platform=darwin-amd64 python_platform=x86_64-apple-darwin node_platform=darwin-x64
	jdk_platform=x64_mac dafny_platform=x64-macos-13
	release_targets="aarch64-apple-darwin x86_64-unknown-linux-musl aarch64-unknown-linux-musl"
	;;
Linux-x86_64)
	shellcheck_platform=linux.x86_64 actionlint_platform=linux_amd64 zig_platform=x86_64-linux
	go_platform=linux-amd64 python_platform=x86_64-unknown-linux-gnu node_platform=linux-x64
	jdk_platform=x64_linux dafny_platform=x64-ubuntu-22.04
	release_targets="x86_64-unknown-linux-musl aarch64-unknown-linux-musl"
	;;
*) fail "no pinned tool downloads for $(uname -s) $(uname -m)" ;;
esac

case $tools in
*[[:space:]]*) fail "$tools contains whitespace; set CUPEL_DEV_TOOLS to a path without it" ;;
esac
mkdir -p "$tools/bin"
tools=$(cd "$tools" && pwd)

# One run at a time: a second waits for the first, and a lock left by a killed run is taken over.
lock=$tools/.lock
until mkdir "$lock" 2>/dev/null; do
	holder=$(cat "$lock/pid" 2>/dev/null || true)
	if [ -n "$holder" ] && ! kill -0 "$holder" 2>/dev/null; then
		rm -rf "$lock"
		continue
	fi
	say "waiting for another dev-setup.sh (pid ${holder:-unknown}) to finish"
	sleep 5
done
echo $$ >"$lock/pid"
staging=$tools/.staging-$$
trap 'rm -rf "$lock" "$staging"' EXIT
trap 'exit 130' INT TERM

installs=0

installing() {
	say "installing $*"
	installs=$((installs + 1))
}

fetch() {
	curl -sSfL --retry 3 -o "$2" "$1" || fail "could not download $1"
}

# needs NAME: whether $tools/NAME is not installed yet. Either way it sets $dir to $tools/NAME; when it
# is not, it also sets $work to an empty staging directory for the caller to install into, and `place`
# then renames $work to $dir. Nothing uses $dir until its tool's link (see `link`) points into it, so a
# process using the tools while this runs sees the old version or the new one, never half of one.
needs() {
	dir=$tools/$1
	[ ! -f "$dir/.installed" ] || return 1
	installing "$1"
	work=$staging/$1
	rm -rf "$work"
	mkdir -p "$work"
}

place() {
	: >"$work/.installed"
	rm -rf "$dir"
	mv "$work" "$dir"
}

# needs_in_place NAME: `needs` for a tool whose files record their own path (a Python venv, the
# Clojure CLI's scripts), installed straight into $dir; `mark` finishes it.
needs_in_place() {
	dir=$tools/$1
	[ ! -f "$dir/.installed" ] || return 1
	installing "$1"
	rm -rf "$dir"
	mkdir -p "$dir"
}

mark() {
	: >"$dir/.installed"
}

# link TARGET NAME: points $tools/NAME at TARGET.
link() {
	if [ "$(readlink "$tools/$2" 2>/dev/null || true)" != "$1" ]; then
		ln -sfn "$1" "$tools/$2"
	fi
}

# bin PROGRAM: links PROGRAM into $tools/bin under its own name.
bin() {
	link "$1" "bin/$(basename "$1")"
}

# unpack ARCHIVE DIR: extracts ARCHIVE into DIR and removes it.
unpack() {
	case "$1" in
	*.tar.gz) tar -xzf "$1" -C "$2" ;;
	*.tar.xz) tar -xJf "$1" -C "$2" ;;
	*.zip) unzip -q "$1" -d "$2" ;;
	*) fail "cannot unpack $1" ;;
	esac
	rm -f "$1"
}

# download NAME URL: installs the archive at URL as $tools/NAME, unless it is there already.
download() {
	if needs "$1"; then
		archive=$work/$(basename "$2")
		fetch "$2" "$archive"
		unpack "$archive" "$work"
		place
	fi
}

# cargo_tool CRATE VERSION: `cargo install --locked` into $tools/CRATE-VERSION, its programs linked into
# $tools/bin. It builds with the repository's pinned toolchain, as CI's `cargo install` steps do.
cargo_tool() {
	if needs "$1-$2"; then
		(cd "$root" && cargo install --quiet --locked --root "$work" "$1" --version "$2")
		place
	fi
	for program in "$dir"/bin/*; do
		bin "$program"
	done
}

# --- Single programs ---

download "shellcheck-$shellcheck_version" \
	"https://github.com/koalaman/shellcheck/releases/download/v$shellcheck_version/shellcheck-v$shellcheck_version.$shellcheck_platform.tar.xz"
bin "$dir/shellcheck-v$shellcheck_version/shellcheck"

download "actionlint-$actionlint_version" \
	"https://github.com/rhysd/actionlint/releases/download/v$actionlint_version/actionlint_${actionlint_version}_$actionlint_platform.tar.gz"
bin "$dir/actionlint"

download "zig-$zig_version" "https://ziglang.org/download/$zig_version/zig-$zig_platform-$zig_version.tar.xz"
link "$dir/zig-$zig_platform-$zig_version" zig

download "dafny-$dafny_version" \
	"https://github.com/dafny-lang/dafny/releases/download/v$dafny_version/dafny-$dafny_version-$dafny_platform.zip"
link "$dir/dafny" dafny

# --- Rust: the pinned toolchain's LLVM tools and release targets, the pinned nightly, and cargo tools ---

channel=$(sed -n 's/^channel = "\(.*\)"$/\1/p' "$root/rust-toolchain.toml")
host=$(rustup show | sed -n 's/^Default host: //p')
if [ -z "$channel" ] || [ -z "$host" ]; then
	fail "cannot read the toolchain channel or rustup's host"
fi

if ! rustup toolchain list | grep -q "^$channel-$host"; then
	installing "Rust $channel"
	(cd "$root" && rustup toolchain install)
fi
if ! rustup component list --installed --toolchain "$channel" | grep -q '^llvm-tools'; then
	installing "llvm-tools-preview for Rust $channel"
	rustup component add llvm-tools-preview --toolchain "$channel"
fi
for target in $release_targets; do
	if ! rustup target list --installed --toolchain "$channel" | grep -qx "$target"; then
		installing "the $target target for Rust $channel"
		rustup target add --toolchain "$channel" "$target"
	fi
done

# The race detectors and cargo-fuzz run through `cargo +nightly`; ci.yml installs the pinned nightly
# and links it as `nightly`. A `nightly` rustup installed itself is replaced, so it cannot shadow the pin.
# Whether the pinned nightly is installed with Miri and rust-src.
nightly_installed() {
	case $(rustup component list --installed --toolchain "$nightly" 2>/dev/null || true) in
	*miri*rust-src* | *rust-src*miri*) return 0 ;;
	*) return 1 ;;
	esac
}
if ! nightly_installed; then
	installing "$nightly with Miri and rust-src"
	rustup toolchain install "$nightly" --profile minimal --component miri --component rust-src
fi
toolchains="$(rustup show home)/toolchains"
nightly_link=$toolchains/nightly-$host
if [ ! -L "$nightly_link" ] && [ -e "$nightly_link" ]; then
	installing "nightly as $nightly, in place of rustup's own nightly ($(rustc +nightly --version))"
	rustup toolchain uninstall "nightly-$host"
fi
if [ "$(readlink "$nightly_link" 2>/dev/null || true)" != "$toolchains/$nightly-$host" ]; then
	[ -L "$nightly_link" ] || [ ! -e "$nightly_link" ] || fail "$nightly_link is still not a link"
	ln -sfn "$toolchains/$nightly-$host" "$nightly_link"
fi
# Builds Miri's sysroot if it is missing; otherwise it only checks it.
(cd "$root" && cargo +nightly miri setup --print-sysroot >/dev/null)

cargo_tool cargo-nextest "$nextest_version"
cargo_tool cargo-llvm-cov "$llvm_cov_version"
cargo_tool cargo-fuzz "$cargo_fuzz_version"
cargo_tool hyperfine "$hyperfine_version"
cargo_tool cargo-zigbuild "$zigbuild_version"
cargo_tool kani-verifier "$kani_version"

# `cargo kani setup` downloads Kani's bundle into KANI_HOME and installs the nightly it needs.
if [ ! -d "$tools/kani-home/kani-$kani_version" ]; then
	installing "Kani $kani_version's bundle"
	mkdir -p "$tools/kani-home"
	(cd "$root" && KANI_HOME=$tools/kani-home PATH=$tools/bin:$PATH cargo kani setup)
fi

# --- Go and staticcheck ---

download "go-$go_version" "https://go.dev/dl/go$go_version.$go_platform.tar.gz"
link "$dir/go" go
if needs "staticcheck-$staticcheck_version"; then
	GOBIN=$work GOTOOLCHAIN=local "$tools/go/bin/go" install "honnef.co/go/tools/cmd/staticcheck@$staticcheck_version"
	place
fi
bin "$dir/staticcheck"

# --- Python: CPython from python-build-standalone, and one venv with every package ci.yml installs ---

download "python-$python_version" \
	"https://github.com/astral-sh/python-build-standalone/releases/download/$python_build/cpython-$python_version+$python_build-$python_platform-install_only.tar.gz"
python=$dir/python/bin/python3
# The venv's name follows its pins, so changing one builds a new venv beside the one in use.
venv=venv-$python_version-$(echo "$python_packages" | cksum | cut -d' ' -f1)
if needs_in_place "$venv"; then
	"$python" -m venv "$dir"
	# --isolated: pip ignores the user's configuration, whose `user = true` would refuse a venv install.
	# shellcheck disable=SC2086 # one word per package
	"$dir/bin/python" -m pip install --isolated --quiet --disable-pip-version-check $python_packages
	mark
fi
link "$dir" venv

# --- Node: the runtime with the global packages in its own prefix, as setup-node's is; Jazzer.js in another ---

node=node-$node_version-$(echo "$npm_packages" | cksum | cut -d' ' -f1)
if needs "$node"; then
	fetch "https://nodejs.org/dist/v$node_version/node-v$node_version-$node_platform.tar.xz" "$work/node.tar.xz"
	unpack "$work/node.tar.xz" "$work"
	prefix=$work/node-v$node_version-$node_platform
	# shellcheck disable=SC2086 # one word per package
	PATH=$prefix/bin:$PATH npm install --global --prefix "$prefix" --no-fund --no-audit --loglevel=error $npm_packages
	place
fi
link "$dir/node-v$node_version-$node_platform" node

# ci.yml's comment says why: a global install nests Jazzer.js's packages where its core does not look.
if needs "jazzer-$jazzer_version"; then
	PATH=$tools/node/bin:$PATH npm install --prefix "$work" --no-fund --no-audit --loglevel=error "@jazzer.js/core@$jazzer_version"
	place
fi
link "$dir" jazzer

# --- The JVM tools, with their caches under $tools (scripts/dev-env.sh points the tools at them) ---

# Homebrew's openjdk@21 keg is used when it is exactly the pinned release (on the owner's Mac it is
# 21.0.12.1, the OpenJDK release Temurin 21.0.12.1+1 builds): a per-application firewall there (LuLu)
# allows that keg's java by its path and holds back a newly downloaded one, so a JVM from Temurin's
# archive cannot reach Maven Central until someone approves it (ADR-139). Elsewhere, Temurin's archive.
jdk_keg=$(brew --prefix 2>/dev/null || true)/Cellar/openjdk@21/$jdk_release/libexec/openjdk.jdk/Contents/Home
if [ -x "$jdk_keg/bin/java" ]; then
	link "$jdk_keg" jdk
else
	jdk_version=$jdk_release+$jdk_build
	download "jdk-$jdk_version" \
		"https://github.com/adoptium/temurin21-binaries/releases/download/jdk-$jdk_release%2B$jdk_build/OpenJDK21U-jdk_${jdk_platform}_hotspot_${jdk_release}_$jdk_build.tar.gz"
	if [ -d "$dir/jdk-$jdk_version/Contents/Home" ]; then
		link "$dir/jdk-$jdk_version/Contents/Home" jdk
	else
		link "$dir/jdk-$jdk_version" jdk
	fi
fi

download "maven-$maven_version" \
	"https://archive.apache.org/dist/maven/maven-3/$maven_version/binaries/apache-maven-$maven_version-bin.tar.gz"
link "$dir/apache-maven-$maven_version" maven

download "gradle-$gradle_version" "https://services.gradle.org/distributions/gradle-$gradle_version-bin.zip"
link "$dir/gradle-$gradle_version" gradle

if needs_in_place "clojure-$clojure_version"; then
	mkdir -p "$staging/clojure"
	fetch "https://github.com/clojure/brew-install/releases/download/$clojure_version/posix-install.sh" "$staging/clojure/posix-install.sh"
	(cd "$staging/clojure" && bash posix-install.sh --prefix "$dir" >/dev/null)
	mark
fi
link "$dir" clojure
# The Clojure CLI's user deps.edn, in CLJ_CONFIG: its dependencies go to the Maven repository above.
mkdir -p "$tools/clojure-config" "$tools/m2/repository"
deps="{:mvn/local-repo \"$tools/m2/repository\"}"
if [ "$(cat "$tools/clojure-config/deps.edn" 2>/dev/null || true)" != "$deps" ]; then
	installing "the Clojure CLI's deps.edn"
	echo "$deps" >"$tools/clojure-config/deps.edn"
fi

# TLC and SANY: the TLA+ tools' jar behind `tlc` and `sany` scripts, written as ci.yml writes them.
if needs "tla-$tla_version"; then
	fetch "https://github.com/tlaplus/tlaplus/releases/download/v$tla_version/tla2tools.jar" "$work/tla2tools.jar"
	mkdir "$work/bin"
	# shellcheck disable=SC2016 # "$@" belongs in the written script
	printf '#!/bin/sh\nexec java -XX:+UseParallelGC -cp "%s" tlc2.TLC "$@"\n' "$dir/tla2tools.jar" >"$work/bin/tlc"
	# shellcheck disable=SC2016 # "$@" belongs in the written script
	printf '#!/bin/sh\nexec java -cp "%s" tla2sany.SANY "$@"\n' "$dir/tla2tools.jar" >"$work/bin/sany"
	chmod +x "$work/bin/tlc" "$work/bin/sany"
	place
fi
bin "$dir/bin/tlc"
bin "$dir/bin/sany"

# --- Verification: every tool, through the environment the Makefile gives every target ---

CUPEL_DEV_TOOLS=$tools
# shellcheck disable=SC1091 # dev-env.sh is checked on its own
. "$root/scripts/dev-env.sh"

failures=0

# report NAME EXPECTED FOUND: one line per tool; a FOUND that differs from EXPECTED is a failure.
report() {
	if [ "$2" = "$3" ]; then
		printf '%-24s %s\n' "$1" "$3"
	else
		printf '%-24s %s (expected %s)\n' "$1" "${3:-missing}" "$2"
		failures=$((failures + 1))
	fi
}

# found COMMAND...: COMMAND's output, or nothing when it fails.
found() {
	"$@" 2>&1 || true
}

word() {
	awk -v n="$1" 'NR == 1 { print $n }'
}

# The version in a Node package's package.json.
package_version() {
	found node -p "require('$1/package.json').version"
}

report shellcheck "$shellcheck_version" "$(found shellcheck --version | sed -n 's/^version: //p')"
report actionlint "$actionlint_version" "$(found actionlint -version | sed -n 1p)"
report zig "$zig_version" "$(found zig version)"
report dafny "$dafny_version" "$(found dafny --version | sed -n '1s/+.*//p')"
report rust "$channel" "$(cd "$root" && found rustc --version | word 2)"
report llvm-tools-preview installed \
	"$(rustup component list --installed --toolchain "$channel" | grep -q '^llvm-tools' && echo installed)"
for target in $release_targets; do
	report "target $target" installed \
		"$(rustup target list --installed --toolchain "$channel" | grep -qx "$target" && echo installed)"
done
report "nightly" "$(found rustc "+$nightly" --version)" "$(found rustc +nightly --version)"
report "nightly miri, rust-src" installed "$(nightly_installed && echo installed)"
report cargo-nextest "$nextest_version" "$(found cargo nextest --version | word 2)"
report cargo-llvm-cov "$llvm_cov_version" "$(found cargo llvm-cov --version | word 2)"
report cargo-fuzz "$cargo_fuzz_version" "$(found cargo fuzz --version | word 2)"
report hyperfine "$hyperfine_version" "$(found hyperfine --version | word 2)"
report cargo-zigbuild "$zigbuild_version" "$(found cargo-zigbuild --version | word 2)"
report kani "$kani_version" "$(found cargo kani --version | word 4)"
report "kani bundle" "$kani_version" "$([ -d "$KANI_HOME/kani-$kani_version" ] && echo "$kani_version")"
report go "go$go_version" "$(found go version | word 3)"
report staticcheck "$staticcheck_version" "$(found staticcheck -version | word 2)"
report python "$python_version" "$(found python3 --version | word 2)"
for package in $python_packages; do
	report "${package%%==*}" "${package#*==}" \
		"$(found python3 -c 'import importlib.metadata, sys; print(importlib.metadata.version(sys.argv[1]))' "${package%%==*}")"
done
report node "v$node_version" "$(found node --version)"
npm_root=$(found npm root --global)
for package in $npm_packages; do
	report "${package%@*}" "${package##*@}" "$(package_version "$npm_root/${package%@*}")"
done
report @jazzer.js/core "$jazzer_version" "$(package_version "$tools/jazzer/node_modules/@jazzer.js/core")"
# The release, without Temurin's build number and -LTS.
report java "$jdk_release" "$(found java -version | sed -n 's/.*(build \([^+-]*\).*)$/\1/p' | sed -n 1p)"
report maven "$maven_version" "$(found mvn --version | word 3)"
report gradle "$gradle_version" "$(found gradle --version | sed -n 's/^Gradle //p')"
report clojure "$clojure_version" "$(found clojure --version | word 4)"
report "tlc, sany" "$tla_version" "$(readlink "$tools/bin/tlc" | sed -n 's|.*/tla-\([^/]*\)/bin/tlc$|\1|p')"

if [ "$installs" -eq 0 ]; then
	say "installed nothing: every tool was already at its pin"
fi
[ "$failures" -eq 0 ] || fail "$failures tools are missing or at the wrong version"
