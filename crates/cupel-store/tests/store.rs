// ABOUTME: Tests for the fact store: opening through a `Writer`, migrations, recording and reading
// ABOUTME: runs, findings, checks, metrics, baselines, facts, symbols, and suppressions (ADR-045).
use std::fs;
use std::path::{Path, PathBuf};

use cupel_core::syntax::{Span, SymbolDef, SymbolKind, Visibility};
use cupel_core::{
    CheckId, Envelope, FactKind, FindingId, LangId, Metric, ProducerId, RunId, Suppression,
    SymbolHash, TestOutcome, TestResult, TestResults, Writer,
};
use cupel_store::{CacheKey, Store, StoreError, StoredFact, StoredTestResult};
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
        fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
            for entry in fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(&path, root, out);
                } else {
                    out.push(path.strip_prefix(root).unwrap().display().to_string());
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.root(), &self.root(), &mut out);
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
