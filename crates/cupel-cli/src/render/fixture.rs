// ABOUTME: A hand-built envelope in which every field holds a distinctive value, for renderer tests.
// ABOUTME: Built from struct literals, so a new envelope field fails to compile here until it is filled.
use std::collections::BTreeMap;

use cupel_core::{
    Action, ActionKind, ArgDescription, BaselineRef, CheckId, CheckResult, CheckStatus,
    CommandDescription, ConfigEcho, ConfigSource, CostClass, CupelInfo, Delta, Description,
    Envelope, Evidence, Example, ExitCode, FactKind, Finding, GateOutcome, GateResult, GateRule,
    GitInfo, LangId, Location, NextAction, PlannedCheck, PlannedInstall, PlannedSkip, PlannedWrite,
    RunInfo, RunPlan, ScopeInfo, ScopeMode, Score, Severity, SeverityCounts, SkipReason, Skipped,
    Stability, Status, Summary, Symbol, Tier, ToolId, ToolchainInfo, ValueType, Verb,
};
use serde_json::{Map, Value, json};

fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => panic!("expected an object, got {other}"),
    }
}

fn tier(value: u8) -> Tier {
    Tier::try_from(value).expect("valid tier")
}

/// A finding with every field set; `n` varies the distinctive values.
pub fn finding(n: u32, check: &str, kind: &str, severity: Severity) -> Finding {
    let id = format!("fnd_{:06x}", 0x7c_2e1a + n);
    let symbol = format!("sh_{:016x}", 0x4b1d_9e0c_4b1d_9e00_u64 + u64::from(n));
    Finding {
        id: id.parse().expect("valid finding ID"),
        check: CheckId::from(check),
        kind: kind.to_owned(),
        severity,
        confidence: 0.95,
        tier: tier(3),
        symbol: Some(Symbol {
            display: format!("exonym::rewrite::Planner::push_limit_{n}"),
            hash: symbol.parse().expect("valid symbol hash"),
            lang: LangId::from("rust"),
        }),
        location: Some(Location {
            file: "src/rewrite/planner.rs".to_owned(),
            line: 142 + n,
            col: Some(21),
            end_line: 144 + n,
            end_col: Some(23),
        }),
        title: format!("Mutant survived: `<=` → `<` at planner.rs:{}", 142 + n),
        detail: Some(
            "No test distinguishes limit == offset from limit < offset.\nSecond detail line."
                .to_owned(),
        ),
        evidence: Evidence(object(json!({
            "mutation": {"from": "<=", "to": "<", "operator": "relational"},
            "tests_run": 118,
            "covering_tests": ["rewrite::limit_tests::pushes_through_sort"],
            "note": "a long evidence string that is far longer than forty characters in total",
            "flaky": false,
            "oracle": null,
        }))),
        action: Action {
            kind: ActionKind::WriteTest,
            priority: n + 1,
            summary: "Add a test where limit == offset asserting the boundary.".to_owned(),
            hints: object(json!({
                "nearest_test_file": "tests/rewrite/limit_tests.rs",
                "covering_tests": ["pushes_through_sort"],
            })),
        },
        repro: format!("cupel check --only {check} --symbols {symbol} --seed 42"),
        suppress: format!("cupel suppress {id} --reason '…'"),
        new_since_baseline: n.is_multiple_of(2),
        suppressed: n == 2,
    }
}

fn argument() -> ArgDescription {
    ArgDescription {
        name: "--against".to_owned(),
        positional: false,
        value: ValueType::Enum,
        value_name: Some("TARGET".to_owned()),
        values: vec!["baseline".to_owned(), "head".to_owned()],
        default: Some("baseline".to_owned()),
        env: Some("CUPEL_AGAINST".to_owned()),
        required: true,
        repeated: true,
        conflicts: vec!["--since-baseline".to_owned()],
        summary: "What to compare with.".to_owned(),
        stability: Stability::Stable,
        pending_step: Some("P2.2".to_owned()),
    }
}

