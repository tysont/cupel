#!/bin/sh
# ABOUTME: The environment of the pinned development tools (ADR-139): PATH and each tool's home and cache under
# ABOUTME: $CUPEL_DEV_TOOLS. Source it (`. scripts/dev-env.sh`); the Makefile reads `cupel_dev_env` for every target.

# cupel_dev_env prints NAME=VALUE for each variable the development tools need, one per line, and nothing
# when the tools directory does not exist (as on a CI runner, whose setup steps put the same tools on PATH).
# PATH gains, in front, each tool directory that exists and is not on it already.
cupel_dev_env() {
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
