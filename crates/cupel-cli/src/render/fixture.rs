// ABOUTME: A hand-built envelope in which every field holds a distinctive value, for renderer tests.
// ABOUTME: Built from struct literals, so a new envelope field fails to compile here until it is filled.
use std::collections::BTreeMap;

use cupel_core::config::{OracleConfig, OracleKind};
use cupel_core::{
    Action, ActionKind, ArgDescription, BaselineRef, CheckId, CheckResult, CheckStatus,
    CommandDescription, ConfigEcho, ConfigSource, CostClass, CupelInfo, Delta, Description,
    Envelope, Evidence, Example, Excerpt, ExcerptLine, ExitCode, ExplainSubject, Explanation,
    FactKind, Finding, GateOutcome, GateResult, GateRule, GitInfo, ImpactBasis, ImpactedTest,
    LangId, Location, Metric, MetricPer, NextAction, PlannedCheck, PlannedInstall, PlannedSkip,
    PlannedWrite, ProducerId, Protection, RankedTarget, RankingDimension, Rankings, Recommendation,
    RecommendedTarget, RuleOutcome, RunInfo, RunPlan, ScopeInfo, ScopeMode, Score, SetDelta,
    SetFindings, SetMetric, Severity, SeverityCounts, SkipReason, Skipped, Stability, Status,
    Summary, Symbol, Target, TargetValue, TestCommand, TestsReport, Tier, ToolId, ToolchainInfo,
    ValueType, Verb, Worst,
};
use cupel_core::{OracleReport, OracleState};
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

/// A symbol of a matched set (ADR-131), named `exonym::rewrite::Planner::<name>`.
fn matched_symbol(name: &str, hash: u64) -> Symbol {
    Symbol {
        display: format!("exonym::rewrite::Planner::{name}"),
        hash: format!("sh_{hash:016x}")
            .parse()
            .expect("valid symbol hash"),
        lang: LangId::from("rust"),
    }
}

