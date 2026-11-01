#!/bin/sh
# ABOUTME: Caps the checkout's build directory (ADR-156 §6): over CAP, cargo-sweep removes its oldest artifacts until it is
# ABOUTME: under, holding the heavy lock; under CAP it does nothing. `make check` runs it first, at `TARGET_CAP`.
#
# Usage: scripts/target-sweep.sh CAP, where CAP is a whole number of MB or GB, such as 40GB.
#
# target/ grows with every build: each dependency change, toolchain, and profile leaves its artifacts
# beside the current ones, and the coverage build (target/cupel-llvm-cov) and the incremental caches
# add more. cargo-sweep (`make dev-setup` pins it) removes artifacts oldest first, in every profile
# under target/, the coverage build's included; the next build rebuilds what it still needs. The sweep
# takes the heavy lock, so a dogfood of this checkout is not reading the build while it goes, and
# turns off the lock's free-space floor, since it frees space.
set -u

usage() {
	echo "usage: scripts/target-sweep.sh CAP (a whole number of MB or GB, such as 40GB)" >&2
	exit 2
}

[ "$#" -eq 1 ] || usage
cap=$1
case $cap in
*[!0-9]*GB | *[!0-9]*MB | GB | MB) usage ;;
*GB) unit=GB kib_per_unit=1048576 ;;
*MB) unit=MB kib_per_unit=1024 ;;
*) usage ;;
esac
cap_kib=$((${cap%??} * kib_per_unit))

[ -d target ] || exit 0
held_kib=$(du -sk target | awk '{ print $1 }')
[ "$held_kib" -gt "$cap_kib" ] || exit 0

if ! command -v cargo-sweep >/dev/null 2>&1; then
	echo "target-sweep.sh: target/ holds $((held_kib / kib_per_unit)) $unit, over the $cap cap, and cargo-sweep is not on PATH; run \`make dev-setup\`" >&2
	exit 1
fi
echo "target-sweep.sh: target/ holds $((held_kib / kib_per_unit)) $unit, over the $cap cap; sweeping its oldest artifacts" >&2
CUPEL_HEAVY_MIN_FREE_GB=0 "$(dirname "$0")/heavy.sh" cargo-sweep sweep --maxsize "$cap" .
