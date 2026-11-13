// ABOUTME: A `<= base` ratchet on `static.complexity.cyclomatic_max` end to end (ADR-199, ADR-205): one `if` added to
// ABOUTME: a cyclomatic-3 function passes by the tolerance, eight passes within watch, and crossing watch fails.
use std::path::Path;
use std::process::Command;

use cupel_fixtures::{Outcome, copy_repo, run_cupel};
use serde_json::{Value, json};

/// Runs git in `dir` as a fixed author.
fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The changed-scope `static.complexity` run, in `format`, of a branch whose commit replaces
/// the fixture `base`'s `letter` with the fixture `head`'s.
fn changed(dir: &Path, (base, head): (&str, &str), format: &str) -> Outcome {
    git(dir, &["init", "--quiet", "--initial-branch", "main"]);
    copy_repo(&format!("ratchet-watch/{base}"), dir);
    git(dir, &["add", "--all"]);
    git(dir, &["commit", "--quiet", "-m", "grade"]);
    git(dir, &["checkout", "--quiet", "-b", "work"]);
    copy_repo(&format!("ratchet-watch/{head}"), dir);
    git(dir, &["commit", "--quiet", "--all", "-m", head]);
    run_cupel(
        &[
            "check",
            "--changed",
            "--only",
            "static.complexity",
            "--format",
            format,
        ],
        dir,
    )
}

/// The exit code and the ratchet's gate rule, without what its values were measured on.
fn ratchet(base: &str, head: &str) -> (Option<i32>, Value) {
    let dir = tempfile::tempdir().unwrap();
    let outcome = changed(dir.path(), (base, head), "json");
    let gate = serde_json::to_value(&outcome.envelope().summary.gate).unwrap();
    let mut rule = gate["rules"][0].clone();
    if let Some(fields) = rule.as_object_mut() {
        fields.remove("actual_target");
        fields.remove("bound_target");
    }
    (outcome.code, rule)
}

/// The ratchet's rule with `actual`, `bound` (the base plus the tolerance 2), and `result`.
fn rule(actual: f64, bound: f64, result: &str) -> Value {
    json!({"rule": "static.complexity.cyclomatic_max <= base", "actual": actual, "bound": bound,
           "tolerance": 2.0, "result": result, "reason": null})
}

#[test]
fn one_if_added_to_a_cyclomatic_3_function_passes_and_a_change_past_watch_fails() {
    let watched = |mut rule: Value| {
        rule["watch"] = json!(10.0);
        rule
    };
    assert_eq!(
        [
            ratchet("base", "one-branch"),
            ratchet("base", "within-watch"),
            ratchet("base", "past-watch"),
            ratchet("near-watch", "past-watch"),
        ],
        [
            (Some(0), rule(4.0, 5.0, "pass")),
            (Some(0), watched(rule(8.0, 5.0, "pass"))),
            (Some(1), rule(11.0, 5.0, "fail")),
            // 9 to 11 is within the tolerance, but crosses the watch (ADR-205).
            (Some(1), watched(rule(11.0, 11.0, "fail"))),
        ]
    );
}

#[test]
fn the_text_gate_names_the_tolerance_and_the_watch_that_passed_the_rule() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = changed(dir.path(), ("base", "within-watch"), "text");
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    let line = "pass  static.complexity.cyclomatic_max <= base  actual 8 (grade.letter at \
                src/grade.py:5)  bound 5 (grade.letter at src/grade.py:5)  tolerance 2  \
                within watch 10";
    assert!(
        outcome.stdout.contains(line),
        "{line:?} in\n{}",
        outcome.stdout
    );
}
