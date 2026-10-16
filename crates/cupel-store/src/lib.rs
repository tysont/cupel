// ABOUTME: Root of cupel-store: the SQLite fact store at `.cupel/cupel.db` (design §8.8, ADR-045).
// ABOUTME: Opens through a `Writer`, migrates on open, and offers typed reads and single-writer writes.
//! The fact store.
//!
//! [`Store::open`] opens (creating if needed) `.cupel/cupel.db` for the repository a
//! [`Writer`] belongs to, in WAL mode, and applies any pending migrations;
//! [`Store::open_existing`] opens an existing store read-only, for dry runs. Every write runs in
//! one transaction while holding the exclusive advisory lock `.cupel/cupel.lock`, so there is
//! one writer at a time; readers never wait. See ADR-045 for the schema and its deviations
//! from design §8.8.
//!
//! What the store holds:
//!
//! - **Runs.** [`Store::record_run`] stores an envelope with its findings, check results, and
//!   check scores; [`Store::run`], [`Store::finding`], [`Store::last_run_for_check`],
//!   [`Store::last_result_run`], and [`Store::last_result_run_at`] read them back.
//! - **Baselines.** [`Store::baseline`] and [`Store::set_baseline`] (ADR-046).
//! - **Per-symbol metrics.** [`Store::record_metrics`], for the scheduler to fill.
//! - **Facts.** [`Store::put_fact`] and [`Store::fact`] by [`CacheKey::digest`] (design §8.3.6).
//! - **Symbols.** [`Store::record_symbols`] and [`Store::symbols_new_since`], which answers
//!   `--since-baseline`.
//! - **Test results.** [`Store::record_test_results`] indexes a run's `TestResults` per test;
//!   [`Store::test_results`] reads a run's rows back (ADR-076).
//! - **Mutants.** [`Store::record_mutants`] indexes a run's `MutationOutcomes` per mutant;
//!   [`Store::mutants_by_key`] finds stored outcomes by cache key and
//!   [`Store::last_mutant_outcomes`] the last outcome of each site (ADR-080).
//! - **Durations.** [`Store::record_durations`] keeps how long a run's productions and check
//!   runs took; [`Store::recent_durations`] reads each task shape's latest, and
//!   [`Store::latest_scored`] where each check last ran, for the budget scheduler (ADR-086).
//! - **Suppressions.** [`Store::replace_suppressions`] mirrors `.cupel/suppressions.toml`
//!   (ADR-047); [`Store::suppressions`] reads the mirror.

mod error;
mod lock;
mod migrations;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Duration;

use cupel_core::budget::{RECENT, TaskDuration, TaskKind, TaskShape};
use cupel_core::syntax::SymbolDef;
use cupel_core::{
    CheckId, CheckResult, Envelope, FactKind, Finding, FindingId, LangId, Metric, Mutant,
    MutantOutcome, MutationOutcomes, ProducerId, RunId, Suppression, SymbolHash, TestResults, Verb,
    Writer,
};
use rusqlite::types::Value;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};

pub use crate::error::StoreError;
use crate::lock::WriterLock;
use crate::migrations::MIGRATIONS;

pub type Result<T, E = StoreError> = std::result::Result<T, E>;

/// The last run of one check: its result, its findings in that run, and its metrics.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckRun {
    pub run: RunId,
    pub result: CheckResult,
    pub findings: Vec<Finding>,
    /// Check-level metrics (such as its score) first, then per-symbol metrics, each in the order
    /// they were recorded.
    pub metrics: Vec<Metric>,
}

/// One row of the `test_results` index: a test as one run's baseline reported it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredTestResult {
    pub lang: LangId,
    pub runner: ProducerId,
    /// The test's ID in its `TestResults`.
    pub test: String,
    pub file: Option<String>,
    /// `passed`, `failed`, `skipped`, or `errored`.
    pub outcome: String,
    pub duration_ms: Option<u64>,
}

/// The cache key of a fact (design §8.3.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheKey {
    pub kind: FactKind,
    pub version: u32,
    pub producer: ProducerId,
    /// Hashes of everything the producer read, such as file contents.
    pub inputs: Vec<String>,
    pub toolchain_hash: String,
    pub config_hash: String,
}

