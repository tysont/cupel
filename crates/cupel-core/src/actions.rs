// ABOUTME: `next_actions` (design §7.1, ADR-061): findings ranked by severity, novelty, and tier, plus
// ABOUTME: rerun/install/configure actions for gate rules and checks that could not run; de-duplicated, capped.
use std::cmp::Reverse;
use std::collections::HashSet;

use crate::envelope::{
    CheckResult, Envelope, GateRule, NextAction, RuleOutcome, SkipReason, Skipped,
};
use crate::finding::{ActionKind, Finding, Severity};
use crate::grade::Tier;

/// The most actions an envelope lists (ADR-061 §5).
pub const NEXT_ACTIONS: usize = 10;

/// Ranking bands, most urgent first (ADR-061 §2).
const FAIL: u8 = 0;
const GATE: u8 = 1;
const WARN: u8 = 2;
const SKIPPED: u8 = 3;
const INFO: u8 = 4;

/// One possible action with what it ranks and de-duplicates by.
struct Candidate {
    band: u8,
    new: bool,
    tier: Tier,
    key: (ActionKind, String),
    action: NextAction,
}

/// What to do next about a graded envelope, most urgent first. `rerun` is the command that
/// reruns the run's own verb and scope, such as `cupel check --changed`; a rerun action appends
/// `--only <check>` to it.
pub fn next_actions(envelope: &Envelope, rerun: &str) -> Vec<NextAction> {
    let mut candidates: Vec<Candidate> = envelope
        .findings
        .iter()
        .filter(|finding| !finding.suppressed)
        .map(finding_action)
        .collect();
    if let Some(gate) = &envelope.summary.gate {
        candidates.extend(
            gate.rules
                .iter()
                .filter_map(|rule| rule_action(rule, rerun, &envelope.checks)),
        );
    }
    candidates.extend(
        envelope
            .checks
            .iter()
            .filter_map(|check| skip_action(check, rerun)),
    );
    // A stable sort keeps envelope order, then gate-rule and `checks[]` order, within ties.
    candidates.sort_by_key(|candidate| (candidate.band, !candidate.new, Reverse(candidate.tier)));
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|candidate| seen.insert(candidate.key.clone()))
        .take(NEXT_ACTIONS)
        .zip(1..)
        .map(|(candidate, priority)| NextAction {
            priority,
            ..candidate.action
        })
        .collect()
}

fn finding_action(finding: &Finding) -> Candidate {
    let band = match finding.severity {
        Severity::Fail => FAIL,
        Severity::Warn => WARN,
        Severity::Info => INFO,
    };
    let place = if let Some(location) = &finding.location {
        format!("{}:{}-{}", location.file, location.line, location.end_line)
    } else if let Some(symbol) = &finding.symbol {
        symbol.hash.to_string()
    } else {
        finding.id.to_string()
    };
    Candidate {
        band,
        new: finding.new_since_baseline,
        tier: finding.tier,
        key: (finding.action.kind, place),
        action: NextAction {
            priority: 0,
            finding: Some(finding.id.clone()),
            kind: finding.action.kind,
            summary: finding.action.summary.clone(),
            command: None,
        },
    }
}

/// A rerun for a rule whose check did not run, or a configuration fix for one Cupel cannot
/// evaluate. A check skipped because it needs a range reruns as its skip says (ADR-056).
fn rule_action(rule: &GateRule, rerun: &str, checks: &[CheckResult]) -> Option<Candidate> {
    let check = rule_check(&rule.rule);
    let command = checks
        .iter()
        .filter(|result| result.id.as_str() == check)
        .filter_map(|result| result.skipped.as_ref())
        .find(|skipped| skipped.reason == SkipReason::NeedsRange)
        .and_then(|skipped| skipped.see.clone())
        .unwrap_or_else(|| format!("{rerun} --only {check}"));
    match rule.result {
        RuleOutcome::Pass | RuleOutcome::Fail => None,
        RuleOutcome::Skipped => Some(other(
            GATE,
            ActionKind::Rerun,
            check,
            format!(
                "Run `{check}`: the gate rule `{}` was not evaluated in this run.",
                rule.rule
            ),
            Some(command),
        )),
        RuleOutcome::Unsupported => Some(other(
            GATE,
            ActionKind::Configure,
            check,
            format!(
                "Remove the gate rule `{}`: Cupel cannot evaluate it yet.",
                rule.rule
            ),
            None,
        )),
    }
}

/// An action for a check skipped for a reason the user can change.
fn skip_action(check: &CheckResult, rerun: &str) -> Option<Candidate> {
    let skipped = check.skipped.as_ref()?;
    let id = check.id.as_str();
    let lang = skipped
        .lang
        .as_ref()
        .map(|lang| format!(" on {lang}"))
        .unwrap_or_default();
    match skipped.reason {
        SkipReason::Timeout | SkipReason::Budget => Some(other(
            SKIPPED,
            ActionKind::Rerun,
            id,
            format!(
                "Run `{id}`, which the {} cut short{lang}.",
                reason_name(skipped)
            ),
            Some(format!("{rerun} --only {id}")),
        )),
        SkipReason::Unavailable => {
            let tool = skipped
                .tool
                .as_ref()
                .map_or_else(|| "its tool".to_owned(), |tool| format!("`{tool}`"));
            let mut candidate = other(
                SKIPPED,
                ActionKind::InstallTool,
                id,
                format!("Install {tool} so `{id}` can run{lang}."),
                Some("cupel doctor".to_owned()),
            );
            candidate.key.1 = format!("{id} {tool}");
            Some(candidate)
        }
        SkipReason::Unconfigured => Some(other(
            SKIPPED,
            ActionKind::Configure,
            id,
            format!("Configure what `{id}` needs{lang}."),
            Some("cupel doctor".to_owned()),
        )),
        // The command is the skip's own `see`: the check compares a range's base and head,
        // so rerunning the same tree scope would skip it again (ADR-056).
        SkipReason::NeedsRange => Some(other(
            SKIPPED,
            ActionKind::Rerun,
            id,
            format!(
                "Run `{id}` over a range (`--changed` or `--base`): it compares base and head."
            ),
            Some(
                skipped
                    .see
                    .clone()
                    .unwrap_or_else(|| format!("cupel check --changed --only {id}")),
            ),
        )),
        SkipReason::Unsupported => None,
    }
}

fn reason_name(skipped: &Skipped) -> &'static str {
    match skipped.reason {
        SkipReason::Budget => "budget",
        _ => "timeout",
    }
}

/// An action that is not about a finding, keyed by its kind and check.
fn other(
    band: u8,
    kind: ActionKind,
    check: &str,
    summary: String,
    command: Option<String>,
) -> Candidate {
    Candidate {
        band,
        new: false,
        tier: Tier::try_from(0).expect("tier 0 exists"),
        key: (kind, check.to_owned()),
        action: NextAction {
            priority: 0,
            finding: None,
            kind,
            summary,
            command,
        },
    }
}

/// The check ID a gate rule as written names: the first two segments of its key
/// (`static.crap.max < 30` → `static.crap`; ADR-017 §3).
fn rule_check(rule: &str) -> &str {
    let key = rule.split_whitespace().next().unwrap_or_default();
    match key.match_indices('.').nth(1) {
        Some((end, _)) => &key[..end],
        None => key,
    }
}
