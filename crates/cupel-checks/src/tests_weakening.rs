// ABOUTME: `tests.weakening`: diffs TestFrames between a range's base and head and fails on removed and
// ABOUTME: loosened assertions, newly skipped and deleted tests, and rewritten snapshots (ADR-056, ADR-057).
use std::collections::{BTreeMap, HashMap};

use cupel_core::git::ChangeKind;
use cupel_core::syntax::{AssertionKind, AssertionStrength, Span, TestCase, TestFrames};
use cupel_core::{
    Action, ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Event, EventSink,
    Evidence, FactKind, Facts, Finding, FindingId, Location, Manifest, Plan, Result, Scope, Score,
    Severity, Stability, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::manifest::{evidence, kind, metric};
use crate::metrics::sum;
use crate::outcome;
use crate::settings::Limits;

const ID: &str = "tests.weakening";

/// The check has no settings; any key is a configuration error (ADR-057).
const SETTINGS: Limits = Limits::new(&[]);

/// Evidence of `assertion.removed`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AssertionRemoved {
    /// The test's name.
    test: String,
    /// Assertions the test made at the base commit.
    base: u32,
    /// Assertions it makes at head.
    head: u32,
}

/// Evidence of `matcher.loosened`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct MatcherLoosened {
    test: String,
    /// Assertions that pin one value, at the base commit.
    strong_base: u32,
    /// Assertions that pin one value, at head.
    strong_head: u32,
    /// Assertions that allow many values (truthiness, presence, inequality), at base.
    weak_base: u32,
    /// Weak assertions at head.
    weak_head: u32,
}

/// Evidence of `test.skipped`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TestSkipped {
    test: String,
    /// The reason the skip marker gives, if any.
    reason: Option<String>,
}

/// Evidence of `test.deleted`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TestDeleted {
    test: String,
    /// Where the test was at the base commit.
    file: String,
    line: u32,
}

/// Evidence of `snapshot.rewritten`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SnapshotRewritten {
    /// The snapshot file the range modified.
    snapshot: String,
    /// The changed tests the snapshot belongs to; empty when no changed test file claims it.
    tests: Vec<String>,
}

pub struct TestsWeakening {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(TestsWeakening::new())));

impl TestsWeakening {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(2).expect("tier 2 exists"),
                cost: CostClass::Quick,
                needs: vec![FactKind::TestFrames],
                wants: vec![],
                findings: vec![
                    kind::<AssertionRemoved>(
                        "assertion.removed",
                        ActionKind::RestoreTest,
                        "A test makes fewer assertions than at the base commit.",
                    ),
                    kind::<TestSkipped>(
                        "test.skipped",
                        ActionKind::RestoreTest,
                        "A test that ran at the base commit is skipped or disabled.",
                    ),
                    kind::<MatcherLoosened>(
                        "matcher.loosened",
                        ActionKind::RestoreTest,
                        "A test replaced assertions that pin one value with weaker ones \
                         (truthiness, presence, inequality).",
                    ),
                    kind::<SnapshotRewritten>(
                        "snapshot.rewritten",
                        ActionKind::RestoreTest,
                        "A stored snapshot that existed at the base commit was changed.",
                    ),
                    kind::<TestDeleted>(
                        "test.deleted",
                        ActionKind::RestoreTest,
                        "A test that existed at the base commit is gone.",
                    ),
                ],
                metrics: vec![metric(
                    "weakened",
                    "Tests and snapshots the range weakened; the gate key \
                     `tests.weakening = \"none\"` requires zero.",
                )],
                stability: Stability::Experimental,
                summary: "Diff of the test suite against the range's base: removed and loosened \
                          assertions, skipped and deleted tests, rewritten snapshots. Runs only \
                          with --changed or --base."
                    .to_owned(),
            },
        }
    }
}

