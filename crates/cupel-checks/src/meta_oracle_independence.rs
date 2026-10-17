// ABOUTME: `meta.oracle-independence`: labels the changed functions tests exercise by whether a test that existed before
// ABOUTME: the change runs them, from TestFrames on both sides of the range and the per-test map (ADR-115).
use std::collections::BTreeMap;

use cupel_core::syntax::{NormTree, SymbolDef, Symbols, TestConventions, TestFrames};
use cupel_core::{
    ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Event, EventSink, FactKind,
    Facts, Finding, Manifest, PerTestMap, Plan, Result, Scope, Score, Stability, TestCoverage,
    Tier, names_case,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::finding::SymbolFinding;
use crate::manifest::{kind, metric};
use crate::metrics::{hashes_by_symbol, sum};
use crate::outcome;
use crate::settings::Limits;
use crate::tests_weakening::{Test, Tests, added};

const ID: &str = "meta.oracle-independence";

const KIND: &str = "evidence.self-authored-only";

/// The check has no settings.
const SETTINGS: Limits = Limits::new(&[]);

/// The note when no per-test map was made.
const NO_MAP: &str = "no per-test coverage map: which tests exercise the changed functions is \
                      unknown, so none is labeled; one is made at --depth standard where the \
                      language has a PerTestMap producer";

/// Evidence of `evidence.self-authored-only`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SelfAuthored {
    /// The commit the range starts from: no test that exercises the function existed there.
    base: String,
    /// The runner's IDs of the tests that exercise it, all added by the range.
    tests: Vec<String>,
}

pub struct MetaOracleIndependence {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(
    MetaOracleIndependence::new()
)));

impl MetaOracleIndependence {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(2).expect("tier 2 exists"),
                cost: CostClass::Quick,
                needs: vec![FactKind::Symbols, FactKind::NormTree, FactKind::TestFrames],
                wants: vec![FactKind::PerTestMap, FactKind::TestConventions],
                findings: vec![kind::<SelfAuthored>(
                    KIND,
                    ActionKind::Review,
                    "A changed function is exercised only by tests the change added: the only \
                     evidence it works was written with it.",
                )],
                metrics: vec![
                    metric(
                        "independent",
                        "Changed functions a test that existed before the change exercises.",
                    ),
                    metric(
                        "self_authored_only",
                        "Changed functions exercised only by tests the change added.",
                    ),
                ],
                settings: SETTINGS.specs(),
                stability: Stability::Experimental,
                summary: "Labels each changed function the tests exercise by whether a test that \
                          existed before the change runs it (independent) or only tests the \
                          change added do (self-authored). Runs only with --changed or --base; \
                          needs a per-test coverage map."
                    .to_owned(),
            },
        }
    }
}

impl Check for MetaOracleIndependence {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn base_needs(&self) -> &[FactKind] {
        &[FactKind::TestFrames]
    }

    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        SETTINGS.plan(
            &self.manifest,
            ctx,
            outcome::plan_symbols(&self.manifest, scope),
        )
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        SETTINGS.read(&self.manifest.id, &ctx.config)?;
        let Ok(map) = facts.get::<PerTestMap>() else {
            let mut result = outcome::result(
                &self.manifest,
                outcome::status(false),
                None,
                BTreeMap::new(),
                0,
            );
            result.notes = vec![NO_MAP.to_owned()];
            return Ok(result);
        };
        let head_frames = facts.get::<TestFrames>()?;
        let (head, base) = (
            Tests::of(head_frames),
            Tests::of(facts.base::<TestFrames>()?),
        );
        let added = added(&base, &head);
        let authored = Authored {
            head: &head,
            added: &added,
        };
        let hashes = hashes_by_symbol(facts.get::<NormTree>()?);
        let conventions = facts.get::<TestConventions>().ok();
        let commit = facts
            .range()
            .map_or_else(String::new, |range| range.commit.clone());
        let index = map.index();
        let (mut independent, mut only_self) = (0u64, 0u64);
        for symbol in facts
            .get::<Symbols>()?
            .symbols
            .iter()
            .filter(|symbol| ctx.scope.includes(&symbol.key()))
            .filter(|symbol| !conventions.is_some_and(|c| c.is_test(&symbol.file)))
            .filter(|symbol| !is_test_case(symbol, &head))
        {
            ctx.cancel.checkpoint()?;
            let covering: Vec<&TestCoverage> = index
                .covering(&symbol.file, symbol.span.line, symbol.span.end_line)
                .into_iter()
                .map(|(test, _)| test)
                .collect();
            if covering.is_empty() {
                continue;
            }
            if covering.iter().any(|test| !authored.by_range(test)) {
                independent += 1;
                continue;
            }
            only_self += 1;
            // A symbol without a structural hash cannot be identified, so it is counted but not
            // reported; every language producing NormTree hashes every symbol.
            if let Some(hash) = hashes.get(&symbol.key()) {
                sink.emit(Event::Finding(Box::new(
                    self.finding(symbol, hash, &commit, &covering),
                )));
            }
        }
        Ok(self.result(independent, only_self))
    }

    /// Function counts add up across languages.
    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        sum(name, results)
    }

    /// The independently tested share over every language's exercised functions.
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::weighted(scores)
    }
}

