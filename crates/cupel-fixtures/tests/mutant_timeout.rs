// ABOUTME: A mutant's run is capped and slow tests are left out of whole-suite runs, end to end (ADR-147, 236): the real binary
// ABOUTME: cuts two looping mutants at `max_mutant_timeout`, on time with a `ps` that never answers, and leaves a 61 s Go
// ABOUTME: test out of every mutant with a finding.
use cupel_core::{Envelope, Severity};
use cupel_fixtures::{copy_repo, run_cupel, run_cupel_measuring, run_cupel_with_env};

/// Whether `program` is on `PATH`; prints the skip line when it is not.
fn installed(program: &str, what: &str) -> bool {
    let found = cupel_core::find_program(program).is_some();
    if !found {
        eprintln!("SKIPPED: {program} not installed; {what} was not run");
    }
    found
}

/// Each finding of `envelope` as `kind title`.
fn findings(envelope: &Envelope) -> Vec<String> {
    envelope
        .findings
        .iter()
        .map(|finding| format!("{} {}", finding.kind, finding.title))
        .collect()
}

#[test]
fn a_mutant_that_loops_is_cut_at_max_mutant_timeout() {
    if !installed("pytest", "the mutant-timeout fixture") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    copy_repo("mutant-timeout/loop", dir.path());
    let outcome = run_cupel(
        &["check", "--only", "tests.mutation", "--format", "json"],
        dir.path(),
    );
    let envelope = outcome.envelope();
    assert_eq!(
        findings(envelope),
        [
            "mutant.survived Mutant survived: `0` → `1` at count.py:2",
            "mutant.timeout Mutant timed out: `+=` → `-=` at count.py:4",
            "mutant.timeout Mutant timed out: `1` → `0` at count.py:4",
        ],
        "{}",
        outcome.stdout
    );
    // Uncapped, each looping mutant would run a hundred thousand times its reference (hours);
    // the cap ends each at thirty seconds, so the check ends well within this guard.
    let ran_ms = envelope.checks[0].duration_ms;
    assert!(ran_ms < 600_000, "{ran_ms} ms");
    outcome.assert_golden("mutant-timeout/loop");
}

/// A `ps` that answers no listing in time, first on a `PATH` that is otherwise the caller's: it
/// sleeps past the 5 s listing timeout, and only a little past it, as a listing under way when
/// `cupel` exits is left to end on its own.
#[cfg(unix)]
fn path_with_a_hung_ps(bin: &std::path::Path) -> std::ffi::OsString {
    use std::os::unix::fs::PermissionsExt;
    let ps = bin.join("ps");
    std::fs::write(&ps, "#!/bin/sh\nexec sleep 8\n").unwrap();
    std::fs::set_permissions(&ps, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut path = std::ffi::OsString::from(bin);
    path.push(":");
    path.push(std::env::var_os("PATH").unwrap_or_default());
    path
}

#[cfg(unix)]
#[test]
fn with_a_ps_that_never_answers_every_mutant_timeout_still_fires_on_time() {
    if !installed("pytest", "the hung-ps mutant-timeout fixture") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    copy_repo("mutant-timeout/loop", dir.path());
    let toml = dir.path().join("cupel.toml");
    let config = std::fs::read_to_string(&toml).unwrap();
    std::fs::write(&toml, config.replace("\"30s\"", "\"10s\"")).unwrap();
    let bin = tempfile::tempdir().unwrap();
    let path = path_with_a_hung_ps(bin.path());
    let outcome = run_cupel_with_env(
        &[
            "check",
            "--only",
            "tests.mutation",
            "--format",
            "json",
            "--events",
            ".cupel/events.jsonl",
        ],
        dir.path(),
        &[("PATH", &path)],
    );
    let envelope = outcome.envelope();
    assert_eq!(
        findings(envelope),
        [
            "mutant.survived Mutant survived: `0` → `1` at count.py:2",
            "mutant.timeout Mutant timed out: `+=` → `-=` at count.py:4",
            "mutant.timeout Mutant timed out: `1` → `0` at count.py:4",
        ],
        "{}",
        outcome.stdout
    );
    // Every listing outlasts its 5 s timeout, so each is killed then and skipped; no wait loop waits
    // on one, so the looping mutants, which run side by side, end at their 10 s cap. A reader
    // that waited on a listing would end each up to 5 s late, and a kill that listed its groups
    // up to 10 s more.
    let ran_ms = envelope.checks[0].duration_ms;
    assert!(ran_ms < 20_000, "{ran_ms} ms");
    let events = std::fs::read_to_string(dir.path().join(".cupel/events.jsonl")).unwrap();
    let skipped = events
        .lines()
        .filter(|line| line.contains(r#"{"ev":"sampler.timeout","timeout_ms":5000}"#))
        .count();
    assert!(skipped >= 1, "{events}");
}

#[test]
fn a_slow_test_no_per_test_selection_narrows_is_left_out_of_every_mutant_with_a_finding() {
    if !installed("go", "the slow-test fixture") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    copy_repo("mutant-timeout/slow", dir.path());
    // The finding holds the slow test's measured duration, which a second run would not repeat.
    let outcome = run_cupel_measuring(
        &["check", "--only", "tests.mutation", "--format", "json"],
        dir.path(),
    );
    let envelope = outcome.envelope();
    let excluded: Vec<_> = envelope
        .findings
        .iter()
        .filter(|finding| finding.kind == "mutant.slow-test-excluded")
        .collect();
    assert_eq!(excluded.len(), 1, "{}", outcome.stdout);
    let finding = excluded[0];
    let evidence = &finding.evidence.0;
    // `go test -json` names no test's file.
    assert_eq!(
        (
            finding.severity,
            evidence["test"].as_str(),
            evidence["file"].as_str()
        ),
        (
            Severity::Info,
            Some("example.com/slow/TestSlowIntegration"),
            None
        )
    );
    let baseline_ms = evidence["baseline_ms"].as_u64().unwrap();
    assert!(baseline_ms > 60_000, "{baseline_ms}");
    // `Half` has no test but the slow one, so its mutants survive every run that leaves it out:
    // each ran all tests but the slow one, the fast test alone, with no per-test map to name it.
    let survivors: Vec<(&str, &serde_json::Value, &serde_json::Value)> = envelope
        .findings
        .iter()
        .filter(|finding| finding.kind == "mutant.survived")
        .map(|finding| {
            let symbol = finding.symbol.as_ref().map_or("", |s| s.display.as_str());
            let evidence = &finding.evidence.0;
            (symbol, &evidence["all_tests"], &evidence["tests_run"])
        })
        .collect();
    assert!(!survivors.is_empty(), "{}", outcome.stdout);
    for survivor in survivors {
        assert_eq!(
            survivor,
            ("slow.Half", &serde_json::json!(true), &serde_json::json!(1))
        );
    }
}
