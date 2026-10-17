// ABOUTME: gen.differential over hand-built DivergenceResults, with no engine (ADR-094): the manifest, one finding
// ABOUTME: per divergent entry with both answers, notes for skipped entries, the spec skip, scores, and metrics.
mod support;

use std::collections::BTreeMap;

use cupel_core::config::OracleKind;
use cupel_core::syntax::{
    NormNode, NormTree, NormalizedSymbol, Span, SymbolDef, SymbolKind, Symbols, Visibility,
};
use cupel_core::{
    ActionKind, Answer, CheckId, CheckStatus, ComparedOracle, CostClass, Divergence,
    DivergenceResults, EntryComparison, FactKind, Facts, FindingId, LangId, MetricPer, ProducerId,
    Scope, Severity, SkipReason,
};
use serde_json::{Value, json};
use support::{check, combined, hash_of_line, language, recorded, score, setting_error};

const ID: &str = "gen.differential";

fn quarter() -> SymbolDef {
    SymbolDef {
        name: "quarter".to_owned(),
        namespace: "quarter".to_owned(),
        display: "quarter.quarter".to_owned(),
        kind: SymbolKind::Function,
        visibility: Visibility::Public,
        lang: LangId::from("python"),
        file: "quarter.py".to_owned(),
        span: Span {
            line: 1,
            col: 1,
            end_line: 3,
            end_col: 22,
        },
        body: None,
        parameters: 1,
    }
}

fn command_oracle() -> ComparedOracle {
    ComparedOracle {
        name: "quarter".to_owned(),
        kind: OracleKind::Command,
        commit: None,
    }
}

fn compared(entry: &str, compared: u64, divergence: Option<Divergence>) -> EntryComparison {
    EntryComparison {
        entry: entry.to_owned(),
        symbol: Some(quarter().key()),
        compared,
        skipped: None,
        divergence,
    }
}

fn skipped(entry: &str, reason: &str) -> EntryComparison {
    EntryComparison {
        entry: entry.to_owned(),
        symbol: None,
        compared: 0,
        skipped: Some(reason.to_owned()),
        divergence: None,
    }
}

fn minus_one() -> Divergence {
    Divergence {
        input: vec![json!(-1)],
        found: vec![json!(-5)],
        case: 4,
        shrinks: 2,
        implementation: Answer::Returned("-1".to_owned()),
        oracle: Answer::Returned("0".to_owned()),
    }
}

fn results(oracle: Option<ComparedOracle>, entries: Vec<EntryComparison>) -> DivergenceResults {
    DivergenceResults {
        engine: ProducerId::from("engine.differential"),
        oracle,
        seed: 3,
        cases: 100,
        entries,
    }
}

fn run_over(fact: DivergenceResults) -> (cupel_core::CheckResult, support::Recorded) {
    let check = check(ID);
    let mut facts = Facts::for_manifest(check.manifest());
    facts.insert(fact).unwrap();
    facts
        .insert(Symbols {
            symbols: vec![quarter()],
        })
        .unwrap();
    facts
        .insert(NormTree {
            symbols: vec![NormalizedSymbol {
                symbol: quarter().key(),
                hash: hash_of_line(1),
                root: NormNode::leaf("fn"),
            }],
        })
        .unwrap();
    recorded(check, &facts, Scope::default(), json!({})).unwrap()
}

#[test]
fn the_manifest_needs_divergence_results_at_deep_cost_and_uses_the_oracle() {
    let check = check(ID);
    let manifest = check.manifest();
    assert_eq!(
        manifest.needs,
        [
            FactKind::DivergenceResults,
            FactKind::Symbols,
            FactKind::NormTree
        ]
    );
    assert_eq!(
        (u8::from(manifest.tier), manifest.cost, check.uses_oracle()),
        (4, CostClass::Deep, true)
    );
    let kinds: Vec<(&str, ActionKind)> = manifest
        .findings
        .iter()
        .map(|spec| (spec.kind.as_str(), spec.action))
        .collect();
    assert_eq!(kinds, [("diff.divergence", ActionKind::FixBug)]);
    let metrics: Vec<(&str, MetricPer)> = manifest
        .metrics
        .iter()
        .map(|metric| (metric.name.as_str(), metric.per))
        .collect();
    assert_eq!(
        metrics,
        [
            ("divergences", MetricPer::Scope),
            ("cases", MetricPer::Scope)
        ]
    );
}