/// A split matched across the baseline (ADR-131): one finding carried and the survivors compared.
fn split_set() -> SetDelta {
    SetDelta {
        base: vec![matched_symbol("push_limit", 0xa1)],
        head: vec![
            matched_symbol("push_limit_inner", 0xb2),
            matched_symbol("push_limit_outer", 0xc3),
        ],
        similarity: 0.871,
        findings: SetFindings {
            base: vec!["fnd_0aa0aa".parse().expect("valid finding ID")],
            head: vec!["fnd_7c2e1b".parse().expect("valid finding ID")],
        },
        metrics: BTreeMap::from([(
            "survivors".to_owned(),
            SetMetric {
                base: 3.0,
                head: 4.0,
            },
        )]),
    }
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
/// with a passing, a failing, and a skipped rule, two next actions, rankings with an empty and a
/// filled dimension, and a plan with every kind of entry.
pub fn envelope() -> Envelope {
    let findings = vec![
        finding(0, "tests.mutation", "mutant.survived", Severity::Fail),
        finding(1, "static.crap", "crap.high", Severity::Warn),
        finding(2, "tests.mutation", "mutant.timeout", Severity::Fail),
    ];
    Envelope {
        cupel: CupelInfo {
            version: "9.8.7".to_owned(),
            schema: "https://tysont.github.io/cupel/schema/v1/run.json".to_owned(),
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
        explanation: Some(explanation()),
        rankings: Some(rankings()),
        tests: Some(tests()),
        oracles: Some(oracles()),
        truncated: false,
        total_findings: 3,
    }
}

/// A verified ref oracle and a command oracle whose definition changed since it was verified
/// (ADR-091 §6).
fn oracles() -> Vec<OracleReport> {
    let definition = |kind, reference: Option<&str>, cmd: Option<&str>| OracleConfig {
        kind,
        reference: reference.map(str::to_owned),
        cmd: cmd.map(str::to_owned),
        spec: None,
        lang: None,
        inputs: vec!["2 + 2".to_owned()],
    };
    vec![
        OracleReport {
            name: "bc".to_owned(),
            definition: definition(OracleKind::Command, None, Some("bc -l <<< {input}")),
            state: OracleState::Changed,
            hash: Some("6b1d".to_owned()),
            verified_at: Some("2026-10-09T08:00:00Z".to_owned()),
            verified_run: Some(
                "run_01JA2B3C4D5E6F7G8H9JKMNPQR"
                    .parse()
                    .expect("valid run ID"),
            ),
        },
        OracleReport {
            name: "pg".to_owned(),
            definition: definition(OracleKind::Ref, Some("v1.2.0"), None),
            state: OracleState::Verified,
            hash: Some("9e3a".to_owned()),
            verified_at: Some("2026-10-10T16:00:00Z".to_owned()),
            verified_run: Some(
                "run_01JA2B3C4D5E6F7G8H9JKMNPQS"
                    .parse()
                    .expect("valid run ID"),
            ),
        },
    ]
}

/// One language chose by per-test coverage and one fell back to file-level impact (ADR-077).
fn tests() -> TestsReport {
    TestsReport {
        per_test_map: false,
        impacted: vec![
            ImpactedTest {
                id: "calc tests::adds".to_owned(),
                file: Some("src/lib.rs".to_owned()),
                reason: "covers calc::add (src/lib.rs:5)".to_owned(),
                lang: LangId::from("rust"),
            },
            ImpactedTest {
                id: "web/add.test.ts".to_owned(),
                file: Some("web/add.test.ts".to_owned()),
                reason: "imports web/add.ts".to_owned(),
                lang: LangId::from("typescript"),
            },
        ],
        commands: vec![
            TestCommand {
                lang: LangId::from("rust"),
                runner: ProducerId::from("tool.cargo-nextest"),
                dir: ".".to_owned(),
                command:
                    "cargo nextest run --workspace -E '(binary_id(=calc) & test(=tests::adds))'"
                        .to_owned(),
                narrowed: true,
            },
            TestCommand {
                lang: LangId::from("typescript"),
                runner: ProducerId::from("tool.vitest"),
                dir: "web".to_owned(),
                command: "npx vitest run add.test.ts".to_owned(),
                narrowed: true,
            },
        ],
        languages: vec![
            ImpactBasis {
                lang: LangId::from("rust"),
                per_test_map: true,
                producer: Some(ProducerId::from("tool.llvm-cov-per-test")),
                measured: Some(2),
                reused: Some(1236),
                fallback: None,
            },
            ImpactBasis {
                lang: LangId::from("typescript"),
                per_test_map: false,
                producer: None,
                measured: None,
                reused: None,
                fallback: Some(
                    "no producer makes per-test-map for this language (see `cupel describe matrix`)"
                        .to_owned(),
                ),
            },
        ],
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
            metrics: BTreeMap::from([("score".to_owned(), 0.71), ("survivors".to_owned(), 10.0)]),
            delta: Some(Delta {
                vs: "baseline".to_owned(),
                value: -0.12,
                sets: vec![split_set()],
            }),
            findings: 2,
            duration_ms: 41_000,
            skipped: None,
            oracle: Some("pg".to_owned()),
            notes: Vec::new(),
        },
        CheckResult {
            id: CheckId::from("perf.bench"),
            status: CheckStatus::Skipped,
            tier: tier(5),
            cost: CostClass::Exhaustive,
            score: None,
            metrics: BTreeMap::new(),
            delta: None,
            findings: 0,
            duration_ms: 0,
            skipped: Some(Skipped {
                reason: SkipReason::Unavailable,
                fact: Some(FactKind::Symbols),
                lang: Some(LangId::from("go")),
                tool: Some(ToolId::from("cargo-criterion")),
                see: Some("cupel doctor".to_owned()),
                est_ms: None,
            }),
            oracle: None,
            notes: Vec::new(),
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
                    bound: Some(0.8),
                    result: RuleOutcome::Fail,
                    reason: None,
                },
                GateRule {
                    rule: "tests.weakening none".to_owned(),
                    actual: Some(0.0),
                    bound: Some(0.0),
                    result: RuleOutcome::Pass,
                    reason: None,
                },
                GateRule {
                    rule: "perf.bench.regression < 5%".to_owned(),
                    actual: None,
                    bound: None,
                    result: RuleOutcome::Skipped,
                    reason: Some("`perf.bench` did not run to completion".to_owned()),
                },
            ],
        }),
    }
}

