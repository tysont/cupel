# CUPEL_DECISION_045: The fact store: file, locking, migrations, and what it indexes

**Date:** 2026-10-13 · **Status:** accepted · **Phase:** P2.2

## Context

Design §8.8 specifies a SQLite fact store at `.cupel/cupel.db`, in WAL mode, with a single writer ("the daemon when present, else the CLI process holding a lock"), and lists its tables. It does not say how the store relates to the `Writer` (ADR-020), what the lock is, how the schema evolves, what happens to the JSON run records once the store holds runs (ADR-020 left that to P2.2), how a failed store write is reported, or how several queries P2.2 needs (`explain <check-id>`, "the latest run", `--since-baseline`) are answered from the columns given.

## Options

**Opening.** Let `cupel-store` open any path it is handed (simple, but it bypasses the one rule every write obeys), or open only through a `Writer`, creating `.cupel/` with the `Writer`'s own `create_dir` so the `.cupel/` rule and `.gitignore` creation apply. A recording (dry-run) writer must create nothing, so it either cannot open the store or opens an existing one read-only.

**Locking.** SQLite alone serializes writers per statement and WAL lets readers run beside a writer, so a busy timeout suffices for single statements; but "single writer" in the design is about the process that owns writes (the daemon in P4), and a run's write (a run, its findings, checks, and metrics) must be one unit. Options: hold an exclusive lock for the whole process lifetime (an `explain` would block behind a long `check`), rely on SQLite's `BEGIN IMMEDIATE` only (no way for a later daemon to claim ownership), or take an advisory file lock around each write transaction.

**Migrations.** `PRAGMA user_version`, or a `schema_version` table. Both work; a table is visible to anyone inspecting the database with ordinary SQL and is what the plan names.

**Run records.** Delete the `.cupel/runs/<id>.json` files now that the store holds every envelope, or keep both.

## Decision

1. **Opening.** `Store::open(&Writer)` creates `.cupel/` through `Writer::create_dir` (so a writer that may not write `.cupel/`, such as a check's scratch writer, cannot open the store) and opens `.cupel/cupel.db` read-write; it refuses a recording writer. `Store::open_existing(&Writer)` opens an existing database read-only and returns `None` when there is none; dry runs use it to read without creating anything. `Writer::is_recording` is added to core for this.
2. **Connection settings.** `journal_mode = WAL`, `synchronous = NORMAL`, and a 5-second busy timeout. The WAL and shared-memory files (`cupel.db-wal`, `cupel.db-shm`) live beside the database inside `.cupel/`, which `.cupel/.gitignore` already ignores; SQLite removes them when the last connection closes.
3. **Single writer.** Every write transaction holds an exclusive advisory lock on `.cupel/cupel.lock` (`std::fs::File::try_lock`, retried every 10 ms for up to 5 seconds, then `StoreError::Locked`) and runs inside `BEGIN IMMEDIATE`. Readers take no lock and see a consistent WAL snapshot. The lock file is created on first use and never deleted (deleting lock files races). P4.2's daemon holds the same lock for its lifetime, which is how a CLI process will know to proxy rather than write.
4. **Migrations.** Migrations are an ordered list of SQL scripts embedded in the binary. `schema_version(version)` holds one row: the number of scripts applied. Opening applies every script past that number, each in its own transaction under the writer lock, so opening an up-to-date store changes nothing and two processes racing on a fresh store apply each script once. A store whose version is newer than the binary knows is refused (`StoreError::TooNew`) rather than written with an older schema.
5. **Tables.** Every §8.8 table exists from migration 1: `facts`, `symbols`, `metrics`, `coverage`, `mutants`, `test_results`, `runs`, `findings`, `baselines`, `oracles`, `suppressions`. `coverage`, `mutants`, `test_results`, and `oracles` stay empty until P3 and P5. Deviations from the design's column lists, each needed by a P2.2 query:
   - `runs` gains `status` and `exit_code`, so "the latest result run" (ADR-046) is a query; `seed` is stored as text because SQLite integers are signed and seeds use all 64 bits.
   - `symbols` gains `first_seen_run`, so "symbols whose structural hash changed since the baseline" is `first_seen_run > baseline` over the hashes in scope.
   - `metrics.symbol_hash` is nullable: a check-level score is a metric with no symbol (`key = 'score'`).
   - A `checks (run_id, check_id, json)` table indexes each run's check results, so `explain <check-id>` finds the last run of a check without parsing every stored envelope.
   - Columns the design names `check` are `check_id`, since `CHECK` is an SQL keyword.
   - `findings` has the primary key `(id, run_id)`; a run that emits the same finding twice keeps the first.
6. **`facts`.** Keyed by the §8.3.6 cache key: `CacheKey { kind, version, producer, inputs, toolchain_hash, config_hash }`, whose `digest()` is the BLAKE3 hex of the NUL-joined parts with inputs sorted. P2.2 provides `put_fact`/`fact`; the scheduler and producers start writing when they exist.
7. **Recording.** `Store::record_run(&Envelope)` writes the `runs` row (with the full envelope as JSON), one `findings` row per finding, one `checks` row per check result, and one `metrics` row per scored check, in one transaction; recording a run ID twice replaces it. Per-symbol metrics have no place in the envelope; `record_metrics` stores them once the scheduler produces them.
8. **The persistence hook.** Every invocation that writes a run record also records the run in the store, after the run record, under the same rules (not `describe`, not a dry run or preview). A store failure is one stderr line, `cupel: could not record the run in the fact store: …` (also under `--quiet`), and changes neither the envelope nor the exit code, exactly as a run-record failure does.
9. **Run records stay.** `.cupel/runs/<id>.json` keeps being written: it is plain JSON an agent can read without SQLite, and P0.7's tests and tooling rely on it. Neither the store nor the records are pruned yet; whichever step first needs pruning (P3.4's scratch cleanup at the latest) adds it to both.
10. **Store errors in verbs.** A verb that cannot open or query the store (`explain`, `baseline`, `suppress`) reports a `cli.usage` finding with subject `.cupel/cupel.db` whose fix points at `cupel doctor`, exit 2.

## Consequences

- No Cupel process writes the store outside the `.cupel/` rule, and a dry run never creates it.
- Concurrent CLI invocations in one repository are safe; a writer waits at most about five seconds for another.
- New tables or columns are one appended migration script; an older binary refuses a newer store instead of corrupting it.
- `.cupel/` holds `cupel.db` and `cupel.lock` after any recorded run; tests that list `.cupel/` include them.
