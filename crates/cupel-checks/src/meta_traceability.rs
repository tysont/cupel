// ABOUTME: `meta.traceability`: each requirement ID the task lists must be named by a test the range added, read from
// ABOUTME: TestFrames on both sides of the range; a requirement with none is `spec.untested-requirement` (ADR-114).
use std::collections::BTreeMap;

use cupel_core::config::{ConfigError, Table};
use cupel_core::syntax::TestFrames;
use cupel_core::{
    ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Event, EventSink, FactKind,
    Facts, Plan, Result, Scope, Score, Severity, Stability, Tier,
};
use cupel_core::{FindingKindSpec, Manifest, SettingSpec};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::finding::SubjectFinding;
use crate::manifest::{evidence, kind, metric, setting};
use crate::metrics::sum;
use crate::outcome;
use crate::tests_weakening::{Tests, added};

const ID: &str = "meta.traceability";

const KIND: &str = "spec.untested-requirement";

/// The one setting: the task's requirement IDs.
const REQUIREMENTS: &str = "requirements";

/// Evidence of `spec.untested-requirement`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Untested {
    /// The requirement ID, as configured.
    requirement: String,
    /// The commit the range starts from.
    base: String,
    /// The tests in scope whose names carry the ID, none of them added by the range.
    tests: Vec<Naming>,
}

/// A test whose name carries a requirement ID; inlined, as evidence schemas have no `$defs`.
#[derive(Serialize, JsonSchema)]
#[schemars(inline)]
#[serde(deny_unknown_fields)]
struct Naming {
    file: String,
    name: String,
    line: u32,
    /// Whether the range added it (always false here: one it added would trace the
    /// requirement).
    added: bool,
}

pub struct MetaTraceability {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(MetaTraceability::new())));

impl MetaTraceability {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(2).expect("tier 2 exists"),
                cost: CostClass::Standard,
                needs: vec![FactKind::TestFrames],
                wants: vec![],
                findings: vec![untested_kind()],
                metrics: vec![metric(
                    "untested",
                    "Requirements no test added by the range names.",
                )],
                settings: specs(),
                stability: Stability::Experimental,
                summary: "Maps each requirement ID the task lists (`requirements`) to the tests \
                          the range added whose names carry it (`test_req_12_…`, `… #REQ-12`) \
                          and reports each requirement without one. Runs only with --changed or \
                          --base."
                    .to_owned(),
            },
        }
    }
}

/// `spec.untested-requirement`, as the manifest declares it.
fn untested_kind() -> FindingKindSpec {
    kind::<Untested>(
        KIND,
        ActionKind::WriteTest,
        "A requirement of the task has no test the change added whose name carries its ID, so \
         nothing shows the change meets it.",
    )
}

/// The settings the manifest declares (ADR-111).
fn specs() -> Vec<SettingSpec> {
    vec![setting(
        REQUIREMENTS,
        Value::Array(Vec::new()),
        "The task's requirement IDs, such as `REQ-12`; a per-language override assigns them to \
         the language whose tests trace them.",
    )]
}

impl Check for MetaTraceability {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn base_needs(&self) -> &[FactKind] {
        &[FactKind::TestFrames]
    }

    fn plan(&self, ctx: &CheckCtx, _: &Scope) -> Result<Plan> {
        let requirements = requirements(&self.manifest.id, &ctx.config)?;
        Ok(Plan::estimate(
            self.manifest.cost,
            requirements.len() as u64,
        ))
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        let requirements = requirements(&self.manifest.id, &ctx.config)?;
        if requirements.is_empty() {
            return Ok(outcome::unmeasured(
                &self.manifest,
                format!(
                    "no requirements are configured: list their IDs as `{REQUIREMENTS}` under \
                     [checks.\"{ID}\"]"
                ),
            ));
        }
        let (head, base) = (
            Tests::of(facts.get::<TestFrames>()?),
            Tests::of(facts.base::<TestFrames>()?),
        );
        let added = added(&base, &head);
        let commit = facts
            .range()
            .map_or_else(String::new, |range| range.commit.clone());
        let mut untested = 0u64;
        for requirement in &requirements {
            ctx.cancel.checkpoint()?;
            let naming = naming(&head, &added, requirement);
            if naming.iter().any(|test| test.added) {
                continue;
            }
            untested += 1;
            let finding = self.finding(requirement, &commit, naming);
            sink.emit(Event::Finding(Box::new(finding)));
        }
        Ok(self.result(untested, requirements.len() as u64))
    }

