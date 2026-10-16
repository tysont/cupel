// ABOUTME: Tests for the fact store: opening through a `Writer`, migrations, recording and reading
// ABOUTME: runs, findings, checks, metrics, baselines, facts, symbols, suppressions, and oracles (ADR-045).
use std::fs;
use std::path::PathBuf;

use cupel_core::budget::{TaskDuration, TaskShape};
use cupel_core::config::{OracleConfig, OracleKind};
use cupel_core::syntax::{MutationOperator, Span, SymbolDef, SymbolKey, SymbolKind, Visibility};
use cupel_core::{
    CheckId, CostClass, Envelope, FactKind, FindingId, LangId, Metric, Mutant, MutantOutcome,
    MutationOutcomes, ProducerId, RunId, Suppression, SymbolHash, TestOutcome, TestResult,
    TestResults, Writer,
};
use cupel_store::{
    CacheKey, ScoredAt, Store, StoreError, StoredFact, StoredTestResult, oracles::VerifiedOracle,
};
use serde_json::{Value, json};
use tempfile::TempDir;

struct Repo {
    dir: TempDir,
}

impl Repo {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    fn root(&self) -> PathBuf {
        self.dir.path().canonicalize().unwrap()
    }

    fn writer(&self) -> Writer {
        Writer::new(self.dir.path(), false).unwrap()
    }

    fn store(&self) -> Store {
        Store::open(&self.writer()).unwrap()
    }

    /// Every file under the root, relative and sorted.
    fn files(&self) -> Vec<String> {
        let root = self.root();
        let mut pending = vec![root.clone()];
        let mut out = Vec::new();
        while let Some(dir) = pending.pop() {
            for path in fs::read_dir(dir)
                .unwrap()
                .map(|entry| entry.unwrap().path())
            {
                if path.is_dir() {
                    pending.push(path);
                } else {
                    out.push(path.strip_prefix(&root).unwrap().display().to_string());
                }
            }
        }
        out.sort();
        out
    }

    /// Every file under the root with its bytes, sorted by path.
    fn contents(&self) -> Vec<(String, Vec<u8>)> {
        self.files()
            .into_iter()
            .map(|file| {
                let bytes = fs::read(self.root().join(&file)).unwrap();
                (file, bytes)
            })
            .collect()
    }
}

/// A direct connection to the repository's database, for inspecting what the store wrote.
fn raw(repo: &Repo) -> rusqlite::Connection {
    rusqlite::Connection::open(repo.root().join(".cupel/cupel.db")).unwrap()
}

/// A metric `key` of `value`, about `symbol` or the whole check.
fn metric(symbol: Option<SymbolHash>, key: &str, value: f64) -> Metric {
    Metric {
        symbol,
        key: key.to_owned(),
        value,
    }
}

/// The symbol hash ending in `n`.
fn symbol_hash(n: u8) -> SymbolHash {
    format!("sh_{n:016x}").parse().unwrap()
}

fn run_id(n: u8) -> RunId {
    format!("run_01JA2B3C4D5E6F7G8H9JKMNP{n:02}")
        .parse()
        .unwrap()
}

fn finding(id: &str, check: &str, title: &str) -> Value {
    json!({
        "id": id, "check": check, "kind": "complexity.high",
        "severity": "warn", "confidence": 1.0, "tier": 1,
        "symbol": {"display": "a::b", "hash": "sh_00000000000000aa", "lang": "rust"},
        "location": {"file": "src/lib.rs", "line": 3, "col": null, "end_line": 5, "end_col": null},
        "title": title, "detail": null,
        "evidence": {"cyclomatic": 12}, "action": {"kind": "reduce_complexity", "priority": 1,
        "summary": "Split it.", "hints": {}},
        "repro": "cupel check", "suppress": format!("cupel suppress {id} --reason '…'"),
        "new_since_baseline": false, "suppressed": false
    })
}

fn check(id: &str, score: Option<f64>) -> Value {
    json!({
        "id": id, "status": "pass", "tier": 1, "cost": "quick",
        "score": score.map(|value| json!({"value": value, "unit": "ratio", "n": 3})),
        "delta": null, "findings": 1, "duration_ms": 5, "skipped": null, "oracle": null
    })
}