impl MetaOracleIndependence {
    #[allow(
        clippy::cast_precision_loss,
        reason = "function counts stay far below 2^52, where f64 is exact"
    )]
    fn result(&self, independent: u64, only_self: u64) -> CheckResult {
        let exercised = independent + only_self;
        let share = if exercised == 0 {
            0.0
        } else {
            independent as f64 / exercised as f64
        };
        let mut result = outcome::result(
            &self.manifest,
            outcome::status(false),
            Some(Score {
                value: share,
                unit: "independently tested share".to_owned(),
                n: exercised,
            }),
            BTreeMap::from([
                ("independent".to_owned(), independent as f64),
                ("self_authored_only".to_owned(), only_self as f64),
            ]),
            only_self,
        );
        result.notes = vec![format!(
            "tests that existed before the change exercise {independent} of the {exercised} \
             changed functions tests run; an oracle's independence is its verification (`cupel \
             oracle verify`)"
        )];
        result
    }

    fn finding(
        &self,
        symbol: &SymbolDef,
        hash: &cupel_core::SymbolHash,
        commit: &str,
        covering: &[&TestCoverage],
    ) -> Finding {
        let tests: Vec<String> = covering.iter().map(|test| test.id.clone()).collect();
        let title = format!(
            "`{}` is exercised only by tests the change added ({}), none that existed before it",
            symbol.display,
            tests.len()
        );
        SymbolFinding::new(
            &self.manifest,
            KIND,
            symbol,
            hash,
            SelfAuthored {
                base: commit.to_owned(),
                tests,
            },
        )
        .described(
            title,
            "Check the new tests' expectations against the task rather than the code, or show \
             the change against a test or oracle it did not write (a pre-existing test, a \
             verified oracle with gen.differential)."
                .to_owned(),
        )
        .finding()
    }
}

/// Which tests of the per-test map the range added.
struct Authored<'a> {
    head: &'a Tests<'a>,
    added: &'a [bool],
}

impl Authored<'_> {
    /// Whether the range added `test`: it is a head test case of its file (any file, when the
    /// map does not say) that no base test matches. A test of a file the range did not change
    /// is not in the head's facts, so it existed before.
    fn by_range(&self, test: &TestCoverage) -> bool {
        self.head
            .tests
            .iter()
            .zip(self.added)
            .find(|(case, _)| names(case, test))
            .is_some_and(|(_, added)| *added)
    }
}

/// Whether the map's `test` is the head test `case`.
fn names(case: &Test<'_>, test: &TestCoverage) -> bool {
    test.file.as_deref().is_none_or(|file| file == case.file)
        && names_case(&test.id, &case.case.name)
}

/// Whether `symbol` is a test case's own function: it overlaps a case of its file.
fn is_test_case(symbol: &SymbolDef, head: &Tests<'_>) -> bool {
    head.tests.iter().any(|test| {
        test.file == symbol.file
            && test.case.span.line <= symbol.span.end_line
            && symbol.span.line <= test.case.span.end_line
    })
}
