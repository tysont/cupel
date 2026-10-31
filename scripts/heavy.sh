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
# Two at once starve each other, so every heavy command on the machine takes this lock, and so does an
# agent checkout's `make check` around its build step (ADR-156 §3).
lock=${CUPEL_HEAVY_LOCK:-/tmp/cupel-heavy.lock}

# A command run under this lock by a command that holds it (a check inside a chain or a dogfood that
# took it) runs at once: waiting would wait on its own caller forever. The holder names its lock in
# CUPEL_HEAVY_HELD; a call naming another lock waits as any does (ADR-156 §4).
if [ "${CUPEL_HEAVY_HELD:-}" = "$lock" ]; then
	exec "$@"
fi

# A heavy command that runs out of disk corrupts what it was building (a coverage build, a mutant
# slot, a clone's build) and takes the machine's other work with it. A fresh checkout's build is 15 to
# 30 GB and a coverage run doubles it, so the floor is 40 GB of free space on the working tree's
# volume; CUPEL_HEAVY_MIN_FREE_GB moves it (0 turns the check off).
min_free_gb=${CUPEL_HEAVY_MIN_FREE_GB:-40}
free_gb=$(($(df -Pk . | awk 'NR == 2 { print $4 }') / 1048576))
[ "$free_gb" -ge "$min_free_gb" ] ||
	fail "$free_gb GB free on this volume, below the $min_free_gb GB a heavy command needs; run \`make reclaim\` in idle checkouts, or remove merged worktrees with \`scripts/worktree.sh remove NAME\`"

# The queue has two priorities. The integrator's merge chains set CUPEL_HEAVY_PRIORITY=first and take
# the lock before any waiting command without it, however long that one has waited: a prioritized
# waiter holds a shared lock on the turnstile beside the lock while it waits, and a default waiter
# tries the lock only in a moment when it can take the turnstile exclusively, which it cannot while
# any prioritized waiter is pending.
priority=${CUPEL_HEAVY_PRIORITY:-default}

export CUPEL_HEAVY_HELD="$lock"

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
