#!/bin/sh
# ABOUTME: Runs one dogfood (`make dogfood`'s cupel invocation) under a machine-wide lock, so at most one grades
# ABOUTME: at a time across every checkout, and refuses to start from a working tree that no longer exists.
set -eu

fail() {
	echo "dogfood.sh: $*" >&2
	exit 1
}

[ $# -gt 0 ] || fail "usage: dogfood.sh COMMAND [ARG...]"

# A checkout removed under a still-running shell (an agent's worktree, say) must not grade: the
# command would run against nothing, and loop forever if its caller retries.
git rev-parse --show-toplevel >/dev/null 2>&1 ||
	fail "the working tree this dogfood was started from no longer exists"

# One lock for the machine: every dogfood grades a whole tree, the deep gate builds mutants in
# slots that each hold a build of the workspace, and two at once starve each other.
lock=${CUPEL_DOGFOOD_LOCK:-/tmp/cupel-dogfood.lock}

# flock(1) is not on macOS; perl is on every platform the Makefile runs on. The lock lives on the
# descriptor, which must survive the exec ($^F keeps it open across it) so the command holds it.
exec perl -MFcntl=:flock -e '
	my ($lock, @command) = @ARGV;
	$^F = 1023;
	open(my $fh, ">>", $lock) or die "dogfood.sh: cannot open $lock: $!\n";
	unless (flock($fh, LOCK_EX | LOCK_NB)) {
		print STDERR "dogfood.sh: waiting for $lock (another dogfood is running)\n";
		flock($fh, LOCK_EX) or die "dogfood.sh: cannot lock $lock: $!\n";
	}
	exec @command or die "dogfood.sh: cannot run $command[0]: $!\n";
' "$lock" "$@"