impl CacheKey {
    /// The key as 64 hex digits: BLAKE3 of the NUL-separated parts, inputs sorted so their order
    /// does not matter.
    pub fn digest(&self) -> String {
        let mut inputs = self.inputs.clone();
        inputs.sort();
        let mut hasher = blake3::Hasher::new();
        let version = self.version.to_string();
        let parts = [
            self.kind.as_str(),
            version.as_str(),
            self.producer.as_str(),
            self.toolchain_hash.as_str(),
            self.config_hash.as_str(),
        ];
        for part in parts
            .iter()
            .copied()
            .chain(inputs.iter().map(String::as_str))
        {
            hasher.update(part.as_bytes());
            hasher.update(&[0]);
        }
        hasher.finalize().to_hex().to_string()
    }
}

/// One cached fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFact {
    /// The [`CacheKey::digest`] it is stored under.
    pub key: String,
    pub kind: FactKind,
    pub version: u32,
    pub producer: ProducerId,
    pub lang: Option<LangId>,
    /// The run that produced it.
    pub run: Option<RunId>,
    pub payload: Vec<u8>,
}

/// An open fact store.
#[derive(Debug)]
pub struct Store {
    connection: Connection,
    lock: PathBuf,
    read_only: bool,
}

impl Store {
    /// The database's file name inside `.cupel/`.
    pub const FILE: &'static str = "cupel.db";
    /// The writer lock's file name inside `.cupel/`.
    pub const LOCK: &'static str = "cupel.lock";
    /// The schema version this build migrates to.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "the migration list is a handful of entries"
    )]
    pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

    /// Opens the store of `writer`'s repository for reading and writing, creating `.cupel/` and
    /// the database as needed and applying pending migrations. A recording writer is refused.
    pub fn open(writer: &Writer) -> Result<Self> {
        if writer.is_recording() {
            return Err(StoreError::DryRun);
        }
        let dir = writer.create_dir(&writer.cupel_dir())?;
        let path = dir.join(Self::FILE);
        let lock = dir.join(Self::LOCK);
        let connection = Connection::open(&path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        {
            // Switching to WAL needs the database to itself and does not wait on the busy
            // timeout, so concurrent first opens take turns.
            let _lock = WriterLock::acquire(&lock)?;
            connection.pragma_update(None, "journal_mode", "WAL")?;
        }
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        let store = Self {
            connection,
            lock,
            read_only: false,
        };
        store.migrate()?;
        Ok(store)
    }

    /// Opens an existing store read-only; `None` when the repository has none. Creates nothing,
    /// so dry runs use it.
    pub fn open_existing(writer: &Writer) -> Result<Option<Self>> {
        let dir = writer.cupel_dir();
        let path = dir.join(Self::FILE);
        if !path.is_file() {
            return Ok(None);
        }
        // `immutable=1` keeps SQLite from creating the WAL and shared-memory files a read-only
        // WAL connection otherwise leaves behind; a dry run must change nothing on disk. It reads
        // the database as last checkpointed, which every closed writer leaves it.
        let uri = format!("file:{}?immutable=1", uri_path(&path));
        let connection = Connection::open_with_flags(
            uri,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        connection.busy_timeout(Duration::from_secs(5))?;
        let store = Self {
            connection,
            lock: dir.join(Self::LOCK),
            read_only: true,
        };
        let found = store.schema_version()?;
        if found > Self::SCHEMA_VERSION {
            return Err(StoreError::TooNew {
                found,
                known: Self::SCHEMA_VERSION,
            });
        }
        Ok(Some(store))
    }

    /// The number of migrations applied.
    pub fn schema_version(&self) -> Result<u32> {
        let exists: bool = self.connection.query_row(
            "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_version')",
            [],
            |row| row.get(0),
        )?;
        if !exists {
            return Ok(0);
        }
        Ok(self
            .connection
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .optional()?
            .unwrap_or(0))
    }

    /// Applies every migration past the store's version, each in its own locked transaction.
    fn migrate(&self) -> Result<()> {
        loop {
            let applied = self.write(|tx| {
                tx.execute_batch(
                    "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);
                     INSERT INTO schema_version (version)
                        SELECT 0 WHERE NOT EXISTS (SELECT 1 FROM schema_version);",
                )?;
                let version: u32 =
                    tx.query_row("SELECT version FROM schema_version", [], |row| row.get(0))?;
                if version > Self::SCHEMA_VERSION {
                    return Err(StoreError::TooNew {
                        found: version,
                        known: Self::SCHEMA_VERSION,
                    });
                }
                let Some(script) = MIGRATIONS.get(version as usize) else {
                    return Ok(false);
                };
                tx.execute_batch(script)?;
                tx.execute("UPDATE schema_version SET version = ?1", [version + 1])?;
                Ok(true)
            })?;
            if !applied {
                return Ok(());
            }
        }
    }

    /// Runs `work` in one immediate transaction while holding the writer lock.
    fn write<T>(&self, work: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        if self.read_only {
            return Err(StoreError::Io {
                path: self.lock.clone(),
                message: "the fact store was opened read-only".to_owned(),
            });
        }
        let _lock = WriterLock::acquire(&self.lock)?;
        self.connection.execute_batch("BEGIN IMMEDIATE")?;
        match work(&self.connection) {
            Ok(value) => {
                self.connection.execute_batch("COMMIT")?;
                Ok(value)
            }
            Err(error) => {
                // The work's error is the one worth reporting; a failed rollback adds nothing.
                let _ = self.connection.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    /// Stores `envelope`: its run row with the whole envelope, its findings (the first of any
    /// repeated ID), its check results, and each scored check's score as a metric. Recording a
    /// run ID again replaces everything stored for it. Runs of `explain`, `baseline`, and
    /// `suppress` store only their envelope, since their findings repeat earlier runs'.
    pub fn record_run(&self, envelope: &Envelope) -> Result<()> {
        let run = envelope.run.id.to_string();
        let json = serde_json::to_string(envelope)?;
        let config_hash = hash_json(&envelope.run.config)?;
        let toolchain_hash = hash_json(&envelope.run.toolchain)?;
        self.write(|tx| {
            for table in ["findings", "checks", "metrics"] {
                tx.execute(&format!("DELETE FROM {table} WHERE run_id = ?1"), [&run])?;
            }
            tx.execute(
                "INSERT OR REPLACE INTO runs (id, verb, started, duration_ms, git_sha, base_sha,
                    depth, seed, config_hash, toolchain_hash, status, exit_code, envelope)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    run,
                    envelope.run.verb.map(name).transpose()?,
                    envelope.run.started,
                    i64::try_from(envelope.run.duration_ms).unwrap_or(i64::MAX),
                    envelope.run.git.as_ref().map(|git| git.sha.clone()),
                    envelope.run.git.as_ref().and_then(|git| git.base.clone()),
                    envelope.run.depth.map(name).transpose()?,
                    envelope.run.seed.to_string(),
                    config_hash,
                    toolchain_hash,
                    name(envelope.status)?,
                    envelope.exit_code.code(),
                    json,
                ],
            )?;
            if repeats_other_runs(envelope) {
                return Ok(());
            }
            for finding in &envelope.findings {
                tx.execute(
                    "INSERT OR IGNORE INTO findings
                        (id, run_id, check_id, kind, symbol_hash, severity, json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        finding.id.as_str(),
                        run,
                        finding.check.as_str(),
                        finding.kind,
                        finding.symbol.as_ref().map(|symbol| symbol.hash.as_str()),
                        name(finding.severity)?,
                        serde_json::to_string(finding)?,
                    ],
                )?;
            }
            for check in &envelope.checks {
                insert_check(tx, &run, check)?;
            }
            Ok(())
        })
    }

    /// Stores `metrics` of `check` in `run`, after any already stored.
    pub fn record_metrics(&self, run: &RunId, check: &CheckId, metrics: &[Metric]) -> Result<()> {
        let run = run.to_string();
        self.write(|tx| {
            for metric in metrics {
                insert_metric(tx, &run, check, metric)?;
            }
            Ok(())
        })
    }

    /// Indexes every test of `results`, which `run` produced for `lang`, in `test_results`.
    pub fn record_test_results(
        &self,
        run: &RunId,
        lang: &LangId,
        results: &TestResults,
    ) -> Result<()> {
        let fixed = [
            run.to_string(),
            lang.to_string(),
            results.runner.to_string(),
        ];
        self.insert_rows(
            "INSERT INTO test_results (run_id, lang, runner, test_id, file, outcome, duration_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            &fixed,
            results.tests.iter().map(|test| Ok(test_row(test))),
        )
    }

    /// The `test_results` rows of `run`, by language, then test ID.
    pub fn test_results(&self, run: &RunId) -> Result<Vec<StoredTestResult>> {
        let mut statement = self.connection.prepare(
            "SELECT lang, runner, test_id, file, outcome, duration_ms FROM test_results
             WHERE run_id = ?1 ORDER BY lang, test_id",
        )?;
        let rows = statement.query_map([run.to_string()], |row| {
            Ok(StoredTestResult {
                lang: LangId::from(row.get::<_, String>(0)?),
                runner: ProducerId::from(row.get::<_, String>(1)?),
                test: row.get(2)?,
                file: row.get(3)?,
                outcome: row.get(4)?,
                duration_ms: row
                    .get::<_, Option<i64>>(5)?
                    .and_then(|ms| u64::try_from(ms).ok()),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The stored envelope of `run`.
    pub fn run(&self, run: &RunId) -> Result<Option<Envelope>> {
        self.connection
            .query_row(
                "SELECT envelope FROM runs WHERE id = ?1",
                [run.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|json| serde_json::from_str(&json).map_err(StoreError::from))
            .transpose()
    }

    /// The finding `id` as stored by the latest run that produced it, with that run's ID.
    pub fn finding(&self, id: &FindingId) -> Result<Option<(Finding, RunId)>> {
        self.connection
            .query_row(
                "SELECT json, run_id FROM findings WHERE id = ?1 ORDER BY run_id DESC LIMIT 1",
                [id.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
            .map(|(json, run)| Ok((serde_json::from_str(&json)?, parse_run(&run)?)))
            .transpose()
    }

    /// The latest run of `check`: its result, findings, and metrics.
    pub fn last_run_for_check(&self, check: &CheckId) -> Result<Option<CheckRun>> {
        let Some((run, json)) = self
            .connection
            .query_row(
                "SELECT run_id, json FROM checks WHERE check_id = ?1 ORDER BY run_id DESC LIMIT 1",
                [check.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
        else {
            return Ok(None);
        };
        let findings = self.findings_of(&run, check)?;
        let metrics = self.metrics_of(&run, check)?;
        Ok(Some(CheckRun {
            run: parse_run(&run)?,
            result: serde_json::from_str(&json)?,
            findings,
            metrics,
        }))
    }

    /// The findings `check` reported in `run`, in the order recorded.
    fn findings_of(&self, run: &str, check: &CheckId) -> Result<Vec<Finding>> {
        let mut findings = self.connection.prepare(
            "SELECT json FROM findings WHERE run_id = ?1 AND check_id = ?2 ORDER BY rowid",
        )?;
        findings
            .query_map([run, check.as_str()], |row| row.get::<_, String>(0))?
            .map(|json| Ok(serde_json::from_str(&json?)?))
            .collect()
    }

    /// The metrics of `check` in `run`: check-level first, then per-symbol, each as recorded.
    fn metrics_of(&self, run: &str, check: &CheckId) -> Result<Vec<Metric>> {
        let mut metrics = self.connection.prepare(
            "SELECT symbol_hash, key, value FROM metrics WHERE run_id = ?1 AND check_id = ?2
             ORDER BY symbol_hash IS NOT NULL, rowid",
        )?;
        metrics
            .query_map([run, check.as_str()], |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, f64>(2)?,
                ))
            })?
            .map(|row| {
                let (symbol, key, value) = row?;
                Ok(Metric {
                    symbol: symbol.map(|hash| parse_symbol(&hash)).transpose()?,
                    key,
                    value,
                })
            })
            .collect()
    }

    /// The latest *result run*: a run of `init`, `doctor`, `check`, `survey`, or `tests` that
    /// did not end in a usage or internal error (ADR-046 §1).
    pub fn last_result_run(&self) -> Result<Option<RunId>> {
        self.result_run("", [])
    }

    /// The latest result run made at commit `sha`.
    pub fn last_result_run_at(&self, sha: &str) -> Result<Option<RunId>> {
        self.result_run("AND git_sha = ?1", [sha])
    }

    fn result_run<const N: usize>(&self, filter: &str, values: [&str; N]) -> Result<Option<RunId>> {
        let query = format!(
            "SELECT id FROM runs
             WHERE verb IN ('init', 'doctor', 'check', 'survey', 'tests')
               AND exit_code NOT IN (2, 6) {filter}
             ORDER BY id DESC LIMIT 1"
        );
        self.connection
            .query_row(&query, rusqlite::params_from_iter(values), |row| {
                row.get::<_, String>(0)
            })
            .optional()?
            .map(|run| parse_run(&run))
            .transpose()
    }

    /// The run the baseline `name` points at.
    pub fn baseline(&self, name: &str) -> Result<Option<RunId>> {
        self.connection
            .query_row(
                "SELECT run_id FROM baselines WHERE name = ?1",
                [name],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|run| parse_run(&run))
            .transpose()
    }

    /// Points the baseline `name` at `run`.
    pub fn set_baseline(&self, name: &str, run: &RunId) -> Result<()> {
        let run = run.to_string();
        self.write(|tx| {
            tx.execute(
                "INSERT OR REPLACE INTO baselines (name, run_id) VALUES (?1, ?2)",
                [name, run.as_str()],
            )?;
            Ok(())
        })
    }

    /// Stores `fact` under its key, replacing any fact stored there.
    pub fn put_fact(&self, fact: &StoredFact) -> Result<()> {
        self.write(|tx| {
            tx.execute(
                "INSERT OR REPLACE INTO facts (cache_key, kind, version, producer, lang, run_id, payload)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    fact.key,
                    fact.kind.as_str(),
                    fact.version,
                    fact.producer.as_str(),
                    fact.lang.as_ref().map(LangId::as_str),
                    fact.run.map(|run| run.to_string()),
                    fact.payload,
                ],
            )?;
            Ok(())
        })
    }

    /// The fact stored under `key`, a [`CacheKey::digest`].
    pub fn fact(&self, key: &str) -> Result<Option<StoredFact>> {
        let row = self
            .connection
            .query_row(
                "SELECT kind, version, producer, lang, run_id, payload FROM facts WHERE cache_key = ?1",
                [key],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, u32>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Vec<u8>>(5)?,
                    ))
                },
            )
            .optional()?;
        let Some((kind, version, producer, lang, run, payload)) = row else {
            return Ok(None);
        };
        Ok(Some(StoredFact {
            key: key.to_owned(),
            kind: serde_json::from_value(serde_json::Value::String(kind))?,
            version,
            producer: ProducerId::from(producer),
            lang: lang.map(LangId::from),
            run: run.map(|run| parse_run(&run)).transpose()?,
            payload,
        }))
    }

    /// Records the symbols seen in `run`: a hash not seen before is first seen in `run`; every
    /// hash's location and last sighting are updated.
    pub fn record_symbols(&self, run: &RunId, symbols: &[(SymbolHash, SymbolDef)]) -> Result<()> {
        let run = run.to_string();
        self.write(|tx| {
            for (hash, symbol) in symbols {
                tx.execute(
                    "INSERT INTO symbols (hash, lang, file, name, ns, visibility, start_line,
                        end_line, first_seen_run, last_seen_run)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
                     ON CONFLICT (hash) DO UPDATE SET lang = ?2, file = ?3, name = ?4, ns = ?5,
                        visibility = ?6, start_line = ?7, end_line = ?8, last_seen_run = ?9",
                    params![
                        hash.as_str(),
                        symbol.lang.as_str(),
                        symbol.file,
                        symbol.name,
                        symbol.namespace,
                        name(symbol.visibility)?,
                        symbol.span.line,
                        symbol.span.end_line,
                        run,
                    ],
                )?;
            }
            Ok(())
        })
    }

    /// Those of `hashes` not seen at or before `run`: structurally new or changed since then.
    /// This is what `--since-baseline` scopes to (ADR-046 §9). Order follows `hashes`.
    pub fn symbols_new_since(&self, run: &RunId, hashes: &[SymbolHash]) -> Result<Vec<SymbolHash>> {
        let run = run.to_string();
        let mut statement = self
            .connection
            .prepare("SELECT first_seen_run FROM symbols WHERE hash = ?1")?;
        let mut new = Vec::new();
        for hash in hashes {
            let first: Option<String> = statement
                .query_row([hash.as_str()], |row| row.get(0))
                .optional()?;
            if first.is_none_or(|first| first > run) {
                new.push(hash.clone());
            }
        }
        Ok(new)
    }

    /// Replaces the mirrored suppressions with `suppressions` (ADR-047 §1).
    pub fn replace_suppressions(&self, suppressions: &[Suppression]) -> Result<()> {
        self.write(|tx| {
            tx.execute("DELETE FROM suppressions", [])?;
            for suppression in suppressions {
                tx.execute(
                    "INSERT INTO suppressions (finding, reason, until, created) VALUES (?1, ?2, ?3, ?4)",
                    params![
                        suppression.finding.as_str(),
                        suppression.reason,
                        suppression.until.as_ref().map(ToString::to_string),
                        suppression.created,
                    ],
                )?;
            }
            Ok(())
        })
    }

    /// The mirrored suppressions, by finding ID.
    pub fn suppressions(&self) -> Result<Vec<Suppression>> {
        let mut statement = self
            .connection
            .prepare("SELECT finding, reason, until, created FROM suppressions ORDER BY finding")?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .map(|row| {
                let (finding, reason, until, created) = row?;
                Ok(Suppression {
                    finding: finding.parse().map_err(|error| invalid(&error))?,
                    reason,
                    until: until
                        .map(|until| until.parse().map_err(|error| invalid(&error)))
                        .transpose()?,
                    created,
                })
            })
            .collect()
    }
}

/// Where a check last produced a result that ran (not skipped): the commit and whether the
/// tree was dirty (ADR-086 §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoredAt {
    /// `None` outside a work tree or before the first commit.
    pub sha: Option<String>,
    pub dirty: bool,
}

impl Store {
    /// Stores how long each of `durations`, timed in `run`, took.
    pub fn record_durations(&self, run: &RunId, durations: &[TaskDuration]) -> Result<()> {
        self.insert_rows(
            "INSERT INTO durations (run_id, kind, task, lang, class, duration_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            &[run.to_string()],
            durations.iter().map(|timed| {
                let shape = &timed.shape;
                Ok(vec![
                    Value::from(shape.kind.as_str().to_owned()),
                    Value::from(shape.task.clone()),
                    Value::from(shape.lang.to_string()),
                    Value::from(shape.class.clone()),
                    Value::from(millis(Some(timed.duration_ms))),
                ])
            }),
        )
    }

    /// Every task shape's latest [`RECENT`] durations, newest first.
    pub fn recent_durations(&self) -> Result<Vec<(TaskShape, Vec<u64>)>> {
        let mut statement = self.connection.prepare(
            "SELECT kind, task, lang, class, duration_ms FROM (
                SELECT *, ROW_NUMBER() OVER (
                    PARTITION BY kind, task, lang, class ORDER BY rowid DESC) AS age
                FROM durations)
             WHERE age <= ?1 ORDER BY kind, task, lang, class, age",
        )?;
        let recent = i64::try_from(RECENT).unwrap_or(i64::MAX);
        let rows = statement.query_map([recent], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })?;
        let mut recent: Vec<(TaskShape, Vec<u64>)> = Vec::new();
        for row in rows {
            let (kind, task, lang, class, duration_ms) = row?;
            let kind = TaskKind::named(&kind).ok_or_else(|| StoreError::Value(kind.clone()))?;
            let shape = TaskShape {
                kind,
                task,
                lang: LangId::from(lang),
                class,
            };
            let duration_ms = u64::try_from(duration_ms).unwrap_or_default();
            match recent.last_mut() {
                Some((last, durations)) if *last == shape => durations.push(duration_ms),
                _ => recent.push((shape, vec![duration_ms])),
            }
        }
        Ok(recent)
    }

    /// For every check with a stored result that ran, where its latest such result was made.
    pub fn latest_scored(&self) -> Result<BTreeMap<CheckId, ScoredAt>> {
        let mut statement = self.connection.prepare(
            "SELECT checks.check_id, runs.git_sha,
                    json_extract(runs.envelope, '$.run.git.dirty')
             FROM checks JOIN runs ON runs.id = checks.run_id
             WHERE json_extract(checks.json, '$.status') != 'skipped'
             ORDER BY checks.run_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<bool>>(2)?,
            ))
        })?;
        let mut scored = BTreeMap::new();
        for row in rows {
            let (check, sha, dirty) = row?;
            scored.insert(
                CheckId::from(check),
                ScoredAt {
                    sha,
                    dirty: dirty.unwrap_or(false),
                },
            );
        }
        Ok(scored)
    }
}

