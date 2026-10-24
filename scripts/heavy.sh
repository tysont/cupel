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

# One lock for the machine: a tree dogfood grades a whole tree and builds mutants in slots that each
# hold a build of the workspace; an external suite run for the dogfood of another project is as heavy.
# Two at once starve each other, so every heavy command on the machine takes this lock. (`make check`
# does not: Cupel's own suite is deterministic under load, ADR-145.)
lock=${CUPEL_HEAVY_LOCK:-/tmp/cupel-heavy.lock}

# The queue has two priorities. The integrator's merge chains set CUPEL_HEAVY_PRIORITY=first and take
# the lock before any waiting command without it, however long that one has waited: a prioritized
# waiter holds a shared lock on the turnstile beside the lock while it waits, and a default waiter
# tries the lock only in a moment when it can take the turnstile exclusively, which it cannot while
# any prioritized waiter is pending.
priority=${CUPEL_HEAVY_PRIORITY:-default}

# flock(1) is not on macOS; perl is on every platform the Makefile runs on. The lock lives on the
# descriptor, which must survive the exec ($^F keeps it open across it) so the command holds it; the
# turnstile is closed before the exec, so a running command holds only the lock.
exec perl -MFcntl=:flock -e '
	my ($lock, $priority, @command) = @ARGV;
	$^F = 1023;
	open(my $fh, ">>", $lock) or die "heavy.sh: cannot open $lock: $!\n";
	open(my $turnstile, ">>", "$lock.turnstile") or die "heavy.sh: cannot open $lock.turnstile: $!\n";
	my $waiting = sub { print STDERR "heavy.sh: waiting for $lock (another heavy command is running)\n" };
	if ($priority eq "first") {
		flock($turnstile, LOCK_SH) or die "heavy.sh: cannot lock $lock.turnstile: $!\n";
		unless (flock($fh, LOCK_EX | LOCK_NB)) {
			$waiting->();
			flock($fh, LOCK_EX) or die "heavy.sh: cannot lock $lock: $!\n";
		}
	} else {
		my $said = 0;
		until (flock($turnstile, LOCK_EX | LOCK_NB) && flock($fh, LOCK_EX | LOCK_NB)) {
			flock($turnstile, LOCK_UN);
			$waiting->() unless $said++;
			sleep 1;
		}
	}
	close($turnstile);
	exec @command or die "heavy.sh: cannot run $command[0]: $!\n";
' "$lock" "$priority" "$@"
