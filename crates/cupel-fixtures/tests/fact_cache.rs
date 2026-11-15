// ABOUTME: The fact cache (ADR-181, ADR-188): a run that reads every file's parsed facts from the store prints what
// ABOUTME: a run that parses every file prints, for each language fixture, and parses nothing to do it.
use std::path::Path;

use cupel_core::{RunId, Writer};
use cupel_fixtures::{Outcome, canonical_json, copy_repo, redact, run_cupel_measuring};
use cupel_store::Store;

const QUICK: &[&str] = &[
    "check",
    "--depth",
    "quick",
    "--no-gate",
    "--format",
    "json",
    "--verbose",
];

/// The `--verbose` line counting the files read from the store and the trees parsed.
fn parsed_counts(outcome: &Outcome) -> (u64, u64) {
    let line = outcome
        .stderr
        .lines()
        .find_map(|line| line.strip_prefix("cupel: parsed facts: "))
        .unwrap_or_else(|| panic!("no parsed facts line in:\n{}", outcome.stderr));
    let numbers: Vec<u64> = line
        .split(' ')
        .filter_map(|word| word.parse().ok())
        .collect();
    (numbers[0], numbers[1])
}

/// A copy of the fixture `lang` after one quick check, which keeps every file's facts.
fn checked_once(lang: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_repo(lang, dir.path());
    let first = run_cupel_measuring(QUICK, dir.path());
    let (read, parsed) = parsed_counts(&first);
    assert_eq!(read, 0, "{lang}: a first run finds nothing in the store");
    assert!(parsed > 0, "{lang}: a first run parses");
    dir
}

/// Drops every parsed fact `dir`'s store keeps: recording another build's drops the rest.
fn forget_parsed_facts(dir: &Path) {
    let store = Store::open(&Writer::new(dir, false).unwrap()).unwrap();
    store
        .record_parsed_facts("another build", &RunId::generate(), &[], &[])
        .unwrap();
}

#[test]
fn a_run_reading_parsed_facts_from_the_store_prints_what_a_run_parsing_every_file_prints() {
    for lang in ["clojure", "go", "java", "javascript", "python", "rust"] {
        let (cached, cold) = (checked_once(lang), checked_once(lang));
        forget_parsed_facts(cold.path());

        let from_store = run_cupel_measuring(QUICK, cached.path());
        let parsing = run_cupel_measuring(QUICK, cold.path());
        let (read, parsed) = parsed_counts(&from_store);
        assert!(read > 0, "{lang}: the second run reads the store");
        assert_eq!(parsed, 0, "{lang}: and parses nothing");
        assert_eq!(
            parsed_counts(&parsing).0,
            0,
            "{lang}: its twin reads nothing"
        );
        assert_eq!(
            canonical_json(&redact(from_store.envelope(), cached.path())),
            canonical_json(&redact(parsing.envelope(), cold.path())),
            "{lang}: the run from the store differs from the run that parsed"
        );
    }
}

/// One quick check of `dir` with `src/pruned.py` holding `body`; the files it read from the store
/// and the trees it parsed.
fn checked_with(dir: &Path, body: &str) -> (u64, u64) {
    std::fs::write(dir.join("src/pruned.py"), body).unwrap();
    parsed_counts(&run_cupel_measuring(QUICK, dir))
}

#[test]
fn a_file_s_parsed_facts_are_kept_while_one_of_the_last_20_runs_used_them_and_pruned_after() {
    // ADR-201 through the binary: a run keeps what it parsed and records the files it used;
    // the sweep after each run drops the facts of a file version none of the last 20 runs used.
    let (old, new) = (
        "def pruned():\n    return 1\n",
        "def pruned():\n    return 2\n",
    );
    let dir = tempfile::tempdir().unwrap();
    copy_repo("python", dir.path());
    let (_, parsed) = checked_with(dir.path(), old);
    assert!(parsed > 0, "the first run parses every file");
    // Runs 2 to 20 use the new version, so `old` is still in one of the last 20 runs.
    for _ in 2..=20 {
        checked_with(dir.path(), new);
    }
    assert_eq!(
        checked_with(dir.path(), old).1,
        0,
        "the old version, used 20 runs ago, is read from the store"
    );
    // Run 21 used `old`; runs 22 to 41 use only `new`, so after them no recent run used `old`.
    for _ in 22..=41 {
        checked_with(dir.path(), new);
    }
    assert_eq!(
        checked_with(dir.path(), old).1,
        1,
        "the old version, last used 21 runs ago, was pruned and is parsed again"
    );
}