fn rankings() -> Rankings {
    Rankings {
        top: 10,
        dimensions: vec![
            RankingDimension {
                check: CheckId::from("static.crap"),
                metric: "crap".to_owned(),
                per: MetricPer::Symbol,
                summary: "CRAP score per function.".to_owned(),
                worst: Worst::Highest,
                measured: 11,
                entries: vec![
                    RankedTarget {
                        rank: 1,
                        value: 42.5,
                        lang: LangId::from("rust"),
                        target: Target {
                            display: "rewrite::Planner::push_limit".to_owned(),
                            symbol: Some("sh_4b1d9e0c4b1d9e01".parse().expect("valid hash")),
                            location: Some(Location {
                                file: "src/rewrite/planner.rs".to_owned(),
                                line: 140,
                                col: Some(5),
                                end_line: 171,
                                end_col: Some(6),
                            }),
                        },
                    },
                    RankedTarget {
                        rank: 2,
                        value: 12.0,
                        lang: LangId::from("typescript"),
                        target: Target {
                            display: "web/limits".to_owned(),
                            symbol: None,
                            location: None,
                        },
                    },
                ],
            },
            RankingDimension {
                check: CheckId::from("static.deps"),
                metric: "coupling".to_owned(),
                per: MetricPer::Module,
                summary: "Afferent plus efferent coupling per module.".to_owned(),
                worst: Worst::Highest,
                measured: 0,
                entries: Vec::new(),
            },
        ],
        recommended: 2,
        targets: targets(),
    }
}

/// A target to protect with its protection and one value, and one to simplify with nothing
/// measured.
fn targets() -> Vec<RecommendedTarget> {
    vec![
        RecommendedTarget {
            rank: 1,
            recommend: Recommendation::Protect,
            lang: LangId::from("rust"),
            target: Target {
                display: "rewrite::Planner::push_limit".to_owned(),
                symbol: Some("sh_4b1d9e0c4b1d9e01".parse().expect("valid hash")),
                location: Some(Location {
                    file: "src/rewrite/planner.rs".to_owned(),
                    line: 140,
                    col: Some(5),
                    end_line: 171,
                    end_col: Some(6),
                }),
            },
            protection: Protection {
                mutation_score: Some(0.5),
                coverage: Some(0.8),
                tests: Some(3),
            },
            values: vec![TargetValue {
                check: CheckId::from("static.crap"),
                metric: "crap".to_owned(),
                value: 42.5,
                rank: 1,
            }],
            findings: vec!["fnd_7c2e1a".parse().expect("valid finding ID")],
        },
        RecommendedTarget {
            rank: 2,
            recommend: Recommendation::Simplify,
            lang: LangId::from("typescript"),
            target: Target {
                display: "web/limits".to_owned(),
                symbol: None,
                location: None,
            },
            protection: Protection::default(),
            values: Vec::new(),
            findings: Vec::new(),
        },
    ]
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

/// An explanation with an excerpt (one focused line), test hints, and two metrics, one of them
/// for the whole check.
fn explanation() -> Explanation {
    Explanation {
        subject: ExplainSubject::Finding,
        target: "fnd_7c2e1a".to_owned(),
        run: "run_01J9ZZZZZZZZZZZZZZZZZZZZZZ"
            .parse()
            .expect("valid run ID"),
        excerpt: Some(Excerpt {
            file: "src/rewrite/planner.rs".to_owned(),
            lines: vec![
                ExcerptLine {
                    line: 141,
                    text: "fn push_limit(limit: u64, offset: u64) -> bool {".to_owned(),
                    focus: false,
                },
                ExcerptLine {
                    line: 142,
                    text: "    limit <= offset".to_owned(),
                    focus: true,
                },
                ExcerptLine {
                    line: 143,
                    text: "}".to_owned(),
                    focus: false,
                },
            ],
        }),
        covering_tests: vec!["rewrite::limit_tests::pushes_through_sort".to_owned()],
        nearest_test_file: Some("tests/rewrite/limit_tests.rs".to_owned()),
        metrics: vec![
            Metric {
                symbol: Some("sh_4b1d9e0c4b1d9e00".parse().expect("valid symbol hash")),
                key: "cyclomatic".to_owned(),
                value: 7.0,
            },
            Metric {
                symbol: None,
                key: "score".to_owned(),
                value: 0.71,
            },
        ],
    }
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
                est_ms: None,
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