impl Check for TestsWeakening {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn base_needs(&self) -> &[FactKind] {
        &[FactKind::TestFrames]
    }

    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        SETTINGS.read(&self.manifest.id, &ctx.config)?;
        Ok(outcome::plan_files(&self.manifest, scope))
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        SETTINGS.read(&self.manifest.id, &ctx.config)?;
        let head = Tests::of(facts.get::<TestFrames>()?);
        let base = Tests::of(facts.base::<TestFrames>()?);
        let range = facts.range();
        let commit = range.map_or("", |range| range.commit.as_str());
        let matched = matches(&base, &head);
        let mut findings = Vec::new();
        for (base_index, head_index) in matched.iter().enumerate() {
            ctx.cancel.checkpoint()?;
            let old = &base.tests[base_index];
            match head_index {
                Some(index) => findings.extend(self.compare(old, &head.tests[*index], commit)),
                None => findings.push(self.deleted(old, commit)),
            }
        }
        let modified_snapshots = range
            .into_iter()
            .flat_map(|range| &range.changes)
            .filter(|change| change.kind == ChangeKind::Modified && is_snapshot(&change.path))
            .map(|change| change.path.as_str());
        for snapshot in modified_snapshots {
            if let Some(finding) = self.snapshot(snapshot, &head, &matched, commit) {
                findings.push(finding);
            }
        }
        let count = findings.len() as u64;
        #[allow(
            clippy::cast_precision_loss,
            reason = "finding counts stay far below 2^52, where f64 is exact"
        )]
        let value = count as f64;
        for finding in findings {
            sink.emit(Event::Finding(Box::new(finding)));
        }
        Ok(outcome::result(
            &self.manifest,
            outcome::status(count > 0),
            Some(Score {
                value,
                unit: "weakened".to_owned(),
                n: matched.iter().flatten().count() as u64,
            }),
            BTreeMap::from([("weakened".to_owned(), value)]),
            count,
        ))
    }

    /// Weakenings add up across languages (ADR-063).
    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        sum(name, results)
    }

    /// The total over every language: weakenings add up (ADR-057).
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::summed(scores)
    }
}

/// One test with where it is and how its file names repeats of its name.
struct Test<'a> {
    file: &'a str,
    case: &'a TestCase,
    /// 0 for the first test of this name in its file, 1 for the second, and so on.
    repeat: usize,
}

impl Test<'_> {
    /// The structural subject of a finding about this test: file, name, and repeat.
    fn subject(&self) -> String {
        if self.repeat == 0 {
            format!("{}::{}", self.file, self.case.name)
        } else {
            format!("{}::{}#{}", self.file, self.case.name, self.repeat)
        }
    }

    fn has_snapshot(&self) -> bool {
        self.case
            .assertions
            .iter()
            .any(|assertion| assertion.kind == AssertionKind::Snapshot)
    }
}

/// Every test of one side, in fact order.
struct Tests<'a> {
    tests: Vec<Test<'a>>,
}

impl<'a> Tests<'a> {
    fn of(frames: &'a TestFrames) -> Self {
        let mut tests = Vec::new();
        for file in &frames.files {
            let mut seen: HashMap<&str, usize> = HashMap::new();
            for case in &file.cases {
                let repeat = seen.entry(case.name.as_str()).or_default();
                tests.push(Test {
                    file: &file.file,
                    case,
                    repeat: *repeat,
                });
                *repeat += 1;
            }
        }
        Self { tests }
    }
}

/// For each base test, the index of its head test: same file, name, and repeat first; then
/// the first unmatched head test of the same name in another file (a move); else `None`.
fn matches(base: &Tests<'_>, head: &Tests<'_>) -> Vec<Option<usize>> {
    let mut taken = vec![false; head.tests.len()];
    let mut matched: Vec<Option<usize>> = base
        .tests
        .iter()
        .map(|old| {
            let index = head.tests.iter().position(|new| {
                new.file == old.file && new.case.name == old.case.name && new.repeat == old.repeat
            })?;
            taken[index] = true;
            Some(index)
        })
        .collect();
    for (old, slot) in base.tests.iter().zip(&mut matched) {
        if slot.is_some() {
            continue;
        }
        let moved = head
            .tests
            .iter()
            .enumerate()
            .position(|(index, new)| !taken[index] && new.case.name == old.case.name);
        if let Some(index) = moved {
            taken[index] = true;
            *slot = Some(index);
        }
    }
    matched
}

/// Assertion counts of one test.
struct Counts {
    total: u32,
    strong: u32,
    weak: u32,
}

