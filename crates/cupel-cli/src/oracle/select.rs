// ABOUTME: Which oracle a run's checks compare against (ADR-091 §2–3): `--oracle`, else the first verified,
// ABOUTME: else the first defined; a check that uses one when none is defined is skipped as unconfigured.
use cupel_core::config::Config;
use cupel_core::{
    Check, CheckRegistry, CheckResult, CheckStatus, OracleState, Outcome, PlannedSkip, SkipReason,
    Skipped, Writer,
};

use super::{defined, report};
use crate::error::CupelError;
use crate::persist;

/// Where a check that uses an oracle, with none defined, points the reader.
const SEE: &str = "cupel describe oracle add";

/// The oracle chosen for a run, and the checks left out because none is defined.
pub struct Selected {
    name: Option<String>,
    unconfigured: Vec<CheckResult>,
}

impl Selected {
    /// Chooses the oracle for the checks in `selected` (`flag` is `--oracle`, which must name a
    /// defined oracle whenever it is given), removing from `selected` the checks that use one
    /// when none is defined.
    pub fn choose(
        writer: &Writer,
        config: &Config,
        flag: Option<&str>,
        selected: &mut Vec<&dyn Check>,
    ) -> Result<Self, CupelError> {
        if let Some(name) = flag {
            defined(config, name, "--oracle")?;
        }
        if !selected.iter().any(|check| check.uses_oracle()) {
            return Ok(Self {
                name: None,
                unconfigured: Vec::new(),
            });
        }
        let name = match flag {
            Some(name) => Some(name.to_owned()),
            None => first_verified(writer, config)?
                .or_else(|| config.file.oracles.keys().next().cloned()),
        };
        let mut unconfigured = Vec::new();
        if name.is_none() {
            selected.retain(|check| {
                let uses = check.uses_oracle();
                if uses {
                    unconfigured.push(skipped(*check));
                }
                !uses
            });
        }
        Ok(Self { name, unconfigured })
    }

    /// Names the oracle on each result, that ran, of a check that uses one, and reports the
    /// checks left out as skipped (in the plan of a dry run).
    pub fn apply(self, outcome: &mut Outcome, checks: &CheckRegistry) {
        if let Some(name) = &self.name {
            let uses = |result: &CheckResult| {
                result.skipped.is_none() && checks.get(&result.id).is_some_and(Check::uses_oracle)
            };
            for result in outcome.checks.iter_mut().filter(|result| uses(result)) {
                result.oracle = Some(name.clone());
            }
        }
        for result in self.unconfigured {
            match &mut outcome.plan {
                Some(plan) => plan.would_skip.push(PlannedSkip {
                    id: result.id,
                    skipped: result.skipped.expect("an unconfigured check is skipped"),
                }),
                None => outcome.checks.push(result),
            }
        }
    }
}

/// The first oracle, by name, that is verified now.
fn first_verified(writer: &Writer, config: &Config) -> Result<Option<String>, CupelError> {
    let store = persist::open_for_reading(writer)?;
    for (name, definition) in &config.file.oracles {
        if report(writer.root(), store.as_ref(), name, definition)?.state == OracleState::Verified {
            return Ok(Some(name.clone()));
        }
    }
    Ok(None)
}

/// The result of `check`, skipped because it uses an oracle and none is defined.
fn skipped(check: &dyn Check) -> CheckResult {
    let manifest = check.manifest();
    CheckResult {
        id: manifest.id.clone(),
        status: CheckStatus::Skipped,
        tier: manifest.tier,
        cost: manifest.cost,
        score: None,
        metrics: std::collections::BTreeMap::new(),
        delta: None,
        findings: 0,
        duration_ms: 0,
        skipped: Some(Skipped {
            reason: SkipReason::Unconfigured,
            fact: None,
            lang: None,
            tool: None,
            see: Some(SEE.to_owned()),
            est_ms: None,
        }),
        oracle: None,
        notes: Vec::new(),
    }
}
