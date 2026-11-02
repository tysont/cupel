#!/bin/sh
# ABOUTME: Runs a command with a fresh temporary directory of its own as TMPDIR and fails when the command leaves
# ABOUTME: anything in it, naming what it left; `make check` runs its build, lint, tests, and docs under it, and `make dogfood` its gate.
#
# Usage: scripts/tmpdir-guard.sh [--slots-of REPO] COMMAND [ARG...]
#
# The directory is made under the caller's TMPDIR (/tmp without one) and removed afterwards whatever
# the command did, so the caller's TMPDIR is as it was. Only the command and the processes it starts
# write there, so what is left is theirs and not another process's on the machine. Each entry left is
# named on stderr: the top-level name, and for `cupel-mutate` each repository's slot directory under it
# (the slots' root itself is the machine's and stays once made, so left empty it is not a leak).
# A command that fails returns its own exit code; one that succeeds but leaves anything returns 1.
#
# With --slots-of REPO, the mutation slots the engine keeps for the repository at REPO (ADR-141 §3: each
# repository's slot directory under `cupel-mutate` names it in its `repository` file) are the caller's
# to keep across runs, as a dogfood's are: they are moved into the command's directory before it runs
# and back to the caller's after, with whatever the command added, and are not a leak. Both directories
# are on one volume, so each move is a rename.

set -u

usage() {
	echo "usage: scripts/tmpdir-guard.sh [--slots-of REPO] COMMAND [ARG...]" >&2
	exit 2
}

repo=
if [ "${1:-}" = --slots-of ]; then
	[ "$#" -ge 3 ] || usage
	repo=$(cd "$2" && pwd -P) || exit 2
	shift 2
fi
[ "$#" -gt 0 ] || usage

parent=${TMPDIR:-/tmp}
parent=${parent%/}
guarded=$(mktemp -d "$parent/cupel.XXXXXX") || exit 2

# move FROM TO: moves REPO's slot directories under FROM/cupel-mutate to TO/cupel-mutate, replacing any
# of the same name there.
move() {
	[ -n "$repo" ] || return 0
	for owner in "$1"/cupel-mutate/*/repository; do
		if [ -f "$owner" ] && [ "$(cat "$owner")" = "$repo" ]; then
			slots=${owner%/repository}
			mkdir -p "$2/cupel-mutate"
			rm -rf "$2/cupel-mutate/${slots##*/}"
			mv "$slots" "$2/cupel-mutate/"
		fi
	done
}

# An interrupted command still gives the slots back and leaves nothing behind.
trap 'move "$guarded" "$parent"; rm -rf "$guarded"; exit 130' INT TERM

move "$parent" "$guarded"
TMPDIR=$guarded "$@"
status=$?
move "$guarded" "$parent"

left=$(
	cd "$guarded" || exit 0
	for entry in .[!.]* ..?* *; do
		[ -e "$entry" ] || [ -L "$entry" ] || continue
		if [ "$entry" = cupel-mutate ] && [ -d "$entry" ]; then
			for slots in "$entry"/.[!.]* "$entry"/..?* "$entry"/*; do
				[ -e "$slots" ] && echo "$slots"
			done
		else
			echo "$entry"
		fi
	done
)
rm -rf "$guarded"

if [ -n "$left" ]; then
	count=$(printf '%s\n' "$left" | wc -l | tr -d ' ')
	echo "tmpdir-guard: the command left $count entries in its temporary directory:" >&2
	printf '%s\n' "$left" >&2
	[ "$status" -ne 0 ] || status=1
fi
exit "$status"
