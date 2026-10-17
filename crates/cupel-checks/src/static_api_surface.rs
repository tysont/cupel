// ABOUTME: `static.api-surface`: diffs ApiSurface between a range's base and head by path and reports removed or
// ABOUTME: changed public items as `api.breaking` and added ones as `api.widened` (ADR-101).
use std::collections::BTreeMap;

use cupel_core::syntax::{ApiItem, ApiItemKind, ApiSurface};
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

const ID: &str = "static.api-surface";

/// The check has no settings; any key is a configuration error (ADR-101).
const SETTINGS: Limits = Limits::new(&[]);

/// One public item as a side of the diff shows it; inlined, as evidence schemas have no
/// `$defs` (ADR-043).
#[derive(Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(inline)]
struct Shown {
    /// `function`, `method`, `type`, `interface`, `constant`, `variable`, or `module`.
    kind: String,
    /// The normalized signature: parameter names are `_`, layout is gone.
    signature: String,
}

/// Evidence of `api.breaking` and `api.widened`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ApiChange {
    /// The item's fully qualified path.
    path: String,
    /// `removed` or `changed` (breaking); `added` or `extended` (widened).
    change: String,
    /// The items at this path at the base commit, in fact order.
    base: Vec<Shown>,
    /// The items at this path at head, in fact order.
    head: Vec<Shown>,
}

pub struct StaticApiSurface {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(StaticApiSurface::new())));

impl StaticApiSurface {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(0).expect("tier 0 exists"),
                cost: CostClass::Quick,
                needs: vec![FactKind::ApiSurface],
                wants: vec![],
                findings: vec![
                    kind::<ApiChange>(
                        "api.breaking",
                        ActionKind::Review,
                        "A public item of the base commit was removed (deleted, renamed, or made \
                         private) or lost a signature or kind callers may use.",
                    ),
                    kind::<ApiChange>(
                        "api.widened",
                        ActionKind::Review,
                        "The public API gained an item (added, renamed into, or made public) or \
                         a signature beside the ones it had.",
                    ),
                ],
                metrics: vec![
                    metric(
                        "breaking",
                        "Public paths the range removed or changed; the gate rule \
                         `static.api-surface = \"no-breaking\"` requires none reported.",
                    ),
                    metric("widened", "Public paths the range added or extended."),
                ],
                stability: Stability::Experimental,
                summary: "Diff of the public API against the range's base: removed and changed \
                          items are breaking, added ones widen it. Runs only with --changed or \
                          --base."
                    .to_owned(),
            },
        }
    }
}

impl Check for StaticApiSurface {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn base_needs(&self) -> &[FactKind] {
        &[FactKind::ApiSurface]
    }

    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        SETTINGS.plan_files(&self.manifest, ctx, scope)
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        SETTINGS.read(&self.manifest.id, &ctx.config)?;
        let head = Side::of(facts.get::<ApiSurface>()?);
        let base = Side::of(facts.base::<ApiSurface>()?);
        let commit = facts.range().map_or("", |range| range.commit.as_str());
        let mut findings = Vec::new();
        for path in &base.order {
            ctx.cancel.checkpoint()?;
            let (before, after) = (base.items(path), head.items(path));
            if let Some(change) = compared(before, after) {
                findings.push(self.finding(path, change, before, after, commit));
            }
        }
        let added: Vec<&str> = head
            .order
            .iter()
            .copied()
            .filter(|path| base.items(path).is_empty())
            .collect();
        for path in &added {
            findings.push(self.finding(path, Change::Added, &[], head.items(path), commit));
        }
        let breaking = findings
            .iter()
            .filter(|finding| finding.kind == "api.breaking")
            .count() as u64;
        let widened = findings.len() as u64 - breaking;
        let compared = base.order.len() + added.len();
        for finding in findings {
            sink.emit(Event::Finding(Box::new(finding)));
        }
        #[allow(
            clippy::cast_precision_loss,
            reason = "path counts stay far below 2^52, where f64 is exact"
        )]
        let (breaking_value, widened_value) = (breaking as f64, widened as f64);
        Ok(outcome::result(
            &self.manifest,
            outcome::status(breaking > 0),
            Some(Score {
                value: breaking_value,
                unit: "breaking".to_owned(),
                n: compared as u64,
            }),
            BTreeMap::from([
                ("breaking".to_owned(), breaking_value),
                ("widened".to_owned(), widened_value),
            ]),
            breaking + widened,
        ))
    }

    /// Breaking and widened paths add up across languages.
    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        sum(name, results)
    }

    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::summed(scores)
    }
}

/// One side's public items by path, each path's items in fact order, and the paths in the
/// order they first appear in the fact (which orders the findings).
struct Side<'a> {
    paths: BTreeMap<&'a str, Vec<&'a ApiItem>>,
    order: Vec<&'a str>,
}

