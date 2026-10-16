// ABOUTME: The tier cap of ADR-091 §4: a result against an oracle that is not verified now is capped at tier
// ABOUTME: 3 and its oracle gets one `oracle.unverified` finding, applied while grading so everything sees it.
use std::collections::BTreeMap;

use cupel_core::config::Config;
use cupel_core::{
    ActionKind, Envelope, FindingKindSpec, OracleState, Severity, SeverityCounts, Tier, Writer,
};
use serde_json::{Map, Value, json};

use super::report;
use crate::error::CupelError;
use crate::persist;
use crate::problem::Problem;

/// The kind of the finding a result against an unverified oracle carries.
const UNVERIFIED: &str = "oracle.unverified";
/// The highest tier a result against an oracle that is not verified may report.
const CAP: u8 = 3;

/// Caps every result of `envelope` whose oracle is not verified now, with one finding per such
/// oracle; `repro` is the run's command line.
pub fn cap(
    envelope: &mut Envelope,
    writer: &Writer,
    config: &Config,
    repro: &str,
) -> Result<(), CupelError> {
    let used: Vec<&String> = envelope
        .checks
        .iter()
        .filter_map(|result| result.oracle.as_ref())
        .collect();
    if used.is_empty() {
        return Ok(());
    }
    let store = persist::open_for_reading(writer)?;
    let mut states = BTreeMap::new();
    for name in used {
        let state = match config.file.oracles.get(name) {
            Some(definition) => report(writer.root(), store.as_ref(), name, definition)?.state,
            None => OracleState::Unreachable,
        };
        states.insert(name.clone(), state);
    }
    capped(envelope, &states, repro);
    Ok(())
}

/// Caps `envelope`'s results by their oracles' `states` (an oracle missing from them is
/// unreachable) and recounts its findings.
fn capped(envelope: &mut Envelope, states: &BTreeMap<String, OracleState>, repro: &str) {
    let ceiling = Tier::try_from(CAP).expect("tier 3 is valid");
    let mut capped: BTreeMap<String, (OracleState, Vec<String>)> = BTreeMap::new();
    for result in &mut envelope.checks {
        let Some(name) = &result.oracle else {
            continue;
        };
        let state = states
            .get(name)
            .copied()
            .unwrap_or(OracleState::Unreachable);
        if state == OracleState::Verified {
            continue;
        }
        result.tier = result.tier.min(ceiling);
        let entry = capped.entry(name.clone()).or_insert((state, Vec::new()));
        entry.1.push(result.id.to_string());
    }
    for (name, (state, mut checks)) in capped {
        checks.sort();
        envelope
            .findings
            .push(unverified(&name, state, &checks).finding(repro));
    }
    envelope.summary.findings = SeverityCounts::of(&envelope.findings);
    envelope.total_findings = envelope.findings.len() as u64;
}

/// The `oracle.unverified` finding of the oracle `name` in `state`, which capped `checks`.
fn unverified(name: &str, state: OracleState, checks: &[String]) -> Problem {
    Problem {
        check: "oracle",
        kind: UNVERIFIED,
        subject: name.to_owned(),
        severity: Severity::Warn,
        title: format!(
            "{} ran against oracle `{name}`, which is {}; capped at tier {CAP}",
            checks.join(", "),
            state.as_str()
        ),
        detail: None,
        evidence: json!({"oracle": name, "state": state, "checks": checks}),
        action: ActionKind::VerifyOracle,
        fix: format!("Run `cupel oracle verify {name}`."),
    }
}

/// The finding kinds Cupel attaches to a run outside any check's manifest, described with check
/// `oracle` (ADR-091 §5).
pub fn finding_kinds() -> Vec<FindingKindSpec> {
    let Value::Object(evidence) = json!({
        "type": "object",
        "required": ["oracle", "state", "checks"],
        "properties": {
            "oracle": {"type": "string", "description": "The oracle's name."},
            "state": {
                "enum": ["unverified", "changed", "unreachable"],
                "description": "Why the oracle is not verified now (ADR-090 §5)."
            },
            "checks": {
                "type": "array", "items": {"type": "string"},
                "description": "The checks whose results it capped."
            }
        }
    }) else {
        unreachable!("the schema is an object")
    };
    vec![FindingKindSpec {
        kind: UNVERIFIED.to_owned(),
        action: ActionKind::VerifyOracle,
        summary: format!(
            "A check compared against an oracle that is not verified now; its result is capped \
             at tier {CAP}."
        ),
        evidence: Map::from_iter(evidence),
    }]
}

