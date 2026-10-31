#!/bin/sh
# ABOUTME: The environment of the pinned development tools (ADR-139) and of an agent checkout's cap on cores (ADR-156).
# ABOUTME: Source it (`. scripts/dev-env.sh`); the Makefile reads `cupel_dev_env` and `cupel_dev_agent` for every target.

# cupel_dev_env prints NAME=VALUE for each variable of the development environment, one per line: the
# tools' and, in an agent checkout, the cap's.
cupel_dev_env() {
	cupel_dev_tools_env
	cupel_dev_cap
}

# cupel_dev_agent succeeds in a linked git worktree, which is where agents work; the integrator's checkout, a
# human's clone, and CI's checkout are not linked worktrees (ADR-156 §2).
cupel_dev_agent() {
	cupel_dev_git_dir=$(git rev-parse --absolute-git-dir 2>/dev/null) || return 1
	cupel_dev_common_dir=$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null) || return 1
	[ "$cupel_dev_git_dir" != "$cupel_dev_common_dir" ]
}

# cupel_dev_cap prints, in an agent checkout, cargo's jobs and nextest's and libtest's test threads at half the
# online cores (at least one), so agents' builds and tests leave the machine room; elsewhere nothing. Every build
# and test run below make reads them, those Cupel's tests start included; an explicit flag still wins.
cupel_dev_cap() {
	cupel_dev_agent || return 0
	cupel_dev_half=$(($(getconf _NPROCESSORS_ONLN) / 2))
	[ "$cupel_dev_half" -ge 1 ] || cupel_dev_half=1
	echo "CARGO_BUILD_JOBS=$cupel_dev_half"
	echo "NEXTEST_TEST_THREADS=$cupel_dev_half"
	echo "RUST_TEST_THREADS=$cupel_dev_half"
	unset cupel_dev_half
}

# cupel_dev_tools_env prints NAME=VALUE for each variable the development tools need, one per line, and nothing
# when the tools directory does not exist (as on a CI runner, whose setup steps put the same tools on PATH).
# PATH gains, in front, each tool directory that exists and is not on it already.
cupel_dev_tools_env() {
	cupel_dev_tools=${CUPEL_DEV_TOOLS:-$HOME/.cache/cupel/dev-tools}
	[ -d "$cupel_dev_tools" ] || return 0
	case $cupel_dev_tools in
	*[[:space:]]*)
		echo "dev-env.sh: $cupel_dev_tools contains whitespace; set CUPEL_DEV_TOOLS to a path without it" >&2
		return 0
		;;
	esac
	cupel_dev_path=$PATH
	# In reverse: each is put in front of the ones before it, so `bin` ends up first.
	for cupel_dev_dir in dafny zig go/bin clojure/bin gradle/bin maven/bin jdk/bin venv/bin \
		jazzer/node_modules/.bin node/bin bin; do
		cupel_dev_dir=$cupel_dev_tools/$cupel_dev_dir
		[ -d "$cupel_dev_dir" ] || continue
		case ":$cupel_dev_path:" in
		*":$cupel_dev_dir:"*) ;;
		*) cupel_dev_path=$cupel_dev_dir:$cupel_dev_path ;;
		esac
	done
	echo "PATH=$cupel_dev_path"
	if [ -d "$cupel_dev_tools/jdk" ]; then
		echo "JAVA_HOME=$cupel_dev_tools/jdk"
	fi
	# Maven 3.9 reads MAVEN_ARGS; the Clojure CLI's user deps.edn (written by dev-setup.sh) names the same
	# local repository, so the fixtures' JUnit, JaCoCo, PIT, and Cloverage are downloaded once.
	echo "MAVEN_ARGS=-Dmaven.repo.local=$cupel_dev_tools/m2/repository"
	echo "GRADLE_USER_HOME=$cupel_dev_tools/gradle-home"
	echo "CLJ_CONFIG=$cupel_dev_tools/clojure-config"
	echo "CLJ_CACHE=$cupel_dev_tools/clojure-cache"
	echo "KANI_HOME=$cupel_dev_tools/kani-home"
	unset cupel_dev_tools cupel_dev_path cupel_dev_dir
}

# cupel_dev_env_exports prints the same variables as `export NAME='VALUE'` lines, for `eval`.
cupel_dev_env_exports() {
	cupel_dev_env | sed "s/^\([A-Z_]*\)=\(.*\)\$/export \1='\2'/"
}

eval "$(cupel_dev_env_exports)"