    /// Untested requirements add up across languages.
    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        sum(name, results)
    }

    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::summed(scores)
    }
}

impl MetaTraceability {
    /// The result of `untested` of `requirements`.
    fn result(&self, untested: u64, requirements: u64) -> CheckResult {
        #[allow(
            clippy::cast_precision_loss,
            reason = "requirement counts stay far below 2^52, where f64 is exact"
        )]
        let value = untested as f64;
        outcome::result(
            &self.manifest,
            outcome::status(untested > 0),
            Some(Score {
                value,
                unit: "untested requirements".to_owned(),
                n: requirements,
            }),
            BTreeMap::from([("untested".to_owned(), value)]),
            untested,
        )
    }

    fn finding(&self, requirement: &str, commit: &str, naming: Vec<Naming>) -> cupel_core::Finding {
        let title = match naming.len() {
            0 => format!("requirement {requirement} has no test that names it"),
            1 => format!(
                "requirement {requirement} has no test the change added; 1 test that existed \
                 before names it"
            ),
            n => format!(
                "requirement {requirement} has no test the change added; {n} tests that existed \
                 before name it"
            ),
        };
        SubjectFinding {
            kind: KIND,
            subject: requirement.to_owned(),
            severity: Severity::Warn,
            confidence: 1.0,
            title,
            summary: format!(
                "Add a test that fails without the change and name it after the requirement \
                 (`test_{}_…` or `… #{requirement}`).",
                token(requirement)
            ),
            evidence: evidence(&Untested {
                requirement: requirement.to_owned(),
                base: commit.to_owned(),
                tests: naming,
            }),
            location: None,
            repro: format!("cupel check --changed --only {ID}"),
        }
        .finding(&self.manifest)
    }
}

/// The head tests whose names carry `requirement`, each with whether the range added it.
fn naming(head: &Tests<'_>, added: &[bool], requirement: &str) -> Vec<Naming> {
    head.tests
        .iter()
        .zip(added)
        .filter(|(test, _)| names(&test.case.name, requirement))
        .map(|(test, added)| Naming {
            file: test.file.to_owned(),
            name: test.case.name.clone(),
            line: test.case.span.line,
            added: *added,
        })
        .collect()
}

/// The configured requirement IDs: a list of non-empty strings, and no other key.
fn requirements(check: &CheckId, config: &Table) -> Result<Vec<String>> {
    let error = |key: &str, reason: &str| ConfigError::CheckSetting {
        check: check.to_string(),
        key: key.to_owned(),
        reason: reason.to_owned(),
        valid: vec![REQUIREMENTS.to_owned()],
    };
    if let Some(unknown) = config.keys().find(|key| key.as_str() != REQUIREMENTS) {
        return Err(error(unknown, "not a setting of this check").into());
    }
    let Some(value) = config.get(REQUIREMENTS) else {
        return Ok(Vec::new());
    };
    let expected = "expected a list of requirement IDs, such as [\"REQ-12\"]";
    let Value::Array(items) = value else {
        return Err(error(REQUIREMENTS, expected).into());
    };
    items
        .iter()
        .map(|item| match item {
            Value::String(id) if !token(id).is_empty() => Ok(id.clone()),
            _ => Err(error(REQUIREMENTS, expected).into()),
        })
        .collect()
}

/// `text` lowercased, every run of other characters than letters and digits one `_`, trimmed.
fn token(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_owned()
}

/// Whether the test `name` carries the requirement `id` as whole words: `REQ-12` in
/// `test_req_12_parses` and `parses #REQ-12`, not in `test_req_120` or `testReq12`.
fn names(name: &str, id: &str) -> bool {
    format!("_{}_", token(name)).contains(&format!("_{}_", token(id)))
}