/// A mutant's place across runs: its symbol's structural hash, its site, and its replacement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MutantSite {
    pub symbol_hash: String,
    pub site_hash: String,
    pub replacement: String,
}

impl Store {
    /// Indexes every mutant of `outcomes`, which `run` produced for `lang`, in `mutants`.
    pub fn record_mutants(
        &self,
        run: &RunId,
        lang: &LangId,
        outcomes: &MutationOutcomes,
    ) -> Result<()> {
        let fixed = [
            run.to_string(),
            lang.to_string(),
            outcomes.engine.to_string(),
        ];
        self.insert_rows(
            "INSERT INTO mutants (run_id, lang, engine, cache_key, symbol_hash, site_hash,
                replacement, outcome, duration_ms, mutant)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            &fixed,
            outcomes.mutants.iter().map(|mutant| {
                Ok(vec![
                    Value::from(mutant.cache_key.clone()),
                    Value::from(mutant.symbol_hash.as_ref().map(ToString::to_string)),
                    Value::from(mutant.site_hash.to_string()),
                    Value::from(mutant.to.clone()),
                    Value::from(mutant.outcome.as_str().to_owned()),
                    Value::from(millis(mutant.duration_ms)),
                    Value::from(serde_json::to_string(mutant)?),
                ])
            }),
        )
    }

    /// Inserts one row per item of `rows` with `sql` in one locked transaction: the `fixed`
    /// values, then the item's.
    fn insert_rows(
        &self,
        sql: &str,
        fixed: &[String],
        rows: impl Iterator<Item = Result<Vec<Value>>>,
    ) -> Result<()> {
        let rows: Vec<Vec<Value>> = rows.collect::<Result<_>>()?;
        self.write(|tx| {
            let mut statement = tx.prepare(sql)?;
            for row in rows {
                let values = fixed.iter().cloned().map(Value::from).chain(row);
                statement.execute(rusqlite::params_from_iter(values))?;
            }
            Ok(())
        })
    }

    /// The last mutant stored under each of `keys` that has one.
    pub fn mutants_by_key(&self, keys: &[String]) -> Result<BTreeMap<String, Mutant>> {
        let mut statement = self.connection.prepare(
            "SELECT mutant FROM mutants WHERE cache_key = ?1 ORDER BY rowid DESC LIMIT 1",
        )?;
        let mut found = BTreeMap::new();
        for key in keys {
            let stored: Option<String> = statement.query_row([key], |row| row.get(0)).optional()?;
            if let Some(stored) = stored {
                found.insert(key.clone(), serde_json::from_str(&stored)?);
            }
        }
        Ok(found)
    }

    /// The last stored outcome of every site and replacement whose symbol had a structural hash.
    pub fn last_mutant_outcomes(&self) -> Result<BTreeMap<MutantSite, MutantOutcome>> {
        let mut statement = self.connection.prepare(
            "SELECT symbol_hash, site_hash, replacement, outcome FROM mutants
             WHERE symbol_hash IS NOT NULL ORDER BY rowid",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                MutantSite {
                    symbol_hash: row.get(0)?,
                    site_hash: row.get(1)?,
                    replacement: row.get(2)?,
                },
                row.get::<_, String>(3)?,
            ))
        })?;
        let mut last = BTreeMap::new();
        for row in rows {
            let (site, outcome) = row?;
            let outcome = serde_json::from_value(serde_json::Value::String(outcome))?;
            last.insert(site, outcome);
        }
        Ok(last)
    }
}

