// ABOUTME: The store's schema as an ordered list of embedded SQL migrations; the store's version is
// ABOUTME: how many have been applied (ADR-045 §4). Append new scripts; never edit an applied one.

/// Every migration, in order. Migration `n` (1-based) moves the store from version `n - 1` to `n`.
pub const MIGRATIONS: &[&str] = &[V1, V2];

/// Every table of design §8.8, with the ADR-045 §5 additions.
const V1: &str = "
CREATE TABLE facts (
    cache_key TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    version INTEGER NOT NULL,
    producer TEXT NOT NULL,
    lang TEXT,
    run_id TEXT,
    payload BLOB NOT NULL
);
CREATE TABLE symbols (
    hash TEXT PRIMARY KEY,
    lang TEXT NOT NULL,
    file TEXT NOT NULL,
    name TEXT NOT NULL,
    ns TEXT NOT NULL,
    visibility TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    first_seen_run TEXT NOT NULL,
    last_seen_run TEXT NOT NULL
);
CREATE TABLE metrics (
    symbol_hash TEXT,
    check_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value REAL NOT NULL,
    run_id TEXT NOT NULL
);
CREATE INDEX metrics_by_run ON metrics (run_id, check_id);
CREATE TABLE coverage (
    run_id TEXT NOT NULL,
    file TEXT NOT NULL,
    line INTEGER NOT NULL,
    hits INTEGER NOT NULL,
    branch_id TEXT,
    test_id TEXT
);
CREATE TABLE mutants (
    symbol_hash TEXT NOT NULL,
    site_hash TEXT NOT NULL,
    operator TEXT NOT NULL,
    outcome TEXT NOT NULL,
    suite_hash TEXT NOT NULL,
    toolchain_hash TEXT NOT NULL,
    run_id TEXT NOT NULL,
    duration_ms INTEGER NOT NULL
);
CREATE TABLE test_results (
    run_id TEXT NOT NULL,
    test_id TEXT NOT NULL,
    outcome TEXT NOT NULL,
    duration_ms INTEGER NOT NULL
);
CREATE TABLE runs (
    id TEXT PRIMARY KEY,
    verb TEXT,
    started TEXT NOT NULL,
    duration_ms INTEGER NOT NULL,
    git_sha TEXT,
    base_sha TEXT,
    depth TEXT,
    seed TEXT NOT NULL,
    config_hash TEXT NOT NULL,
    toolchain_hash TEXT NOT NULL,
    status TEXT NOT NULL,
    exit_code INTEGER NOT NULL,
    envelope TEXT NOT NULL
);
CREATE INDEX runs_by_sha ON runs (git_sha);
CREATE TABLE checks (
    run_id TEXT NOT NULL,
    check_id TEXT NOT NULL,
    json TEXT NOT NULL,
    PRIMARY KEY (run_id, check_id)
);
CREATE INDEX checks_by_check ON checks (check_id, run_id);
CREATE TABLE findings (
    id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    check_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    symbol_hash TEXT,
    severity TEXT NOT NULL,
    json TEXT NOT NULL,
    PRIMARY KEY (id, run_id)
);
CREATE TABLE baselines (
    name TEXT PRIMARY KEY,
    run_id TEXT NOT NULL
);
CREATE TABLE oracles (
    name TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    spec TEXT NOT NULL,
    verified_at TEXT,
    verified_hash TEXT
);
CREATE TABLE suppressions (
    finding TEXT PRIMARY KEY,
    reason TEXT NOT NULL,
    until TEXT,
    created TEXT NOT NULL
);
";

/// The `test_results` index of P3.1 (ADR-076 §7): language, runner, and file per row, and a
/// duration that runners which measure none leave null. The V1 table was never written.
const V2: &str = "
DROP TABLE test_results;
CREATE TABLE test_results (
    run_id TEXT NOT NULL,
    lang TEXT NOT NULL,
    runner TEXT NOT NULL,
    test_id TEXT NOT NULL,
    file TEXT,
    outcome TEXT NOT NULL,
    duration_ms INTEGER
);
CREATE INDEX test_results_by_run ON test_results (run_id, lang, test_id);
";
