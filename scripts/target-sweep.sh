#!/bin/sh
# ABOUTME: Caps the checkout's builds, target/ and its mutation slots' (ADR-156 §6, ADR-159): over CAP, cargo-sweep removes
# ABOUTME: their oldest artifacts until they are under, holding the heavy lock; under CAP it does nothing. `make check` runs it first.
#
# Usage: scripts/target-sweep.sh CAP, where CAP is a whole number of MB or GB, such as 40GB.
#
# target/ grows with every build: each dependency change, toolchain, and profile leaves its artifacts
# beside the current ones, and the coverage build (target/cupel-llvm-cov) and the incremental caches
# add more. cargo-sweep (`make dev-setup` pins it) removes artifacts oldest first, in every profile
# under target/, the coverage build's included; the next build rebuilds what it still needs. The sweep
# takes the heavy lock, so a dogfood of this checkout is not reading the build while it goes, and
# turns off the lock's free-space floor, since it frees space. Each mutation slot keeps a build of its
# own in .cupel/work/slots/<lang>/<n>/target, which grows the same way and counts toward the cap.
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

# The checkout's builds: target/ and each mutation slot's (ADR-159), each as the project directory cargo-sweep
# sweeps and the build directory in it.
builds() {
	[ -d target ] && echo .
	for slot in .cupel/work/slots/*/*; do
		[ -d "$slot/target" ] && echo "$slot"
	done
}

held_kib=0
for project in $(builds); do
	held_kib=$((held_kib + $(du -sk "$project/target" | awk '{ print $1 }')))
done
[ "$held_kib" -gt "$cap_kib" ] || exit 0

if ! command -v cargo-sweep >/dev/null 2>&1; then
	echo "target-sweep.sh: the builds hold $((held_kib / kib_per_unit)) $unit, over the $cap cap, and cargo-sweep is not on PATH; run \`make dev-setup\`" >&2
	exit 1
fi
echo "target-sweep.sh: the builds hold $((held_kib / kib_per_unit)) $unit, over the $cap cap; sweeping the oldest artifacts of each to its share of the cap" >&2
# Each build's share of the cap is in proportion to what it holds, in whole MB, rounded to the nearest.
cap_mb=$((cap_kib / 1024))
for project in $(builds); do
	kib=$(du -sk "$project/target" | awk '{ print $1 }')
	share=$(((2 * cap_mb * kib + held_kib) / (2 * held_kib)))
	CUPEL_HEAVY_MIN_FREE_GB=0 "$(dirname "$0")/heavy.sh" cargo-sweep sweep --maxsize "${share}MB" "$project" || exit
done