impl Counts {
    fn of(case: &TestCase) -> Self {
        let count = |strength| {
            u32::try_from(
                case.assertions
                    .iter()
                    .filter(|assertion| assertion.strength == strength)
                    .count(),
            )
            .unwrap_or(u32::MAX)
        };
        let (strong, weak) = (
            count(AssertionStrength::Strong),
            count(AssertionStrength::Weak),
        );
        Self {
            total: strong + weak,
            strong,
            weak,
        }
    }
}

/// A path that holds a stored snapshot: below a `__snapshots__` or `snapshots` directory, or
/// with a `.snap` or `.ambr` extension (ADR-057).
fn is_snapshot(path: &str) -> bool {
    let mut segments = path.split('/');
    let file = segments.next_back().unwrap_or_default();
    segments.any(|segment| segment == "__snapshots__" || segment == "snapshots")
        || file.rsplit_once('.').is_some_and(|(_, extension)| {
            extension.eq_ignore_ascii_case("snap") || extension.eq_ignore_ascii_case("ambr")
        })
}

/// The directory part of `path`, with a trailing `/`; empty at the root.
fn directory(path: &str) -> &str {
    path.rfind('/').map_or("", |at| &path[..=at])
}

/// `path`'s file name without its last extension.
fn stem(path: &str) -> &str {
    let name = &path[directory(path).len()..];
    name.rfind('.').map_or(name, |at| &name[..at])
}

/// What a finding says: its kind, title, evidence, and what to do.
struct Said {
    kind: &'static str,
    title: String,
    evidence: Value,
    summary: String,
}

impl TestsWeakening {
    /// What weakened between a base test and its head counterpart.
    fn compare(&self, old: &Test<'_>, new: &Test<'_>, commit: &str) -> Vec<Finding> {
        let (before, after) = (Counts::of(old.case), Counts::of(new.case));
        [
            skipped(old, new),
            removed(new, &before, &after, commit),
            loosened(new, &before, &after),
        ]
        .into_iter()
        .flatten()
        .map(|said| self.report(said, new, commit))
        .collect()
    }

    fn deleted(&self, old: &Test<'_>, commit: &str) -> Finding {
        let name = &old.case.name;
        let said = Said {
            kind: "test.deleted",
            title: format!("`{name}` was deleted"),
            evidence: evidence(&TestDeleted {
                test: name.clone(),
                file: old.file.to_owned(),
                line: old.case.span.line,
            }),
            summary: format!(
                "Restore `{name}` from {}:{} at {commit}, or move its assertions to the test \
                 that replaces it.",
                old.file, old.case.span.line
            ),
        };
        self.report(said, old, commit)
    }

    /// `snapshot.rewritten` for a modified snapshot, attributed to the head tests it belongs
    /// to; `None` when one of them is new, since then the snapshot may only have grown
    /// (ADR-057).
    fn snapshot(
        &self,
        snapshot: &str,
        head: &Tests<'_>,
        matched: &[Option<usize>],
        commit: &str,
    ) -> Option<Finding> {
        let owners = owners(snapshot, head);
        if owners.iter().any(|index| !matched.contains(&Some(*index))) {
            return None;
        }
        let tests: Vec<String> = owners
            .iter()
            .map(|index| head.tests[*index].case.name.clone())
            .collect();
        let said = Said {
            kind: "snapshot.rewritten",
            title: format!("snapshot `{snapshot}` was rewritten"),
            evidence: evidence(&SnapshotRewritten {
                snapshot: snapshot.to_owned(),
                tests,
            }),
            summary: format!(
                "Restore `{snapshot}` from {commit} and fix the code until it matches, or \
                 justify the new expected output in review."
            ),
        };
        let location = owners.first().map_or_else(
            || Location {
                file: snapshot.to_owned(),
                line: 1,
                col: None,
                end_line: 1,
                end_col: None,
            },
            |index| location(head.tests[*index].file, &head.tests[*index].case.span),
        );
        Some(self.finding(said, snapshot, location, commit))
    }

    /// A finding about `test`, located at the test.
    fn report(&self, said: Said, test: &Test<'_>, commit: &str) -> Finding {
        let location = location(test.file, &test.case.span);
        self.finding(said, &test.subject(), location, commit)
    }