/// `path` for a `file:` URI: the characters URIs give meaning to are percent-encoded.
/// The `test_results` values of `test` after its run, language, and runner.
fn test_row(test: &cupel_core::TestResult) -> Vec<Value> {
    vec![
        Value::from(test.id.clone()),
        Value::from(test.file.clone()),
        Value::from(test.outcome.as_str().to_owned()),
        Value::from(millis(test.duration_ms)),
    ]
}

/// A duration in milliseconds as the store keeps it, a signed integer.
fn millis(duration_ms: Option<u64>) -> Option<i64> {
    duration_ms.map(|ms| i64::try_from(ms).unwrap_or(i64::MAX))
}

fn uri_path(path: &std::path::Path) -> String {
    let mut encoded = String::new();
    for character in path.to_string_lossy().chars() {
        match character {
            '%' | '?' | '#' | ' ' => {
                let mut bytes = [0; 4];
                for byte in character.encode_utf8(&mut bytes).bytes() {
                    let _ = write!(encoded, "%{byte:02X}");
                }
            }
            other => encoded.push(other),
        }
    }
    encoded
}

/// Whether `envelope` is from a verb whose findings and checks repeat those of earlier runs
/// (`explain`, `baseline`, `suppress`). Its envelope is stored, but indexing its findings would
/// make it the latest run that produced them (ADR-045 §7).
fn repeats_other_runs(envelope: &Envelope) -> bool {
    matches!(
        envelope.run.verb,
        Some(Verb::Explain | Verb::Baseline | Verb::Suppress)
    )
}

