// ABOUTME: `spec.prove` (ADR-118): proof obligations a deductive verifier did not discharge, from the ProofResults fact:
// ABOUTME: a failing finding per obligation it could not prove, a warning per obligation assumed or left undecided.
use std::collections::{BTreeMap, HashMap};

use cupel_core::{
    ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Event, EventSink, FactKind,
    Facts, Finding, Manifest, Obligation, ObligationStatus, Plan, ProofResults, Result, RunStatus,
    Scope, Score, Severity, Stability, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::finding::SubjectFinding;
use crate::manifest::{evidence, kind, metric};
use crate::metrics::sum;
use crate::outcome;
use crate::settings::Limits;

const ID: &str = "spec.prove";

/// The check takes no settings: what is proved is the proofs' own.
const LIMITS: Limits = Limits::new(&[]);

/// The metrics, each with what it counts.
const METRICS: [(&str, &str); 3] = [
    ("verified", "Declarations the verifier verified completely."),
    ("failed", "Obligations the verifier could not prove."),
    (
        "open",
        "Obligations assumed instead of proved, or left undecided in the verifier's time or \
         resources.",
    ),
];

/// Evidence of `proof.failed` and `proof.obligation-open`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Undischarged {
    /// The verifier, such as `tool.dafny`.
    tool: String,
    /// What the verifier says, in its words.
    message: String,
    /// The places it relates to the obligation, such as the postcondition a return path breaks.
    related: Vec<Related>,
}

/// One related place; inlined, as evidence schemas have no `$defs` (ADR-043).
#[derive(Serialize, JsonSchema)]
#[schemars(inline)]
#[serde(deny_unknown_fields)]
struct Related {
    message: String,
    file: String,
    line: u32,
}

/// What each obligation status is: its finding kind, severity, what the kind means, and what
/// its action asks.
struct Said {
    kind: &'static str,
    severity: Severity,
    action: ActionKind,
    means: &'static str,
    asks: &'static str,
    title: &'static str,
}

const FAILED: Said = Said {
    kind: "proof.failed",
    severity: Severity::Fail,
    action: ActionKind::FixBug,
    means: "The verifier could not prove a proof obligation: a postcondition, an assertion, a \
            loop invariant, a precondition at a call.",
    asks: "Fix the code so the obligation holds, or strengthen the proof (an invariant, a \
           lemma, an assertion that guides the verifier), then verify again.",
    title: "proof fails",
};

const OPEN: Said = Said {
    kind: "proof.obligation-open",
    severity: Severity::Warn,
    action: ActionKind::Review,
    means: "A proof obligation is not discharged: the proof assumes it (an `assume`, a lemma or \
            method without a body), or the verifier ran out of time or resources on it.",
    asks: "Prove what is assumed (replace the assumption with an assertion and its proof, give \
           the lemma a body), or split the proof so the verifier decides it in its budget.",
    title: "proof obligation open",
};

pub struct SpecProve {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(SpecProve::new())));

impl SpecProve {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(6).expect("tier 6 exists"),
                cost: CostClass::Exhaustive,
                needs: vec![FactKind::ProofResults],
                wants: vec![],
                findings: [FAILED, OPEN]
                    .iter()
                    .map(|said| kind::<Undischarged>(said.kind, said.action, said.means))
                    .collect(),
                metrics: METRICS
                    .iter()
                    .map(|(name, counts)| metric(name, counts))
                    .collect(),
                settings: LIMITS.specs(),
                stability: Stability::Experimental,
                summary: "Verifies the project's proofs with a deductive verifier (Dafny) and \
                          reports each obligation it could not prove and each the proofs assume \
                          or leave undecided."
                    .to_owned(),
            },
        }
    }
}

impl Check for SpecProve {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        LIMITS.plan_files(&self.manifest, ctx, scope)
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        LIMITS.read(&self.manifest.id, &ctx.config)?;
        let fact = facts.get::<ProofResults>()?;
        let mut repeats: HashMap<String, usize> = HashMap::new();
        for obligation in &fact.obligations {
            let subject = format!("{}|{}", obligation.location.file, obligation.message);
            let repeat = repeats.entry(subject.clone()).or_default();
            *repeat += 1;
            let finding = self.finding(fact, obligation, format!("{subject}|{repeat}"));
            sink.emit(Event::Finding(Box::new(finding)));
        }
        let count = |status: ObligationStatus| {
            fact.obligations
                .iter()
                .filter(|obligation| obligation.status == status)
                .count()
        };
        let (failed, open) = (
            count(ObligationStatus::Failed),
            count(ObligationStatus::Open),
        );
        let measured = fact.runs.iter().any(|run| run.status == RunStatus::Ran);
        let mut result = outcome::result(
            &self.manifest,
            outcome::status(failed > 0),
            measured.then(|| outcome::counted(failed + open, "undischarged", fact.files.len())),
            if measured {
                outcome::counts(&[
                    (
                        "verified",
                        usize::try_from(fact.verified).unwrap_or(usize::MAX),
                    ),
                    ("failed", failed),
                    ("open", open),
                ])
            } else {
                BTreeMap::new()
            },
            fact.obligations.len() as u64,
        );
        result.notes = notes(fact);
        Ok(result)
    }

    /// Obligations and proof files add up across languages.
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::summed(scores)
    }

    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        sum(name, results)
    }
}

/// A note for each root whose run failed, or that the verifier found nothing to verify.
fn notes(fact: &ProofResults) -> Vec<String> {
    if fact.files.is_empty() && fact.runs.is_empty() {
        return vec![format!(
            "{} found no proof file in the project roots",
            fact.tool
        )];
    }
    fact.runs
        .iter()
        .filter(|run| run.status == RunStatus::Failed)
        .map(|run| {
            format!(
                "{} failed in {}: {}",
                fact.tool,
                run.dir,
                run.detail.as_deref().unwrap_or("no detail")
            )
        })
        .collect()
}

impl SpecProve {
    fn finding(&self, fact: &ProofResults, obligation: &Obligation, subject: String) -> Finding {
        let said = match obligation.status {
            ObligationStatus::Failed => FAILED,
            ObligationStatus::Open => OPEN,
        };
        let at = &obligation.location;
        SubjectFinding {
            kind: said.kind,
            subject,
            severity: said.severity,
            confidence: 1.0,
            title: format!(
                "{} at {}:{}: {}",
                said.title, at.file, at.line, obligation.message
            ),
            summary: said.asks.to_owned(),
            evidence: evidence(&Undischarged {
                tool: fact.tool.to_string(),
                message: obligation.message.clone(),
                related: obligation
                    .related
                    .iter()
                    .map(|related| Related {
                        message: related.message.clone(),
                        file: related.location.file.clone(),
                        line: related.location.line,
                    })
                    .collect(),
            }),
            location: Some(at.clone()),
            repro: format!("cupel check --only {ID} --paths {}", at.file),
        }
        .finding(&self.manifest)
    }
}
