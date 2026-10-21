// ABOUTME: Integration tests for `cupel explain`: a finding or a check's last result read from the fact
// ABOUTME: store, with a source excerpt, full evidence, test and remediation hints; unknown IDs exit 2; never re-runs.
mod support;

use serde_json::{Value, json};
use support::{
    Repo, complexity_check, complexity_finding, cupel, envelope, run_id, usage_argument,
};

const SOURCE: &str =
    "// one\n// two\nfn tangled() {\n    if a { b() }\n}\n// six\n// seven\n// eight\n// nine\n";

#[test]
fn an_unknown_finding_id_is_a_usage_error_about_the_target() {
    let repo = Repo::new();
    let outcome = repo.run(&["explain", "fnd_abcdef"]);
    assert_eq!(usage_argument(&outcome), json!("TARGET"));
    assert_eq!(outcome.finding()["evidence"]["value"], json!("fnd_abcdef"));
}

#[test]
fn an_unknown_check_id_and_a_malformed_finding_id_are_usage_errors() {
    let repo = Repo::new();
    for target in ["static.complexity", "fnd_not-hex"] {
        let outcome = repo.run(&["explain", target]);
        assert_eq!(usage_argument(&outcome), json!("TARGET"), "{target}");
    }
}

#[test]
fn a_stored_finding_is_explained_with_excerpt_evidence_and_test_hints() {
    let repo = Repo::new();
    repo.write("src/lib.rs", SOURCE);
    let finding = complexity_finding("fnd_00000a", 3, 5);
    repo.seed(&[envelope(
        &run_id(1),
        "check",
        std::slice::from_ref(&finding),
        &[],
    )]);

    let outcome = repo.run(&["explain", "fnd_00000a"]);

    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("pass"));
    assert_eq!(envelope["findings"], json!([finding]));
    let explanation = &envelope["explanation"];
    assert_eq!(explanation["subject"], json!("finding"));
    assert_eq!(explanation["target"], json!("fnd_00000a"));
    assert_eq!(explanation["run"], json!(run_id(1)));
    assert_eq!(explanation["covering_tests"], json!(["tangled_works"]));
    assert_eq!(explanation["nearest_test_file"], json!("tests/lib.rs"));
    assert_eq!(explanation["metrics"], json!([]));
    let excerpt = &explanation["excerpt"];
    assert_eq!(excerpt["file"], json!("src/lib.rs"));
    let lines: Vec<(u64, bool)> = excerpt["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| {
            (
                line["line"].as_u64().unwrap(),
                line["focus"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        lines,
        [
            (1, false),
            (2, false),
            (3, true),
            (4, true),
            (5, true),
            (6, false),
            (7, false),
            (8, false)
        ]
    );
    assert_eq!(excerpt["lines"][3]["text"], json!("    if a { b() }"));
}

#[test]
fn the_latest_run_that_produced_a_finding_is_the_one_explained() {
    let repo = Repo::new();
    let mut older = complexity_finding("fnd_00000a", 3, 5);
    older["title"] = json!("older");
    let mut newer = complexity_finding("fnd_00000a", 3, 5);
    newer["title"] = json!("newer");
    repo.seed(&[
        envelope(&run_id(2), "check", &[newer], &[]),
        envelope(&run_id(1), "check", &[older], &[]),
    ]);
    let envelope = repo.run(&["explain", "fnd_00000a"]).envelope();
    assert_eq!(envelope["findings"][0]["title"], json!("newer"));
    assert_eq!(envelope["explanation"]["run"], json!(run_id(2)));
}

#[test]
fn explaining_twice_keeps_pointing_at_the_run_that_produced_the_finding() {
    let repo = Repo::new();
    repo.seed(&[envelope(
        &run_id(1),
        "check",
        &[complexity_finding("fnd_00000a", 3, 5)],
        &[],
    )]);
    repo.run(&["explain", "fnd_00000a"]);
    let envelope = repo.run(&["explain", "fnd_00000a"]).envelope();
    assert_eq!(envelope["explanation"]["run"], json!(run_id(1)));
}

#[test]
fn a_finding_whose_file_is_gone_or_outside_the_root_has_no_excerpt() {
    let repo = Repo::new();
    let mut escaping = complexity_finding("fnd_00000b", 1, 1);
    escaping["location"]["file"] = json!("../outside.rs");
    repo.seed(&[envelope(
        &run_id(1),
        "check",
        &[complexity_finding("fnd_00000a", 3, 5), escaping],
        &[],
    )]);
    for id in ["fnd_00000a", "fnd_00000b"] {
        let envelope = repo.run(&["explain", id]).envelope();
        assert_eq!(envelope["explanation"]["excerpt"], Value::Null, "{id}");
    }
}

#[test]
fn a_check_is_explained_by_its_last_runs_result_findings_and_metrics() {
    let repo = Repo::new();
    repo.seed(&[
        envelope(
            &run_id(1),
            "check",
            &[complexity_finding("fnd_00000a", 3, 5)],
            &[complexity_check(0.25)],
        ),
        envelope(
            &run_id(2),
            "check",
            &[complexity_finding("fnd_00000b", 3, 5)],
            &[complexity_check(0.5)],
        ),
        envelope(&run_id(3), "doctor", &[], &[]),
    ]);

    let outcome = repo.run(&["explain", "static.complexity"]);

    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["checks"], json!([complexity_check(0.5)]));
    let ids: Vec<&str> = envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["fnd_00000b"]);
    let explanation = &envelope["explanation"];
    assert_eq!(explanation["subject"], json!("check"));
    assert_eq!(explanation["run"], json!(run_id(2)));
    assert_eq!(
        explanation["metrics"],
        json!([{"symbol": null, "key": "score", "value": 0.5}])
    );
}