/// Records run `n` of `verb`, exiting with 0, with `findings` and `checks`.
fn record(store: &Store, n: u8, verb: &str, findings: &[Value], checks: &[Value]) {
    store
        .record_run(&envelope(&run_id(n), verb, 0, findings, checks))
        .unwrap();
}

fn envelope(
    run: &RunId,
    verb: &str,
    exit_code: u8,
    findings: &[Value],
    checks: &[Value],
) -> Envelope {
    serde_json::from_value(json!({
        "cupel": {"version": "0.1.0", "schema": "https://cupel.dev/schema/v1/run.json"},
        "run": {
            "id": run.to_string(), "verb": verb, "started": "2026-10-13T10:00:00Z", "duration_ms": 1,
            "git": {"sha": format!("sha-{run}"), "dirty": false, "base": null, "branch": null},
            "scope": null, "depth": "standard", "budget_ms": null,
            "seed": u64::MAX, "toolchain": {}, "config": {"effective": {}, "sources": []},
            "baseline": null
        },
        "status": if exit_code == 0 { "pass" } else { "error" }, "exit_code": exit_code,
        "checks": checks, "total_findings": findings.len(), "findings": findings,
        "summary": {"findings": {"fail": 0, "warn": findings.len(), "info": 0}, "new": 0,
                    "resolved": 0, "suppressed": 0, "gate": null},
        "next_actions": [], "truncated": false
    }))
    .unwrap()
}

#[test]
fn opening_creates_the_database_and_lock_under_cupel_only() {
    let repo = Repo::new();
    fs::write(repo.root().join("main.rs"), "fn main() {}\n").unwrap();
    let store = repo.store();
    record(&store, 1, "doctor", &[], &[]);
    drop(store);
    let outside: Vec<String> = repo
        .files()
        .into_iter()
        .filter(|path| !path.starts_with(".cupel/"))
        .collect();
    assert_eq!(outside, ["main.rs"]);
    let inside: Vec<String> = repo
        .files()
        .into_iter()
        .filter(|path| path.starts_with(".cupel/"))
        .collect();
    assert_eq!(
        inside,
        [".cupel/.gitignore", ".cupel/cupel.db", ".cupel/cupel.lock"]
    );
}

#[test]
fn the_database_is_in_wal_mode_while_open() {
    let repo = Repo::new();
    let store = repo.store();
    let mode: String = raw(&repo)
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    record(&store, 1, "doctor", &[], &[]);
    assert!(repo.root().join(".cupel/cupel.db-wal").exists());
}

#[test]
fn migrations_run_from_empty_and_reopening_changes_nothing() {
    let repo = Repo::new();
    let store = repo.store();
    let version = store.schema_version().unwrap();
    assert_eq!(version, Store::SCHEMA_VERSION);
    record(&store, 1, "doctor", &[], &[]);
    drop(store);
    let again = repo.store();
    assert_eq!(again.schema_version().unwrap(), version);
    assert!(again.run(&run_id(1)).unwrap().is_some());
    let connection = raw(&repo);
    let mut statement = connection
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .unwrap();
    let tables: Vec<String> = statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        tables,
        [
            "baselines",
            "checks",
            "coverage",
            "durations",
            "facts",
            "findings",
            "metrics",
            "mutants",
            "oracles",
            "runs",
            "schema_version",
            "suppressions",
            "symbols",
            "test_results",
        ]
    );
}

#[test]
fn a_store_newer_than_the_binary_is_refused() {
    let repo = Repo::new();
    drop(repo.store());
    raw(&repo)
        .execute(
            "UPDATE schema_version SET version = ?1",
            [Store::SCHEMA_VERSION + 1],
        )
        .unwrap();
    let error = Store::open(&repo.writer()).err().map(|err| err.to_string());
    let (found, known) = (Store::SCHEMA_VERSION + 1, Store::SCHEMA_VERSION);
    assert_eq!(
        error,
        Some(format!(
            "the fact store has schema version {found}, newer than this cupel's {known}"
        ))
    );
}

