#!/bin/sh
# ABOUTME: Runs a command with a fresh temporary directory of its own as TMPDIR and fails when the command leaves
# ABOUTME: anything in it, naming what it left; `make check` runs its build, lint, tests, and docs under it, `make parity` its suite, and `make dogfood` its gate.
#
# Usage: scripts/tmpdir-guard.sh COMMAND [ARG...]
#
# The directory is made under /tmp, whatever the caller's TMPDIR, and removed afterwards whatever the
# command did, so the caller's TMPDIR is as it was. Only the command and the processes it starts write
# there, so what is left is theirs and not another process's on the machine. Each top-level entry left
# is named on stderr. A command that fails returns its own exit code; one that succeeds but leaves
# anything returns 1.
#
# The directory's path is short: a guarded mutation run nests its own TMPDIR and a test's repository
# below it, and a socket a test makes there must fit macOS's 103-character socket path with room to
# spare. Under macOS's per-user TMPDIR (57 characters) it would not.

set -u

[ "$#" -gt 0 ] || {
	echo "usage: scripts/tmpdir-guard.sh COMMAND [ARG...]" >&2
	exit 2
}

guarded=$(mktemp -d /tmp/c.XXXXXX) || exit 2

# An interrupted command leaves nothing behind.
trap 'rm -rf "$guarded"; exit 130' INT TERM

TMPDIR=$guarded "$@"
status=$?

left=$(
	cd "$guarded" || exit 0
	for entry in .[!.]* ..?* *; do
		if [ -e "$entry" ] || [ -L "$entry" ]; then
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