#[test]
fn a_divergent_entry_is_one_finding_with_both_answers_on_the_minimized_input() {
    let (result, recorded) = run_over(results(
        Some(command_oracle()),
        vec![compared("quarter.py:quarter", 5, Some(minus_one()))],
    ));
    assert_eq!(recorded.findings.len(), 1);
    let finding = &recorded.findings[0];
    let hash = hash_of_line(1);
    assert_eq!(
        finding.id,
        FindingId::compute(&CheckId::from(ID), "diff.divergence", &hash, None)
    );
    assert_eq!(
        Value::Object(finding.evidence.0.clone()),
        json!({
            "entry": "quarter.py:quarter",
            "oracle": {"name": "quarter", "kind": "command", "commit": null},
            "input": [-1],
            "found": [-5],
            "case": 4,
            "shrinks": 2,
            "implementation": {"returned": "-1"},
            "reference": {"returned": "0"},
        })
    );
    assert_eq!(
        finding.title,
        "`quarter(-1)` returned `-1`, but oracle `quarter` returned `0`"
    );
    assert_eq!(
        finding.action.summary,
        "Decide which answer is right for `quarter(-1)`: fix the implementation, or, if the \
         change is intended, the oracle."
    );
    assert_eq!(
        finding.repro,
        format!(
            "cupel check --only gen.differential --symbols {hash} --oracle quarter --seed 3 --cases 100"
        )
    );
    assert_eq!(
        (finding.severity, finding.confidence, finding.action.kind),
        (Severity::Fail, 1.0, ActionKind::FixBug)
    );
    let location = finding.location.as_ref().unwrap();
    assert_eq!((location.file.as_str(), location.line), ("quarter.py", 1));
    assert_eq!(result.status, CheckStatus::Fail);
    assert_eq!(result.score, Some(score(1.0, "divergent", 1)));
    assert_eq!(
        result.metrics,
        BTreeMap::from([("divergences".to_owned(), 1.0), ("cases".to_owned(), 5.0)])
    );
}

#[test]
fn errors_and_crashes_are_described_by_how_the_call_ended() {
    let mut divergence = minus_one();
    divergence.input = vec![json!(2), json!("a b")];
    divergence.implementation = Answer::Raised("ValueError: no".to_owned());
    divergence.oracle = Answer::Crashed("exit 3".to_owned());
    let (_, recorded) = run_over(results(
        Some(command_oracle()),
        vec![compared("quarter.py:quarter", 5, Some(divergence))],
    ));
    assert_eq!(
        recorded.findings[0].title,
        "`quarter(2, \"a b\")` raised `ValueError: no`, but oracle `quarter` crashed (exit 3)"
    );
}

#[test]
fn entries_that_agree_pass_and_skipped_entries_are_notes() {
    let (result, recorded) = run_over(results(
        Some(command_oracle()),
        vec![
            compared("quarter.py:quarter", 102, None),
            skipped("quarter.py:half", "no function `half` in `quarter.py`"),
        ],
    ));
    assert!(recorded.findings.is_empty());
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(result.score, Some(score(0.0, "divergent", 1)));
    assert_eq!(result.metrics.get("cases").copied(), Some(102.0));
    assert_eq!(
        result.notes,
        ["quarter.py:half not compared: no function `half` in `quarter.py`"]
    );
}

#[test]
fn nothing_compared_passes_with_no_score_and_no_metrics() {
    let (result, _) = run_over(results(
        Some(command_oracle()),
        vec![skipped(
            "quarter.py:half",
            "no function `half` in `quarter.py`",
        )],
    ));
    let (none, _) = run_over(results(None, Vec::new()));
    assert_eq!(
        (result.status, result.score, result.metrics.len()),
        (CheckStatus::Pass, None, 0)
    );
    assert_eq!(
        (none.score, none.notes),
        (None, vec!["the run has no oracle".to_owned()])
    );
}

#[test]
fn a_spec_oracle_skips_the_check() {
    let spec = Some(ComparedOracle {
        name: "model".to_owned(),
        kind: OracleKind::Spec,
        commit: None,
    });
    let (result, recorded) = run_over(results(spec, Vec::new()));
    let skip = result.skipped.unwrap();
    assert_eq!(
        (result.status, skip.reason, skip.see.as_deref()),
        (
            CheckStatus::Skipped,
            SkipReason::Unconfigured,
            Some("cupel describe gen.differential")
        )
    );
    assert!(recorded.findings.is_empty());
}

#[test]
fn scores_and_metrics_add_up_across_languages() {
    let check = check(ID);
    let python = language(check, &[("divergences", 1.0), ("cases", 5.0)], 1);
    let go = language(check, &[("divergences", 0.0), ("cases", 100.0)], 2);
    assert_eq!(
        combined(check, "cases", &[python.clone(), go.clone()]),
        Some(105.0)
    );
    assert_eq!(combined(check, "divergences", &[python, go]), Some(1.0));
    assert_eq!(
        check.combine(&[score(1.0, "divergent", 1), score(0.0, "divergent", 2)]),
        Some(score(1.0, "divergent", 3))
    );
}

#[test]
fn the_check_takes_no_settings() {
    let (key, valid) = setting_error(check(ID), json!({"cases": 5}));
    assert_eq!((key.as_str(), valid.len()), ("cases", 0));
}