#[test]
fn a_recording_writer_cannot_open_and_open_existing_creates_nothing() {
    let repo = Repo::new();
    assert!(matches!(
        Store::open(&repo.writer().recording()),
        Err(StoreError::DryRun)
    ));
    assert!(
        Store::open_existing(&repo.writer().recording())
            .unwrap()
            .is_none()
    );
    assert!(repo.files().is_empty());
}

#[test]
fn open_existing_reads_but_never_writes() {
    let repo = Repo::new();
    repo.store()
        .record_run(&envelope(&run_id(1), "doctor", 0, &[], &[]))
        .unwrap();
    let before = repo.contents();
    let store = Store::open_existing(&repo.writer().recording())
        .unwrap()
        .unwrap();
    let read = store
        .run(&run_id(1))
        .unwrap()
        .map(|envelope| envelope.run.id);
    assert_eq!(read, Some(run_id(1)));
    drop(store);
    let after = repo.contents();
    assert_eq!(after, before, "reading changed the store's files");
    let store = Store::open_existing(&repo.writer().recording())
        .unwrap()
        .unwrap();
    let error = store
        .record_run(&envelope(&run_id(2), "doctor", 0, &[], &[]))
        .err()
        .map(|err| err.to_string());
    let lock = repo.root().join(".cupel/cupel.lock");
    assert_eq!(
        error,
        Some(format!(
            "cannot use `{}`: the fact store was opened read-only",
            lock.display()
        ))
    );
}

#[test]
fn a_store_under_a_path_with_uri_characters_opens_read_only() {
    let repo = Repo::new();
    let odd = repo.root().join("my repo #1 ?100%");
    fs::create_dir(&odd).unwrap();
    let writer = Writer::new(&odd, false).unwrap();
    Store::open(&writer)
        .unwrap()
        .record_run(&envelope(&run_id(1), "doctor", 0, &[], &[]))
        .unwrap();
    let store = Store::open_existing(&writer.recording()).unwrap().unwrap();
    let read = store
        .run(&run_id(1))
        .unwrap()
        .map(|envelope| envelope.run.id);
    assert_eq!(read, Some(run_id(1)));
}

#[test]
fn a_writer_that_may_not_write_cupel_cannot_open_the_store() {
    let repo = Repo::new();
    let scratch = repo.writer().scratch(&run_id(1));
    let error = Store::open(&scratch).err().map(|err| err.to_string());
    let cupel = repo.root().join(".cupel");
    assert_eq!(
        error,
        Some(format!(
            "`{}` is outside .cupel/; pass --write to permit writing it",
            cupel.display()
        ))
    );
}

#[test]
fn a_recorded_run_round_trips() {
    let repo = Repo::new();
    let store = repo.store();
    let run = envelope(
        &run_id(1),
        "check",
        0,
        &[finding("fnd_00000a", "static.complexity", "first")],
        &[check("static.complexity", Some(0.5))],
    );
    store.record_run(&run).unwrap();
    assert_eq!(store.run(&run_id(1)).unwrap(), Some(run));
    assert_eq!(store.run(&run_id(9)).unwrap(), None);
}

#[test]
fn recording_a_run_twice_replaces_it() {
    let repo = Repo::new();
    let store = repo.store();
    let first = envelope(
        &run_id(1),
        "check",
        0,
        &[finding("fnd_00000a", "static.complexity", "first")],
        &[],
    );
    store.record_run(&first).unwrap();
    let second = envelope(&run_id(1), "check", 0, &[], &[]);
    store.record_run(&second).unwrap();
    assert_eq!(store.run(&run_id(1)).unwrap(), Some(second));
    assert_eq!(store.finding(&"fnd_00000a".parse().unwrap()).unwrap(), None);
}

#[test]
fn a_finding_is_read_from_its_latest_run() {
    let repo = Repo::new();
    let store = repo.store();
    let id: FindingId = "fnd_00000a".parse().unwrap();
    for (n, title) in [(1, "older"), (3, "newest"), (2, "middle")] {
        store
            .record_run(&envelope(
                &run_id(n),
                "check",
                0,
                &[finding(id.as_str(), "static.complexity", title)],
                &[],
            ))
            .unwrap();
    }
    let (found, run) = store.finding(&id).unwrap().unwrap();
    assert_eq!(found.title, "newest");
    assert_eq!(run, run_id(3));
    assert_eq!(store.finding(&"fnd_ffffff".parse().unwrap()).unwrap(), None);
}

