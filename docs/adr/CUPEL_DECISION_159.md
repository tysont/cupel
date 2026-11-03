# CUPEL_DECISION_159: Mutation slots in the repository's `.cupel/work/slots/`, and a bound on socket paths

**Date:** 2026-11-02 · **Status:** accepted (the owner's v0.2.4 ruling); amends [080](CUPEL_DECISION_080.md) §8 (where slots live), [141](CUPEL_DECISION_141.md) §3 (no reclaim across repositories), and [156](CUPEL_DECISION_156.md) §5, §6, and §9 (worktree removal, the build cap, the guard) · **Phase:** B.2

## Context

ADR-080 §8 placed each repository's mutation slots under the system temporary directory, `$TMPDIR/cupel-mutate/<blake3 of the repository path>/<lang>/<n>/`, so no runner in the repository would discover them. ADR-141 §3 then bounded their disk: each production claimed its repository's slot directory under a shared lock and removed other repositories' directories that were gone, idle for 7 days, or beyond 32 GiB in all. The housekeeping batch after v0.2.3 found two failures that follow from that placement.

1. **The OS prunes the slots.** macOS's `dirhelper` runs daily at 03:35 and deletes files in `$TMPDIR` older than 3 days. It deleted build-script outputs (`libsqlite3-sys`'s `out/bindgen.rs`, `tree-sitter`'s `out/stdlib-symbols.txt`) from the Rust slot of Cupel's own changed dogfood. The files were made that night, so they were probably copies that kept older timestamps. Cargo's fingerprints still read fresh, so every later build in the slot failed and `tests.mutation` errored until the slot was removed by hand. Linux clears `/tmp` at boot and, under systemd, by age. A slot is a build cache that only Cupel should remove.
2. **Concurrent claims removed each other's slots.** CI's `check (ubuntu-latest)` failed once in `cupel-mutate::phases a_runner_that_builds_once_serves_every_slot_from_one_build_in_the_first`: its slot sync hit ENOENT in `$TMPDIR/cupel-mutate/<key>/rust/0`. The suite's seven tests claim slots at once in one process. A claim reclaiming other repositories' directories treated one with no `lock` file and no `repository` record as a gone repository's (the rule for slots an earlier, lockless Cupel made). But that is exactly the state of a directory another claim has just made and not yet locked, so it removed the directory while the other claim built in it.
   - A test of sixteen claims at once, 200 rounds, fails on main's `slots.rs` in 20 of 20 Linux runs (16 test threads, `rust:1.92-bookworm`) and in each of 5 runs on macOS. It reports lost builds and ENOENT.
   - The suite's own binary run 200 times at 16 threads did not hit it: the window is the microseconds between `create_dir_all` and the lock file's creation.
   - Neither of the owner's two candidates was the cause. Parallel tests never shared a slot path (each key is the hash of a distinct temporary directory), and no copy read a source directory being dropped.

The owner's v0.2.4 ruling: slots move out of `$TMPDIR` into `.cupel/work/slots/` (gitignored), covered by the existing target cap and cargo-sweep, never placed where the OS prunes files on its own schedule. The MCP daemon's socket path must keep comfortable headroom under macOS's 103-character limit, with a test that fails if any socket path Cupel creates exceeds 100 characters.

**Socket paths.** On macOS a Unix socket path may hold 103 characters. A daemon's socket is `<repository>/.cupel/serve.sock`. Where Cupel's own runs place a test's repository decides the length:

| Placement | Length |
|---|---|
| A mutation run's test repository, unguarded: canonical `$TMPDIR` (57), the run's `cmXXXX/` (7), `.tmpXXXXXX/` (11), `.cupel/serve.sock` (17) | 92 |
| The same under the guard as v0.2.3 left it, `$TMPDIR/c.XXXXXX/` (9) | 101 |
| The same under the guard made in `/tmp` (`/private/tmp/` 13, `c.XXXXXX/` 9) | 57 |

The run's scratch directory (`cmXXXX`) stays in the caller's `$TMPDIR`, not in the slot. A run's processes need a temporary directory, the slot's path is as long as the repository's, and a scratch directory is removed when its run ends, so the OS pruning it is no loss. So moving the slots does not move the sockets. The guard's directory is what has to shrink.

## Options

**Where the slots live.**
- (a) `$TMPDIR`, as before: pruned by the OS.
- (b) A per-user cache directory (`~/Library/Caches`, `$XDG_CACHE_HOME`): macOS also purges `~/Library/Caches` under disk pressure, and the slots would still need a cross-repository reclaim.
- (c) The repository's `.cupel/work/slots/`:
  - Never pruned by the OS.
  - Removed with the repository or a worktree.
  - Ignored by `.cupel/.gitignore` (`*`).
  - Each repository's slots are its own, so no claim ever touches another's.
  - The cost: a slot is inside the repository, so a tool that searches upward from its working directory (cargo's `.cargo/config.toml`, git's `.git`, Node's `node_modules`, a `go.work`) finds the repository's after the slot's own, where under `$TMPDIR` it found nothing.

The owner's ruling is (c).

**The race.** With (c) there is no cross-repository reclaim, so the race has no cause left: a claim makes its own repository's directory and touches no other. Fixing the reclaim rule instead (for example, by making a claim's directory, lock, and record atomically with a rename) would keep machinery whose only job, bounding slots shared across repositories in one directory, no longer exists.

**The bound on sockets.** A check in the daemon's `bind` would also fire for a user whose repository path is long, where `cupel serve` already reports a failed bind with the fix (`--socket` a shorter path). The bound is a property of where Cupel's own runs put test repositories, so the tests check it.

## Decision

1. **Slots live in `<repository>/.cupel/work/slots/<lang>/<n>/`** (`cupel_mutate::slots::{root, slot_dir}`).
   - `claim(repo)` makes the directory and returns it. There is no lock, no `repository` or `size` record, and no reclaim of other repositories' slots.
   - Slots are kept between runs, as before, so builds are incremental.
   - The overlay already treats the tree's top-level `.cupel` as the slot's own, so a sync never walks into the slots or links the tree's `.cupel` into a slot (tested).
   - Two productions in one repository still share its slots, as they did before this decision.
2. **`cupel doctor`** reports this repository's slots: `doctor.mutation-slots` (info, `review`), with `{root, bytes}` and the title `Mutation slots hold <size> in <root>`, when the directory exists.
3. **The build cap covers the slots** (ADR-156 §6 amended).
   - `scripts/target-sweep.sh` counts `target/` and each slot's `target/` against `TARGET_CAP`.
   - Over the cap, it sweeps each build with cargo-sweep to its share of the cap, in proportion to what it holds.
   - `TARGET_CAP` is 96 GB: one check and dogfood build about 53 GB, and a changed dogfood's Rust slot about 39 GB beside them.
4. **`scripts/worktree.sh remove`** no longer looks in `$TMPDIR`: a worktree's slots are inside it and go with it (ADR-156 §5 amended).
5. **`scripts/tmpdir-guard.sh`** makes its directory under `/tmp`, whatever the caller's `TMPDIR`, and has no `--slots-of` (ADR-156 §9 amended).
   - Lending slots to a guarded run, by renaming them into its directory on the same volume, existed only because slots lived in `$TMPDIR`.
   - Every entry a command leaves is a leak, `cupel-mutate` included.
   - `make dogfood` runs its gate under the guard without the flag.
6. **The socket bound.**
   - `cupel_fixtures::assert_socket_fits` fails when a socket path is longer than `SOCKET_PATH_MAX`, 100 characters.
   - Every test that makes Cupel create a socket checks its path through it before starting: cupel-cli's `support::daemon_socket` (the daemon and MCP tests, the stale-socket tests) and the parity harness's daemon.
   - cupel-serve's own server tests check the same bound inline, since cupel-fixtures depends on cupel-serve.
   - Under a `TMPDIR` nested as a guarded mutation run's was in v0.2.3, `serve_mcp_runs_beside_a_socket_daemon_and_takes_no_socket_flags` now fails at once, naming the 101-character path, where before it waited 120 s for a daemon that never listened.
7. **The regression test** is `cupel-mutate/tests/slots.rs` `claims_made_at_once_keep_the_slots_each_made`: sixteen claims at once through `claim(repo)`, each writing a build into its first slot while all are held, 200 rounds.

## Consequences

- No OS schedule removes a slot; only removing the repository, the worktree, the directory, or a sweep over the cap does.
- Slots no longer have a machine-wide total. Each repository holds its own, as many slots as the engine's jobs, and for Rust one shared build in the first. `cupel doctor` shows how much.
- Slots left by an earlier Cupel under `$TMPDIR/cupel-mutate/` are not migrated. The OS's own cleanup of `$TMPDIR` removes them, the failure this decision exists for, and removing the directory frees the space at once.
- A tool run in a slot that searches upward past the slot's own files reaches the repository's (a `.cargo/config.toml` above the slot, `.git`, `node_modules`, `go.work`). Where the slot holds its own copy, which it does for everything the tree tracks, that copy is found first. Cupel's own repository has no `.cargo/config.toml`. A git command run in a slot now finds the repository, in which the slot's files are ignored, where under `$TMPDIR` it found none.
- The socket bound holds where Cupel's runs place test repositories: 92 characters for an unguarded mutation run's, 57 for a guarded one's, so a new nesting level or a longer socket name fails a test instead of a daemon that never listens.
