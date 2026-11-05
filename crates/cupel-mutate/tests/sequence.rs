// ABOUTME: Running a narrowed mutant's tests one at a time (ADR-160): the order by baseline duration, each test's cap, what
// ABOUTME: one test's run says about the mutant (failed, hung, slow, unviable, passed), and the slowest tests a timeout names.
use std::collections::BTreeMap;
use std::time::Duration;

use cupel_mutate::sequence::{Caps, Limit, Run, Test, Verdict, order, slowest, verdict};

fn secs(seconds: f64) -> Duration {
    Duration::from_secs_f64(seconds)
}

fn test(id: &str, baseline: Option<f64>) -> Test {
    Test {
        id: id.to_owned(),
        baseline: baseline.map(secs),
    }
}

/// The default factor (10), a start-up of half a second, and the default overall cap (120 s).
fn caps() -> Caps {
    Caps {
        factor: 10,
        startup: secs(0.5),
        overall: secs(120.0),
    }
}

#[test]
fn tests_run_fastest_first_with_unknown_durations_last_and_ties_by_id() {
    let baseline = BTreeMap::from([
        ("slow".to_owned(), secs(30.0)),
        ("fast".to_owned(), secs(0.01)),
        ("tie-b".to_owned(), secs(1.0)),
        ("tie-a".to_owned(), secs(1.0)),
    ]);
    let ids = ["unknown-z", "slow", "unknown-a", "tie-b", "fast", "tie-a"].map(str::to_owned);
    let ordered: Vec<String> = order(&ids, &baseline).into_iter().map(|t| t.id).collect();
    assert_eq!(
        ordered,
        ["fast", "tie-a", "tie-b", "slow", "unknown-a", "unknown-z"]
    );
}

/// A test's own cap is the factor times its baseline, at least 2 s, plus the start-up; the overall cap left cuts it.
#[test]
fn a_tests_timeout_is_its_own_cap_unless_the_overall_cap_left_is_less() {
    let caps = caps();
    let cases = [
        (test("fast", Some(0.01)), secs(0.0), secs(0.0)),
        (test("medium", Some(1.0)), secs(0.0), secs(0.0)),
        (test("medium", Some(1.0)), secs(115.0), secs(0.0)),
        (test("unknown", None), secs(20.0), secs(0.0)),
        (test("rebuilt", Some(1.0)), secs(0.0), secs(30.0)),
    ];
    let limits: Vec<Limit> = cases
        .iter()
        .map(|(test, spent, build)| caps.limit(test, *spent, *build))
        .collect();
    assert_eq!(
        limits,
        [
            Limit {
                timeout: secs(2.5),
                own: true
            },
            Limit {
                timeout: secs(10.5),
                own: true
            },
            Limit {
                timeout: secs(5.0),
                own: false
            },
            Limit {
                timeout: secs(100.0),
                own: false
            },
            Limit {
                timeout: secs(40.5),
                own: true
            },
        ]
    );
}

fn run(failed: bool, complete: bool, reached: bool) -> Run {
    Run {
        failed,
        complete,
        reached,
    }
}

#[test]
fn a_failure_kills_and_a_completed_pass_moves_on() {
    let (caps, fast) = (caps(), test("fast", Some(0.01)));
    let own = caps.limit(&fast, secs(0.0), secs(0.0));
    assert_eq!(
        [
            verdict(&run(true, true, false), own, &fast, &caps),
            verdict(&run(false, true, false), own, &fast, &caps),
            verdict(&run(false, false, false), own, &fast, &caps),
        ],
        [Verdict::Failed, Verdict::Passed, Verdict::Unviable]
    );
}

/// A test that normally takes under a quarter of its own cap and reaches it hangs, as does a file copy's first run,
/// which also had the reference run's time to rebuild; one cut by the overall cap, one with no known duration, and
/// one near its own cap are slow.
#[test]
fn reaching_a_cap_is_a_hang_only_for_a_test_well_inside_its_own_cap() {
    let caps = caps();
    let reached = run(false, false, true);
    let fast = test("fast", Some(0.01));
    let unknown = test("unknown", None);
    let near = Caps { factor: 3, ..caps };
    let medium = test("medium", Some(1.0));
    let cases = [
        (&fast, caps, secs(0.0), secs(0.0)),
        (&fast, caps, secs(0.0), secs(9.0)),
        (&fast, caps, secs(119.0), secs(0.0)),
        (&unknown, caps, secs(0.0), secs(0.0)),
        (&medium, near, secs(0.0), secs(0.0)),
    ];
    let verdicts: Vec<Verdict> = cases
        .iter()
        .map(|(test, caps, spent, build)| {
            verdict(&reached, caps.limit(test, *spent, *build), test, caps)
        })
        .collect();
    assert_eq!(
        verdicts,
        [
            Verdict::Hung,
            Verdict::Hung,
            Verdict::Slow,
            Verdict::Slow,
            Verdict::Slow
        ]
    );
    // A test that failed before the cap killed whatever else its run reports.
    let own = caps.limit(&fast, secs(0.0), secs(0.0));
    assert_eq!(
        verdict(&run(true, false, true), own, &fast, &caps),
        Verdict::Failed
    );
}

/// A timeout names up to three of the tests that did not complete, slowest first, those of unknown duration after.
#[test]
fn a_timeout_names_the_slowest_tests_that_did_not_complete() {
    let left = [
        test("b", Some(2.0)),
        test("unknown", None),
        test("c", Some(40.0)),
        test("a", Some(3.0)),
    ];
    let named: Vec<(String, Option<u64>)> = slowest(&left)
        .into_iter()
        .map(|slow| (slow.id, slow.baseline_ms))
        .collect();
    assert_eq!(
        named,
        [
            ("c".to_owned(), Some(40_000)),
            ("a".to_owned(), Some(3_000)),
            ("b".to_owned(), Some(2_000)),
        ]
    );
    assert_eq!(
        slowest(&[test("unknown", None)])
            .into_iter()
            .map(|slow| (slow.id, slow.baseline_ms))
            .collect::<Vec<_>>(),
        [("unknown".to_owned(), None)]
    );
}
