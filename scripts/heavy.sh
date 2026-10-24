#!/bin/sh
# ABOUTME: Runs one heavy command (a dogfood, an external project's test suite) under a machine-wide lock, so at
# ABOUTME: most one runs at a time across every checkout, and refuses to start from a working tree that no longer exists.
set -eu

fail() {
	echo "heavy.sh: $*" >&2
	exit 1
}

[ $# -gt 0 ] || fail "usage: heavy.sh COMMAND [ARG...]"

# A checkout removed under a still-running shell (an agent's worktree, say) must not run anything:
# the command would run against nothing, and loop forever if its caller retries.
git rev-parse --show-toplevel >/dev/null 2>&1 ||
	fail "the working tree this command was started from no longer exists"

# One lock for the machine: a dogfood grades a whole tree and the deep gate builds mutants in slots
# that each hold a build of the workspace; an external suite run for the dogfood of another project
# is as heavy. Two at once starve each other, and a third load makes Cupel's own timing-sensitive
# tests fail, so every heavy command on the machine takes this lock.
lock=${CUPEL_HEAVY_LOCK:-/tmp/cupel-heavy.lock}

# flock(1) is not on macOS; perl is on every platform the Makefile runs on. The lock lives on the
# descriptor, which must survive the exec ($^F keeps it open across it) so the command holds it.
exec perl -MFcntl=:flock -e '
	my ($lock, @command) = @ARGV;
	$^F = 1023;
	open(my $fh, ">>", $lock) or die "heavy.sh: cannot open $lock: $!\n";
	unless (flock($fh, LOCK_EX | LOCK_NB)) {
		print STDERR "heavy.sh: waiting for $lock (another heavy command is running)\n";
		flock($fh, LOCK_EX) or die "heavy.sh: cannot lock $lock: $!\n";
	}
	exec @command or die "heavy.sh: cannot run $command[0]: $!\n";
' "$lock" "$@"