#[test]
fn a_real_doctor_finding_is_explained_with_its_evidence() {
    let repo = Repo::new();
    repo.write(
        "cupel.toml",
        "[lang.rust]\ntest = \"cupel-test-no-such-runner --all\"\n",
    );
    let doctor = repo.run(&["doctor"]).envelope();
    let finding = doctor["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["evidence"]["tool"] == json!("cupel-test-no-such-runner"))
        .unwrap()
        .clone();
    let id = finding["id"].as_str().unwrap();

    let outcome = repo.run(&["explain", id]);

    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["findings"], json!([finding]));
    assert_eq!(envelope["explanation"]["run"], doctor["run"]["id"]);
    assert_eq!(envelope["explanation"]["excerpt"], Value::Null);

    let text = repo.run(&["explain", id, "--format", "text"]).stdout;
    assert!(text.contains("explain   finding"), "{text}");
    assert!(
        text.contains("\"needed_for\": \"[lang.rust] test\""),
        "{text}"
    );
}

#[test]
fn explain_never_reruns_and_writes_nothing_outside_cupel() {
    let repo = Repo::new();
    repo.write("src/lib.rs", SOURCE);
    repo.seed(&[envelope(
        &run_id(1),
        "check",
        &[complexity_finding("fnd_00000a", 3, 5)],
        &[],
    )]);
    let before = repo.tree_outside_cupel();
    let outcome = repo.run_with(cupel().env("PATH", ""), &["explain", "fnd_00000a"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    assert_eq!(repo.tree_outside_cupel(), before);
}

#[test]
fn a_dry_run_explains_nothing_and_writes_nothing() {
    let repo = Repo::new();
    let outcome = repo.run(&["explain", "fnd_00000a", "--dry-run"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    assert_eq!(outcome.envelope()["status"], json!("plan"));
    assert!(repo.tree().is_empty(), "{:?}", repo.tree());
}

/// `finding` with the remediation hint `key` set to `hint`.
fn hinted(mut finding: Value, kind: &str, key: &str, hint: &Value) -> Value {
    finding["kind"] = json!(kind);
    finding["action"]["hints"] = json!({ key: hint });
    finding
}

fn place(file: &str, line: u32, end_line: u32) -> Value {
    json!({"file": file, "line": line, "col": 1, "end_line": end_line, "end_col": 2})
}

/// Explains `finding`, stored by a check run, as JSON and as text.
fn explained(finding: &Value) -> (Value, String) {
    let repo = Repo::new();
    repo.write("src/lib.rs", SOURCE);
    repo.seed(&[envelope(
        &run_id(1),
        "check",
        std::slice::from_ref(finding),
        &[],
    )]);
    let id = finding["id"].as_str().unwrap();
    let json = repo.run(&["explain", id]).envelope();
    let text = repo.run(&["explain", id, "--format", "text"]).stdout;
    (json, text)
}

#[test]
fn a_clone_cluster_is_explained_with_the_subtree_to_extract_and_its_parameters() {
    let hint = json!({
        "nodes": 34, "share": 0.412, "location": place("src/lib.rs", 3, 5),
        "others": [
            {"symbol": "lib::other", "location": place("src/other.rs", 10, 12)},
            {"symbol": "lib::third", "location": place("src/third.rs", 20, 22)}
        ],
        "parameters": [
            {"kind": "identifier", "values": ["rate", "fee", "tax"], "occurrences": 3},
            {"kind": "literal", "values": ["1", "2", "3"], "occurrences": 1}
        ]
    });
    let finding = hinted(
        complexity_finding("fnd_0000c1", 3, 5),
        "clone.cluster",
        "common_subtree",
        &hint,
    );
    let (json, text) = explained(&finding);
    assert_eq!(
        json["findings"][0]["action"]["hints"]["common_subtree"],
        hint
    );
    for line in [
        "  common_subtree: 34 nodes at src/lib.rs:3:1-5:2, shared by 2 others, 2 parameters",
        "  remedy    extract the 34-node subtree the cluster shares (0.412 of this symbol)",
        "    here          src/lib.rs:3:1-5:2",
        "    lib::other    src/other.rs:10:1-12:2",
        "    lib::third    src/third.rs:20:1-22:2",
        "    parameter     identifier rate | fee | tax (3 places)",
        "    parameter     literal 1 | 2 | 3",
    ] {
        assert!(text.contains(line), "{line:?} in\n{text}");
    }
}

#[test]
fn a_complex_function_is_explained_with_its_deepest_block() {
    let hint = json!({
        "location": place("src/lib.rs", 4, 4), "nesting": 2,
        "path": ["branch", "loop", "branch"], "cyclomatic": 2, "cognitive": 5
    });
    let finding = hinted(
        complexity_finding("fnd_0000c2", 3, 5),
        "complexity.high",
        "deepest_block",
        &hint,
    );
    let (json, text) = explained(&finding);
    assert_eq!(
        json["findings"][0]["action"]["hints"]["deepest_block"],
        hint
    );
    for line in [
        "  deepest_block: branch > loop > branch at src/lib.rs:4:1-4:2, nesting 2",
        "  remedy    extract the deepest block, branch > loop > branch at nesting 2, which carries \
         cyclomatic 2 and cognitive 5",
        "    here          src/lib.rs:4:1-4:2",
    ] {
        assert!(text.contains(line), "{line:?} in\n{text}");
    }
}

#[test]
fn an_import_cycle_is_explained_with_the_cheapest_import_to_cut() {
    let hint = json!({
        "from": "src/a.rs", "to": "src/b.rs", "line": 3, "specifiers": ["super::b::X", "super::b"],
        "names": ["X"], "unnamed": 1, "breaks_component": true
    });
    let finding = hinted(
        complexity_finding("fnd_0000c3", 3, 5),
        "dep.cycle",
        "cheapest_cut",
        &hint,
    );
    let (json, text) = explained(&finding);
    assert_eq!(json["findings"][0]["action"]["hints"]["cheapest_cut"], hint);
    for line in [
        "  cheapest_cut: src/a.rs -> src/b.rs at line 3, 1 name and 1 whole-module import",
        "  remedy    cut the imports of src/b.rs from src/a.rs (line 3), which breaks every cycle \
         among these modules",
        "    imports       super::b::X, super::b",
        "    names         X",
        "    whole-module  1",
    ] {
        assert!(text.contains(line), "{line:?} in\n{text}");
    }
}