fn description() -> Description {
    Description::Command(CommandDescription {
        name: "baseline".to_owned(),
        summary: "Manage named baselines.".to_owned(),
        stability: Stability::Experimental,
        pending_step: Some("P2.2".to_owned()),
        arguments: vec![argument()],
        subcommands: vec![CommandDescription {
            name: "baseline set".to_owned(),
            summary: "Record the current run as the baseline.".to_owned(),
            stability: Stability::Stable,
            pending_step: None,
            arguments: vec![ArgDescription {
                name: "FILE".to_owned(),
                positional: true,
                value: ValueType::Path,
                value_name: Some("FILE".to_owned()),
                values: Vec::new(),
                default: None,
                env: None,
                required: false,
                repeated: false,
                conflicts: Vec::new(),
                summary: "Where to write.".to_owned(),
                stability: Stability::Experimental,
                pending_step: None,
            }],
            subcommands: Vec::new(),
            examples: vec![Example {
                command: "cupel baseline set FILE".to_owned(),
                summary: "Set it.".to_owned(),
                exit_code: ExitCode::Pass,
            }],
        }],
        examples: vec![Example {
            command: "cupel baseline show".to_owned(),
            summary: "Show the baseline.".to_owned(),
            exit_code: ExitCode::Usage,
        }],
    })
}

/// The envelope of a failed `check` run with every field filled: three findings across two
/// severities and two checks (one new, one suppressed), a run check and a skipped check, a gate
/// with a passing and a failing rule, two next actions, and a plan with every kind of entry.
pub fn envelope() -> Envelope {
    let findings = vec![
        finding(0, "tests.mutation", "mutant.survived", Severity::Fail),
        finding(1, "static.crap", "crap.high", Severity::Warn),
        finding(2, "tests.mutation", "mutant.timeout", Severity::Fail),
    ];
    Envelope {
        cupel: CupelInfo {
            version: "9.8.7".to_owned(),
            schema: "https://cupel.dev/schema/v1/run.json".to_owned(),
        },
        run: run_info(),
        status: Status::Fail,
        exit_code: ExitCode::Fail,
        checks: checks(),
        findings,
        summary: summary(),
        next_actions: next_actions(),
        description: Some(description()),
        plan: Some(plan()),
        truncated: false,
        total_findings: 3,
    }
}

fn run_info() -> RunInfo {
    RunInfo {
        id: "run_01JA2B3C4D5E6F7G8H9J0KMNPQ"
            .parse()
            .expect("valid run ID"),
        verb: Some(Verb::Check),
        started: "2026-10-10T16:31:04Z".to_owned(),
        duration_ms: 48_211,
        git: Some(GitInfo {
            sha: "9f1c2d3".to_owned(),
            dirty: true,
            base: Some("a81e4f5".to_owned()),
            branch: Some("feat/limit-pushdown".to_owned()),
        }),
        scope: Some(ScopeInfo {
            mode: ScopeMode::Changed,
            files: 4,
            symbols: 11,
            roots: vec![".".to_owned(), "web".to_owned()],
            languages: vec![LangId::from("rust"), LangId::from("typescript")],
        }),
        depth: Some(CostClass::Deep),
        budget_ms: Some(20_000),
        seed: 42,
        toolchain: ToolchainInfo(BTreeMap::from([
            ("rust".to_owned(), "1.93.0".to_owned()),
            ("cargo-llvm-cov".to_owned(), "0.6.12".to_owned()),
        ])),
        config: ConfigEcho {
            effective: object(json!({
                "depth": "deep",
                "format": null,
                "budget": "20s",
                "profile": "ci",
                "checks": {"static.clones": {"threshold": 0.82}},
                "lang": {},
            })),
            sources: vec![
                ConfigSource::Flags,
                ConfigSource::Profile,
                ConfigSource::CupelToml,
            ],
        },
        baseline: Some(BaselineRef {
            id: "run_01J9ZZZZZZZZZZZZZZZZZZZZZZ"
                .parse()
                .expect("valid run ID"),
            name: "default".to_owned(),
        }),
    }
}