    /// A `fail` finding identified by its structural subject (ADR-057).
    fn finding(&self, said: Said, subject: &str, location: Location, commit: &str) -> Finding {
        let id = FindingId::for_subject(&self.manifest.id, said.kind, subject);
        let evidence = match said.evidence {
            Value::Object(fields) => fields,
            _ => Map::new(),
        };
        Finding {
            suppress: format!("cupel suppress {id} --reason '…'"),
            repro: format!("cupel check --only {} --base {commit}", self.manifest.id),
            id,
            check: self.manifest.id.clone(),
            kind: said.kind.to_owned(),
            severity: Severity::Fail,
            confidence: 1.0,
            tier: self.manifest.tier,
            symbol: None,
            location: Some(location),
            title: said.title,
            detail: None,
            evidence: Evidence(evidence),
            action: Action {
                kind: ActionKind::RestoreTest,
                priority: 1,
                summary: said.summary,
                hints: Map::new(),
            },
            new_since_baseline: false,
            suppressed: false,
        }
    }
}

/// `test.skipped` when the head test is skipped and the base one was not.
fn skipped(old: &Test<'_>, new: &Test<'_>) -> Option<Said> {
    let name = &new.case.name;
    let skip = new.case.skip.as_ref().filter(|_| old.case.skip.is_none())?;
    Some(Said {
        kind: "test.skipped",
        title: format!("`{name}` is now skipped"),
        evidence: evidence(&TestSkipped {
            test: name.clone(),
            reason: skip.reason.clone(),
        }),
        summary: format!("Remove the skip marker from `{name}` and fix what made it fail."),
    })
}

/// `assertion.removed` when the head test makes fewer assertions.
fn removed(new: &Test<'_>, before: &Counts, after: &Counts, commit: &str) -> Option<Said> {
    let name = &new.case.name;
    (after.total < before.total).then(|| Said {
        kind: "assertion.removed",
        title: format!(
            "`{name}` makes {} assertions, down from {}",
            after.total, before.total
        ),
        evidence: evidence(&AssertionRemoved {
            test: name.clone(),
            base: before.total,
            head: after.total,
        }),
        summary: format!(
            "Restore the assertions removed from `{name}` (see it at {commit}); if one is \
             wrong, replace it with a correct one rather than dropping it."
        ),
    })
}

/// `matcher.loosened` when strong assertions became weak ones.
fn loosened(new: &Test<'_>, before: &Counts, after: &Counts) -> Option<Said> {
    let name = &new.case.name;
    let loosened = before
        .strong
        .saturating_sub(after.strong)
        .min(after.weak.saturating_sub(before.weak));
    (loosened > 0).then(|| Said {
        kind: "matcher.loosened",
        title: format!("`{name}` replaced {loosened} exact assertion(s) with weaker ones"),
        evidence: evidence(&MatcherLoosened {
            test: name.clone(),
            strong_base: before.strong,
            strong_head: after.strong,
            weak_base: before.weak,
            weak_head: after.weak,
        }),
        summary: format!(
            "Restore the exact expected values in `{name}` instead of truthiness, presence, or \
             inequality checks."
        ),
    })
}

/// The head tests a snapshot belongs to: those whose file's stem appears in the snapshot path
/// below the test's directory, narrowed to those whose name appears too when any does.
fn owners(snapshot: &str, head: &Tests<'_>) -> Vec<usize> {
    let owners: Vec<usize> = head
        .tests
        .iter()
        .enumerate()
        .filter(|(_, test)| test.has_snapshot())
        .filter_map(|(index, test)| {
            let below = snapshot.strip_prefix(directory(test.file))?;
            below.contains(stem(test.file)).then_some(index)
        })
        .collect();
    let named: Vec<usize> = owners
        .iter()
        .copied()
        .filter(|index| {
            let test = &head.tests[*index];
            snapshot[directory(test.file).len()..].contains(test.case.name.as_str())
        })
        .collect();
    if named.is_empty() { owners } else { named }
}

fn location(file: &str, span: &Span) -> Location {
    Location {
        file: file.to_owned(),
        line: span.line,
        col: Some(span.col),
        end_line: span.end_line,
        end_col: Some(span.end_col),
    }
}