#[cfg(test)]
mod tests {
    use cupel_core::CheckId;
    use proptest::prelude::*;

    use super::*;
    use crate::render::fixture;

    const STATES: [OracleState; 4] = [
        OracleState::Verified,
        OracleState::Unverified,
        OracleState::Changed,
        OracleState::Unreachable,
    ];

    /// The fixture envelope with its results at `tiers`, the first two against `pg`, the third
    /// against `bc`, and no findings.
    fn against_oracles(tiers: [u8; 3]) -> Envelope {
        let mut envelope = fixture::envelope();
        let mut third = envelope.checks[0].clone();
        third.id = CheckId::from("gen.metamorphic");
        third.oracle = Some("bc".to_owned());
        envelope.checks.push(third);
        envelope.checks[1].oracle = Some("pg".to_owned());
        for (result, tier) in envelope.checks.iter_mut().zip(tiers) {
            result.tier = Tier::try_from(tier).unwrap();
        }
        envelope.findings.clear();
        envelope
    }

    fn states(pg: OracleState, bc: OracleState) -> BTreeMap<String, OracleState> {
        BTreeMap::from([("pg".to_owned(), pg), ("bc".to_owned(), bc)])
    }

    fn tiers(envelope: &Envelope) -> Vec<u8> {
        envelope
            .checks
            .iter()
            .map(|result| result.tier.value())
            .collect()
    }

    #[test]
    fn an_unverified_oracle_caps_its_results_and_gets_one_warn_finding() {
        let mut envelope = against_oracles([4, 5, 5]);
        capped(
            &mut envelope,
            &states(OracleState::Unverified, OracleState::Verified),
            "cupel check",
        );
        assert_eq!(tiers(&envelope), [3, 3, 5]);
        let finding = &envelope.findings[0];
        assert_eq!(
            (
                envelope.findings.len(),
                finding.kind.as_str(),
                finding.check.as_str()
            ),
            (1, UNVERIFIED, "oracle")
        );
        assert_eq!(
            Value::Object(finding.evidence.0.clone()),
            json!({"oracle": "pg", "state": "unverified", "checks": ["perf.bench", "tests.mutation"]})
        );
        assert_eq!(
            (
                finding.severity,
                finding.action.kind,
                envelope.summary.findings.warn
            ),
            (Severity::Warn, ActionKind::VerifyOracle, 1)
        );
        assert_eq!(envelope.total_findings, 1);
    }

    #[test]
    fn an_oracle_the_configuration_no_longer_defines_is_unreachable() {
        let mut envelope = against_oracles([6, 2, 4]);
        capped(&mut envelope, &BTreeMap::new(), "cupel check");
        assert_eq!(tiers(&envelope), [3, 2, 3]);
        let states: Vec<&Value> = envelope
            .findings
            .iter()
            .map(|finding| &finding.evidence.0["state"])
            .collect();
        assert_eq!(states, [&json!("unreachable"), &json!("unreachable")]);
    }

    proptest! {
        #[test]
        fn only_results_against_verified_oracles_keep_a_tier_above_3(
            given in proptest::array::uniform3(0u8..=6),
            pg in 0usize..4,
            bc in 0usize..4,
        ) {
            let mut envelope = against_oracles(given);
            capped(&mut envelope, &states(STATES[pg], STATES[bc]), "cupel check");
            let verified = [STATES[pg], STATES[pg], STATES[bc]].map(|s| s == OracleState::Verified);
            for ((result, before), verified) in envelope.checks.iter().zip(given).zip(verified) {
                let expected = if verified { before } else { before.min(CAP) };
                prop_assert_eq!(result.tier.value(), expected);
            }
            let unverified = [pg, bc].iter().filter(|s| STATES[**s] != OracleState::Verified).count();
            prop_assert_eq!(envelope.findings.len(), unverified);
        }
    }
}