#[test]
fn bookkeeping_runs_are_stored_but_their_findings_and_checks_are_not_indexed() {
    let repo = Repo::new();
    let store = repo.store();
    record(
        &store,
        1,
        "check",
        &[finding("fnd_00000a", "static.complexity", "produced")],
        &[check("static.complexity", Some(0.5))],
    );
    for (n, verb) in [(2, "explain"), (3, "baseline"), (4, "suppress")] {
        store
            .record_run(&envelope(
                &run_id(n),
                verb,
                0,
                &[finding("fnd_00000a", "static.complexity", "repeated")],
                &[check("static.complexity", Some(0.9))],
            ))
            .unwrap();
        assert!(store.run(&run_id(n)).unwrap().is_some(), "{verb}");
    }
    let (found, run) = store
        .finding(&"fnd_00000a".parse().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!((found.title.as_str(), run), ("produced", run_id(1)));
    let last = store
        .last_run_for_check(&CheckId::from("static.complexity"))
        .unwrap()
        .unwrap();
    assert_eq!(last.run, run_id(1));
}

#[test]
fn a_run_with_a_repeated_finding_keeps_the_first() {
    let repo = Repo::new();
    let store = repo.store();
    record(
        &store,
        1,
        "check",
        &[
            finding("fnd_00000a", "static.complexity", "first"),
            finding("fnd_00000a", "static.complexity", "second"),
        ],
        &[],
    );
    let (found, _) = store
        .finding(&"fnd_00000a".parse().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(found.title, "first");
}

#[test]
fn the_last_run_of_a_check_has_its_result_findings_and_metrics() {
    let repo = Repo::new();
    let store = repo.store();
    let complexity = CheckId::from("static.complexity");
    record(
        &store,
        1,
        "check",
        &[finding("fnd_00000a", "static.complexity", "old")],
        &[check("static.complexity", Some(0.25))],
    );
    record(
        &store,
        2,
        "check",
        &[
            finding("fnd_00000b", "static.complexity", "new"),
            finding("fnd_00000c", "static.clones", "other check"),
        ],
        &[
            check("static.complexity", Some(0.75)),
            check("static.clones", None),
        ],
    );
    let cyclomatic = metric(Some(symbol_hash(0xaa)), "cyclomatic", 12.0);
    store
        .record_metrics(&run_id(2), &complexity, std::slice::from_ref(&cyclomatic))
        .unwrap();
    record(&store, 3, "doctor", &[], &[]);

    let last = store.last_run_for_check(&complexity).unwrap().unwrap();
    assert_eq!((&last.run, &last.result.id), (&run_id(2), &complexity));
    let titles: Vec<&str> = last.findings.iter().map(|f| f.title.as_str()).collect();
    assert_eq!(titles, ["new"]);
    assert_eq!(last.metrics, [metric(None, "score", 0.75), cyclomatic]);
    let deps = store
        .last_run_for_check(&CheckId::from("static.deps"))
        .unwrap();
    assert_eq!(deps.map(|last| last.run), None);
}

#[test]
fn the_latest_result_run_skips_bookkeeping_verbs_and_failed_invocations() {
    let repo = Repo::new();
    let store = repo.store();
    assert_eq!(store.last_result_run().unwrap(), None);
    for (n, verb, exit_code) in [(1, "doctor", 3), (2, "check", 2)] {
        store
            .record_run(&envelope(&run_id(n), verb, exit_code, &[], &[]))
            .unwrap();
    }
    record(&store, 3, "explain", &[], &[]);
    record(&store, 4, "baseline", &[], &[]);
    assert_eq!(store.last_result_run().unwrap(), Some(run_id(1)));
    record(&store, 5, "init", &[], &[]);
    assert_eq!(store.last_result_run().unwrap(), Some(run_id(5)));
    assert_eq!(
        store
            .last_result_run_at(&format!("sha-{}", run_id(1)))
            .unwrap(),
        Some(run_id(1))
    );
    assert_eq!(
        store
            .last_result_run_at(&format!("sha-{}", run_id(3)))
            .unwrap(),
        None
    );
}

#[test]
fn baselines_are_named_pointers_to_runs() {
    let repo = Repo::new();
    let store = repo.store();
    assert_eq!(store.baseline("default").unwrap(), None);
    store.set_baseline("default", &run_id(1)).unwrap();
    store.set_baseline("release", &run_id(2)).unwrap();
    store.set_baseline("default", &run_id(3)).unwrap();
    assert_eq!(store.baseline("default").unwrap(), Some(run_id(3)));
    assert_eq!(store.baseline("release").unwrap(), Some(run_id(2)));
}

#[test]
fn facts_are_stored_and_read_by_cache_key() {
    let repo = Repo::new();
    let store = repo.store();
    let key = CacheKey {
        kind: FactKind::Symbols,
        version: 1,
        producer: ProducerId::from("lang.rust"),
        inputs: vec!["b".to_owned(), "a".to_owned()],
        toolchain_hash: "t".to_owned(),
        config_hash: "c".to_owned(),
    };
    let fact = StoredFact {
        key: key.digest(),
        kind: FactKind::Symbols,
        version: 1,
        producer: ProducerId::from("lang.rust"),
        lang: Some(LangId::from("rust")),
        run: Some(run_id(1)),
        payload: br#"{"symbols":[]}"#.to_vec(),
    };
    assert_eq!(store.fact(&key.digest()).unwrap(), None);
    store.put_fact(&fact).unwrap();
    assert_eq!(store.fact(&key.digest()).unwrap(), Some(fact));
}

#[test]
fn a_cache_key_ignores_input_order_and_changes_with_every_part() {
    let key = CacheKey {
        kind: FactKind::Symbols,
        version: 1,
        producer: ProducerId::from("lang.rust"),
        inputs: vec!["a".to_owned(), "b".to_owned()],
        toolchain_hash: "t".to_owned(),
        config_hash: "c".to_owned(),
    };
    let reordered = CacheKey {
        inputs: vec!["b".to_owned(), "a".to_owned()],
        ..key.clone()
    };
    assert_eq!(key.digest(), reordered.digest());
    assert_eq!(key.digest().len(), 64);
    let variants = [
        CacheKey {
            kind: FactKind::Decisions,
            ..key.clone()
        },
        CacheKey {
            version: 2,
            ..key.clone()
        },
        CacheKey {
            producer: ProducerId::from("lang.go"),
            ..key.clone()
        },
        CacheKey {
            inputs: vec!["a".to_owned()],
            ..key.clone()
        },
        CacheKey {
            toolchain_hash: "u".to_owned(),
            ..key.clone()
        },
        CacheKey {
            config_hash: "d".to_owned(),
            ..key.clone()
        },
    ];
    for variant in variants {
        assert_ne!(variant.digest(), key.digest(), "{variant:?}");
    }
}

fn symbol(name: &str) -> SymbolDef {
    SymbolDef {
        name: name.to_owned(),
        namespace: "m".to_owned(),
        display: format!("m::{name}"),
        kind: SymbolKind::Function,
        visibility: Visibility::Public,
        lang: LangId::from("rust"),
        file: "src/lib.rs".to_owned(),
        span: Span {
            line: 1,
            col: 1,
            end_line: 3,
            end_col: 2,
        },
        body: None,
        parameters: 0,
    }
}

#[test]
fn symbols_new_since_a_run_are_the_hashes_first_seen_after_it() {
    let repo = Repo::new();
    let store = repo.store();
    let [old, new, unseen] = [1, 2, 3].map(symbol_hash);
    store
        .record_symbols(&run_id(1), &[(old.clone(), symbol("f"))])
        .unwrap();
    store
        .record_symbols(
            &run_id(3),
            &[(old.clone(), symbol("f")), (new.clone(), symbol("g"))],
        )
        .unwrap();
    let changed = store
        .symbols_new_since(&run_id(2), &[old, new.clone(), unseen.clone()])
        .unwrap();
    assert_eq!(changed, [new, unseen]);
}

#[test]
fn suppressions_mirror_the_file() {
    let repo = Repo::new();
    let store = repo.store();
    let entry = |id: &str| Suppression {
        finding: id.parse().unwrap(),
        reason: "known".to_owned(),
        until: Some("2027-01-01".parse().unwrap()),
        created: "2026-10-13".to_owned(),
    };
    store
        .replace_suppressions(&[entry("fnd_00000a"), entry("fnd_00000b")])
        .unwrap();
    store.replace_suppressions(&[entry("fnd_00000b")]).unwrap();
    assert_eq!(store.suppressions().unwrap(), [entry("fnd_00000b")]);
}

/// The verification of a command oracle `name` with identity `hash`, recorded by run `run`.
fn verified(name: &str, hash: &str, run: u8) -> VerifiedOracle {
    VerifiedOracle {
        name: name.to_owned(),
        definition: OracleConfig {
            kind: OracleKind::Command,
            reference: None,
            cmd: Some(format!("{name} {{input}}")),
            spec: None,
            lang: None,
            inputs: vec!["1".to_owned(), "2".to_owned()],
        },
        verified_at: "2026-10-16T09:30:00Z".to_owned(),
        hash: hash.to_owned(),
        run: run_id(run),
    }
}

#[test]
fn oracle_verifications_round_trip_are_replaced_and_are_forgotten() {
    let repo = Repo::new();
    let store = repo.store();
    store.record_oracle(&verified("calc", "h1", 1)).unwrap();
    assert_eq!(store.verified_oracle("bc").unwrap(), None);
    store.record_oracle(&verified("bc", "h2", 2)).unwrap();
    store.record_oracle(&verified("calc", "h3", 3)).unwrap();
    assert_eq!(
        store.verified_oracle("calc").unwrap(),
        Some(verified("calc", "h3", 3))
    );
    assert_eq!(
        store.verified_oracles().unwrap(),
        [verified("bc", "h2", 2), verified("calc", "h3", 3)]
    );
    store.forget_oracle("calc").unwrap();
    store.forget_oracle("never-verified").unwrap();
    assert_eq!(store.verified_oracles().unwrap(), [verified("bc", "h2", 2)]);
}

#[test]
fn the_oracles_table_holds_only_verifications_so_every_column_is_required() {
    let repo = Repo::new();
    drop(repo.store());
    let connection = raw(&repo);
    let mut statement = connection
        .prepare("SELECT name, \"notnull\" FROM pragma_table_info('oracles') ORDER BY cid")
        .unwrap();
    let columns: Vec<(String, bool)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let required = |name: &str| (name.to_owned(), true);
    assert_eq!(
        columns,
        [
            ("name".to_owned(), false),
            required("kind"),
            required("spec"),
            required("verified_at"),
            required("verified_hash"),
            required("run_id"),
        ]
    );
    assert_eq!(Store::SCHEMA_VERSION, 5);
}

/// Records runs 1 to `runs`, each from a thread of its own with a store of its own.
fn record_concurrently(repo: &Repo, runs: u8) {
    let handles: Vec<_> = (1..=runs)
        .map(|n| {
            let writer = repo.writer();
            std::thread::spawn(move || {
                record(&Store::open(&writer).unwrap(), n, "doctor", &[], &[]);
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn concurrent_writers_take_turns() {
    let repo = Repo::new();
    repo.store();
    record_concurrently(&repo, 8);
    let store = repo.store();
    let stored: Vec<RunId> = (1..=8)
        .filter_map(|n| store.run(&run_id(n)).unwrap())
        .map(|envelope| envelope.run.id)
        .collect();
    assert_eq!(stored, (1..=8).map(run_id).collect::<Vec<_>>());
}

fn ran(id: &str, file: Option<&str>, outcome: TestOutcome, duration_ms: Option<u64>) -> TestResult {
    TestResult {
        id: id.to_owned(),
        file: file.map(str::to_owned),
        name: id.to_owned(),
        outcome,
        duration_ms,
        output: None,
    }
}

#[test]
fn test_results_are_indexed_per_run_and_language() {
    let repo = Repo::new();
    let store = repo.store();
    let results = TestResults::new(
        ProducerId::from("tool.pytest"),
        Vec::new(),
        vec![
            ran("t::b", Some("t.py"), TestOutcome::Failed, Some(12)),
            ran("t::a", None, TestOutcome::Skipped, None),
        ],
        &repo.root(),
    );
    store
        .record_test_results(&run_id(1), &LangId::from("python"), &results)
        .unwrap();
    let row = |test: &str, file: Option<&str>, outcome: &str, duration_ms: Option<u64>| {
        StoredTestResult {
            lang: LangId::from("python"),
            runner: ProducerId::from("tool.pytest"),
            test: test.to_owned(),
            file: file.map(str::to_owned),
            outcome: outcome.to_owned(),
            duration_ms,
        }
    };
    assert_eq!(
        (
            store.test_results(&run_id(1)).unwrap(),
            store.test_results(&run_id(2)).unwrap()
        ),
        (
            vec![
                row("t::a", None, "skipped", None),
                row("t::b", Some("t.py"), "failed", Some(12)),
            ],
            vec![]
        )
    );
}

/// A mutant of `site` with replacement `to`, `outcome`, stored under `key`.
fn mutant(site: &str, to: &str, outcome: MutantOutcome, key: Option<&str>) -> Mutant {
    Mutant {
        symbol: SymbolKey {
            file: "calc.py".to_owned(),
            line: 1,
            col: 1,
        },
        symbol_hash: Some("sh_00000000000000aa".parse().unwrap()),
        site_hash: site.parse().unwrap(),
        operator: MutationOperator::Relational,
        from: "<".to_owned(),
        to: to.to_owned(),
        span: Span {
            line: 2,
            col: 5,
            end_line: 2,
            end_col: 6,
        },
        outcome,
        tests: vec!["t::a".to_owned()],
        killing_tests: Vec::new(),
        duration_ms: Some(40),
        reused: false,
        equivalent: None,
        cache_key: key.map(str::to_owned),
        strategy: None,
        fallback: None,
    }
}

/// The language every mutant test records under.
fn python() -> LangId {
    LangId::from("python")
}

fn outcomes(mutants: Vec<Mutant>) -> MutationOutcomes {
    MutationOutcomes::new(
        ProducerId::from("engine.mutation"),
        Some(ProducerId::from("tool.pytest")),
        CostClass::Standard,
        false,
        mutants,
    )
}

#[test]
fn mutants_are_indexed_and_found_again_by_their_cache_key() {
    let repo = Repo::new();
    let store = repo.store();
    let first = mutant(
        "00000000000000b1",
        "<=",
        MutantOutcome::Survived,
        Some("k1"),
    );
    store
        .record_mutants(&run_id(1), &python(), &outcomes(vec![first]))
        .unwrap();
    let killed = mutant("00000000000000b1", "<=", MutantOutcome::Killed, Some("k1"));
    store
        .record_mutants(&run_id(2), &python(), &outcomes(vec![killed.clone()]))
        .unwrap();
    let found = store
        .mutants_by_key(&["k1".to_owned(), "k2".to_owned()])
        .unwrap();
    assert_eq!(
        found.into_iter().collect::<Vec<_>>(),
        vec![("k1".to_owned(), killed)]
    );
}

#[test]
fn the_last_outcome_of_each_site_and_replacement_is_kept() {
    let repo = Repo::new();
    let store = repo.store();
    store
        .record_mutants(
            &run_id(1),
            &python(),
            &outcomes(vec![
                mutant("00000000000000b1", "<=", MutantOutcome::Killed, None),
                mutant("00000000000000b2", ">", MutantOutcome::Survived, None),
            ]),
        )
        .unwrap();
    store
        .record_mutants(
            &run_id(2),
            &python(),
            &outcomes(vec![mutant(
                "00000000000000b1",
                "<=",
                MutantOutcome::Survived,
                None,
            )]),
        )
        .unwrap();
    let last: Vec<(String, String, String, MutantOutcome)> = store
        .last_mutant_outcomes()
        .unwrap()
        .into_iter()
        .map(|(site, outcome)| (site.symbol_hash, site.site_hash, site.replacement, outcome))
        .collect();
    let row = |site: &str, to: &str, outcome| {
        (
            "sh_00000000000000aa".to_owned(),
            site.to_owned(),
            to.to_owned(),
            outcome,
        )
    };
    assert_eq!(
        last,
        [
            row("00000000000000b1", "<=", MutantOutcome::Survived),
            row("00000000000000b2", ">", MutantOutcome::Survived),
        ]
    );
}

/// Whether each of six threads opening the store of a new repository at once succeeded.
fn opened_at_once() -> Vec<bool> {
    let repo = Repo::new();
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..6)
            .map(|_| scope.spawn(|| Store::open(&repo.writer()).is_ok()))
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect()
    })
}

#[test]
fn stores_opened_at_once_on_a_new_repository_all_open() {
    // Concurrent first runs (separate processes, or the daemon's concurrent requests, ADR-085)
    // each create or open the store while the others switch it to WAL.
    let rounds: Vec<Vec<bool>> = (0..20).map(|_| opened_at_once()).collect();
    assert_eq!(rounds, vec![vec![true; 6]; 20]);
}

/// A check result of `id` with `status`, as `checks[]` holds it.
fn result_with(id: &str, status: &str) -> Value {
    let skipped = if status == "skipped" {
        json!({"reason": "budget", "fact": null, "lang": "rust", "tool": null, "see": null,
               "est_ms": 900})
    } else {
        Value::Null
    };
    json!({
        "id": id, "status": status, "tier": 1, "cost": "quick", "score": null,
        "delta": null, "findings": 0, "duration_ms": 5, "skipped": skipped, "oracle": null
    })
}

/// Records `runs` runs, run `n` timing each shape at `n` times its unit in milliseconds.
fn record_runs(store: &Store, shapes: &[(&TaskShape, u64)], runs: u8) {
    for n in 1..=runs {
        let durations: Vec<TaskDuration> = shapes
            .iter()
            .map(|(shape, unit)| TaskDuration {
                shape: (*shape).clone(),
                duration_ms: u64::from(n) * unit,
            })
            .collect();
        store.record_durations(&run_id(n), &durations).unwrap();
    }
}

#[test]
fn durations_come_back_newest_first_per_shape_and_at_most_ten() {
    let repo = Repo::new();
    let store = repo.store();
    let rust = LangId::from("rust");
    let pytest = TaskShape::producer(
        &ProducerId::from("tool.pytest"),
        &LangId::from("python"),
        false,
    );
    let clones = TaskShape::check(&CheckId::from("static.clones"), &rust, 40);
    record_runs(&store, &[(&pytest, 100), (&clones, 1)], 12);
    let recent = store.recent_durations().unwrap();
    let of = |shape: &TaskShape| -> Vec<u64> {
        recent
            .iter()
            .find(|(recorded, _)| recorded == shape)
            .map(|(_, durations)| durations.clone())
            .unwrap()
    };
    assert_eq!(
        of(&pytest),
        (3..=12).rev().map(|n| n * 100).collect::<Vec<u64>>()
    );
    assert_eq!(of(&clones), (3..=12).rev().collect::<Vec<u64>>());
    assert_eq!(recent.len(), 2);
}

#[test]
fn the_latest_scored_result_of_each_check_names_its_commit_and_whether_it_was_dirty() {
    let repo = Repo::new();
    let store = repo.store();
    let ran = [
        result_with("static.clones", "pass"),
        result_with("tests.mutation", "pass"),
    ];
    record(&store, 1, "check", &[], &ran);
    // A later run that skipped tests.mutation for its budget does not count as scoring it.
    let later = [
        result_with("static.clones", "fail"),
        result_with("tests.mutation", "skipped"),
    ];
    record(&store, 2, "check", &[], &later);
    let scored = store.latest_scored().unwrap();
    assert_eq!(
        scored[&CheckId::from("static.clones")],
        ScoredAt {
            sha: Some(format!("sha-{}", run_id(2))),
            dirty: false,
        }
    );
    assert_eq!(
        scored[&CheckId::from("tests.mutation")].sha,
        Some(format!("sha-{}", run_id(1)))
    );
    assert!(!scored.contains_key(&CheckId::from("tests.coverage")));
}
