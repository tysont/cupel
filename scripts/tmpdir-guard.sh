#!/bin/sh
# ABOUTME: Runs a command with a fresh temporary directory of its own as TMPDIR and fails when the command leaves
# ABOUTME: anything in it, naming what it left; `make check` runs its lint, tests, and docs under it.
#
# Usage: scripts/tmpdir-guard.sh COMMAND [ARG...]
#
# The directory is made under the caller's TMPDIR (/tmp without one) and removed afterwards whatever
# the command did, so the caller's TMPDIR is as it was. Only the command and the processes it starts
# write there, so what is left is theirs and not another process's on the machine. Each entry left is
# named on stderr: the top-level name, and for `cupel-mutate` each repository's slot directory under it.
# A command that fails returns its own exit code; one that succeeds but leaves anything returns 1.

set -u

if [ "$#" -eq 0 ]; then
	echo "usage: scripts/tmpdir-guard.sh COMMAND [ARG...]" >&2
	exit 2
fi

parent=${TMPDIR:-/tmp}
guarded=$(mktemp -d "${parent%/}/cupel.XXXXXX") || exit 2

TMPDIR=$guarded "$@"
status=$?

left=$(
	cd "$guarded" || exit 0
	for entry in .[!.]* ..?* *; do
		[ -e "$entry" ] || [ -L "$entry" ] || continue
		if [ "$entry" = cupel-mutate ] && [ -d "$entry" ] && [ -n "$(ls -A "$entry")" ]; then
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