/// Stores one check's result in `run`, and its score as a metric.
fn insert_check(tx: &Connection, run: &str, check: &CheckResult) -> Result<()> {
    tx.execute(
        "INSERT OR REPLACE INTO checks (run_id, check_id, json) VALUES (?1, ?2, ?3)",
        params![run, check.id.as_str(), serde_json::to_string(check)?],
    )?;
    if let Some(score) = &check.score {
        let metric = Metric {
            symbol: None,
            key: "score".to_owned(),
            value: score.value,
        };
        insert_metric(tx, run, &check.id, &metric)?;
    }
    Ok(())
}

fn insert_metric(tx: &Connection, run: &str, check: &CheckId, metric: &Metric) -> Result<()> {
    tx.execute(
        "INSERT INTO metrics (symbol_hash, check_id, key, value, run_id) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            metric.symbol.as_ref().map(SymbolHash::as_str),
            check.as_str(),
            metric.key,
            metric.value,
            run
        ],
    )?;
    Ok(())
}

/// A unit enum's serialized name, such as `doctor` or `warn`.
fn name<T: serde::Serialize>(value: T) -> Result<String> {
    match serde_json::to_value(value)? {
        serde_json::Value::String(name) => Ok(name),
        other => Err(StoreError::Value(other.to_string())),
    }
}

/// The BLAKE3 hex digest of `value`'s JSON.
fn hash_json<T: serde::Serialize>(value: &T) -> Result<String> {
    Ok(blake3::hash(serde_json::to_string(value)?.as_bytes())
        .to_hex()
        .to_string())
}

fn parse_run(run: &str) -> Result<RunId> {
    run.parse().map_err(|error| invalid(&error))
}

fn parse_symbol(hash: &str) -> Result<SymbolHash> {
    hash.parse().map_err(|error| invalid(&error))
}

fn invalid(error: &cupel_core::Error) -> StoreError {
    StoreError::Value(error.to_string())
}