fn checks() -> Vec<CheckResult> {
    vec![
        CheckResult {
            id: CheckId::from("tests.mutation"),
            status: CheckStatus::Fail,
            tier: tier(3),
            cost: CostClass::Standard,
            score: Some(Score {
                value: 0.71,
                unit: "killed_ratio".to_owned(),
                n: 34,
            }),
            delta: Some(Delta {
                vs: "baseline".to_owned(),
                value: -0.12,
            }),
            findings: 2,
            duration_ms: 41_000,
            skipped: None,
            oracle: Some("pg".to_owned()),
        },
        CheckResult {
            id: CheckId::from("perf.bench"),
            status: CheckStatus::Skipped,
            tier: tier(5),
            cost: CostClass::Exhaustive,
            score: None,
            delta: None,
            findings: 0,
            duration_ms: 0,
            skipped: Some(Skipped {
                reason: SkipReason::Unavailable,
                fact: Some(FactKind::Symbols),
                lang: Some(LangId::from("go")),
                tool: Some(ToolId::from("cargo-criterion")),
                see: Some("cupel doctor".to_owned()),
            }),
            oracle: None,
        },
    ]
}

fn summary() -> Summary {
    Summary {
        findings: SeverityCounts {
            fail: 6,
            warn: 9,
            info: 22,
        },
        new: 4,
        resolved: 2,
        suppressed: 1,
        gate: Some(GateResult {
            result: GateOutcome::Fail,
            evaluated: "gate.changed".to_owned(),
            rules: vec![
                GateRule {
                    rule: "tests.mutation.score >= 0.80".to_owned(),
                    actual: Some(0.71),
                    result: GateOutcome::Fail,
                },
                GateRule {
                    rule: "tests.weakening none".to_owned(),
                    actual: None,
                    result: GateOutcome::Pass,
                },
            ],
        }),
    }
}

fn next_actions() -> Vec<NextAction> {
    vec![
        NextAction {
            priority: 1,
            finding: Some("fnd_7c2e1a".parse().expect("valid finding ID")),
            kind: ActionKind::WriteTest,
            summary: "Add a boundary test for push_limit.".to_owned(),
            command: None,
        },
        NextAction {
            priority: 2,
            finding: None,
            kind: ActionKind::Rerun,
            summary: "Run differential vs oracle 'pg'".to_owned(),
            command: Some("cupel check --changed --depth deep --only gen.differential".to_owned()),
        },
    ]
}

/// A plan with two checks to run (one with a note), a skip, an install, and two writes (one
/// with a diff).
fn plan() -> RunPlan {
    RunPlan {
        would_run: vec![
            PlannedCheck {
                id: CheckId::from("static.crap"),
                targets: 11,
                est_ms: 900,
                needs: vec!["coverage(cached)".to_owned()],
                note: None,
            },
            PlannedCheck {
                id: CheckId::from("tests.mutation"),
                targets: 34,
                est_ms: 140_000,
                needs: vec!["coverage(fresh)".to_owned(), "per-test-map".to_owned()],
                note: Some("19/34 sites cached as killed; 15 will execute".to_owned()),
            },
        ],
        would_skip: vec![PlannedSkip {
            id: CheckId::from("conc.dst"),
            skipped: Skipped {
                reason: SkipReason::Unconfigured,
                fact: None,
                lang: Some(LangId::from("rust")),
                tool: None,
                see: Some("cupel describe conc.dst".to_owned()),
            },
        }],
        would_install: vec![PlannedInstall {
            tool: ToolId::from("cargo-llvm-cov"),
            command: Some("cargo install cargo-llvm-cov".to_owned()),
        }],
        would_write: vec![
            PlannedWrite {
                path: ".cupel/runs/run_01JA2B3C4D5E6F7G8H9J0KMNPQ.json".to_owned(),
                diff: None,
            },
            PlannedWrite {
                path: "cupel.toml".to_owned(),
                diff: Some(
                    "--- a/cupel.toml\n+++ b/cupel.toml\n@@ -1 +1 @@\n-depth = \"quick\"\n+depth = \"deep\"\n"
                        .to_owned(),
                ),
            },
        ],
        est_total_ms: 140_900,
    }
}