impl<'a> Side<'a> {
    fn of(surface: &'a ApiSurface) -> Self {
        let mut paths: BTreeMap<&str, Vec<&ApiItem>> = BTreeMap::new();
        let mut order = Vec::new();
        for item in &surface.items {
            let items = paths.entry(&item.path).or_default();
            if items.is_empty() {
                order.push(item.path.as_str());
            }
            items.push(item);
        }
        Self { paths, order }
    }

    /// The items at `path`; none when the side lacks it.
    fn items(&self, path: &str) -> &[&'a ApiItem] {
        self.paths.get(path).map_or(&[], Vec::as_slice)
    }
}

/// How a path's items changed across the range.
#[derive(Clone, Copy)]
enum Change {
    Removed,
    Changed,
    Added,
    Extended,
}

impl Change {
    fn kind(self) -> &'static str {
        match self {
            Self::Removed | Self::Changed => "api.breaking",
            Self::Added | Self::Extended => "api.widened",
        }
    }

    fn word(self) -> &'static str {
        match self {
            Self::Removed => "removed",
            Self::Changed => "changed",
            Self::Added => "added",
            Self::Extended => "extended",
        }
    }
}

/// How a path present at the base changed at head: removed when it is gone, changed when an
/// item of the base is missing, extended when head only adds items; `None` when unchanged.
fn compared(before: &[&ApiItem], after: &[&ApiItem]) -> Option<Change> {
    if after.is_empty() {
        return Some(Change::Removed);
    }
    let mut remaining = shown(after);
    for item in shown(before) {
        match remaining.iter().position(|candidate| *candidate == item) {
            Some(index) => {
                remaining.remove(index);
            }
            None => return Some(Change::Changed),
        }
    }
    (!remaining.is_empty()).then_some(Change::Extended)
}

fn shown(items: &[&ApiItem]) -> Vec<Shown> {
    items
        .iter()
        .map(|item| Shown {
            kind: kind_name(item.kind),
            signature: item.signature.clone(),
        })
        .collect()
}

impl StaticApiSurface {
    /// The finding about `path`, located at its first head item, or its first base item when
    /// it is gone.
    fn finding(
        &self,
        path: &str,
        change: Change,
        before: &[&ApiItem],
        after: &[&ApiItem],
        commit: &str,
    ) -> Finding {
        let at = after.first().or(before.first()).copied();
        let kind = change.kind();
        let id = FindingId::for_subject(&self.manifest.id, kind, path);
        let fields = match evidence(&ApiChange {
            path: path.to_owned(),
            change: change.word().to_owned(),
            base: shown(before),
            head: shown(after),
        }) {
            Value::Object(fields) => fields,
            _ => Map::new(),
        };
        let (severity, title, summary) = match change {
            Change::Removed => (
                Severity::Warn,
                format!("public `{path}` was removed"),
                format!(
                    "Restore `{path}` (see it at {commit}), or keep the old name delegating to \
                     the new one; if the break is intended, say so in review and suppress this."
                ),
            ),
            Change::Changed => (
                Severity::Warn,
                format!("public `{path}` changed its signature"),
                format!(
                    "Keep the signature `{path}` had at {commit} beside the new one, or confirm \
                     every caller is updated and suppress this."
                ),
            ),
            Change::Added => (
                Severity::Info,
                format!("public `{path}` was added"),
                format!("Confirm `{path}` is meant to be public API before callers depend on it."),
            ),
            Change::Extended => (
                Severity::Info,
                format!("public `{path}` gained a signature"),
                format!(
                    "Confirm the new signature of `{path}` is meant to be public API before \
                     callers depend on it."
                ),
            ),
        };
        Finding {
            suppress: format!("cupel suppress {id} --reason '…'"),
            repro: format!("cupel check --only {} --base {commit}", self.manifest.id),
            id,
            check: self.manifest.id.clone(),
            kind: kind.to_owned(),
            severity,
            confidence: 1.0,
            tier: self.manifest.tier,
            symbol: None,
            location: at.map(location),
            title,
            detail: None,
            evidence: Evidence(fields),
            action: Action {
                kind: ActionKind::Review,
                priority: 2,
                summary,
                hints: Map::new(),
            },
            new_since_baseline: false,
            suppressed: false,
        }
    }
}

/// `kind` as the fact spells it.
fn kind_name(kind: ApiItemKind) -> String {
    match serde_json::to_value(kind) {
        Ok(Value::String(name)) => name,
        _ => String::new(),
    }
}

fn location(item: &ApiItem) -> Location {
    Location {
        file: item.file.clone(),
        line: item.span.line,
        col: Some(item.span.col),
        end_line: item.span.end_line,
        end_col: Some(item.span.end_col),
    }
}
