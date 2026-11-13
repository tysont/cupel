// ABOUTME: `static.api-surface`: diffs ApiSurface between a range's base and head by path and reports removed or
// ABOUTME: changed public items as `api.breaking` and added ones as `api.widened` (ADR-101, ADR-174).
use std::collections::BTreeMap;

use cupel_core::hint::LOCATION_COMMIT;
use cupel_core::syntax::{ApiItem, ApiSurface, EdgeKind, ImportEdge, ImportTarget, ModuleGraph};
use cupel_core::{
    Action, ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Event, EventSink,
    Evidence, FactKind, Facts, Finding, FindingId, Location, Manifest, Plan, Result, Scope, Score,
    Severity, Stability, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::finding::located;
use crate::manifest::{evidence, kind, metric, spelling};
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
        let mut manifest = Manifest {
            id: CheckId::from(ID),
            version: 1,
            tier: Tier::try_from(0).expect("tier 0 exists"),
            cost: CostClass::Quick,
            needs: vec![FactKind::ApiSurface],
            wants: vec![FactKind::ModuleGraph],
            findings: Vec::new(),
            metrics: Vec::new(),
            settings: SETTINGS.specs(),
            stability: Stability::Experimental,
            summary: "Diff of the public API against the range's base: removed and changed items \
                      are breaking, added ones widen it. Runs only with --changed or --base."
                .to_owned(),
        };
        for (name, summary) in KINDS {
            manifest
                .findings
                .push(kind::<ApiChange>(name, ActionKind::Review, summary));
        }
        for (name, summary) in METRICS {
            manifest.metrics.push(metric(name, summary));
        }
        Self { manifest }
    }
}

/// `api.breaking` and `api.widened`, whose evidence is the same, with what each means.
const KINDS: [(&str, &str); 2] = [
    (
        "api.breaking",
        "A public item of the base commit was removed (deleted, renamed, or made private) or lost \
         a signature or kind callers may use.",
    ),
    (
        "api.widened",
        "The public API gained an item (added, renamed into, or made public), a signature beside \
         the ones it had, or trailing parameters a caller may leave out.",
    ),
];

/// The gate metrics, with what each counts.
const METRICS: [(&str, &str); 2] = [
    (
        "breaking",
        "Public paths the range removed or changed; the gate rule `static.api-surface = \
         \"no-breaking\"` requires none reported.",
    ),
    ("widened", "Public paths the range added or extended."),
];

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
        let reexports = Reexports::of(facts.get::<ModuleGraph>().ok());
        let mut findings = Vec::new();
        for path in &base.order {
            ctx.cancel.checkpoint()?;
            let Some(after) = reexports.at_head(path, base.items(path), &head) else {
                continue;
            };
            if let Some(change) = compared(base.items(path), &after) {
                findings.push(self.finding(path, change, base.items(path), &after, commit));
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

    /// The items defined in `file`.
    fn in_file(&self, file: &str) -> Vec<&'a ApiItem> {
        self.paths
            .values()
            .flatten()
            .copied()
            .filter(|item| item.file == file)
            .collect()
    }
}

/// The head's re-exports (`export … from`, `pub use`) by the file that makes them, so a path
/// whose item moved behind one is compared with what it re-exports (ADR-174).
struct Reexports<'a> {
    by_file: BTreeMap<&'a str, Vec<&'a ImportEdge>>,
}

impl<'a> Reexports<'a> {
    fn of(graph: Option<&'a ModuleGraph>) -> Self {
        let mut by_file: BTreeMap<&str, Vec<&ImportEdge>> = BTreeMap::new();
        let edges = graph.into_iter().flat_map(|graph| &graph.roots);
        for edge in edges.flat_map(|root| &root.edges) {
            if edge.edge_kind == EdgeKind::ReExport {
                by_file.entry(edge.from.as_str()).or_default().push(edge);
            }
        }
        Self { by_file }
    }

    /// The head items at `path`, whose base items are `before`: its own, else those a
    /// re-export of its file names (none when nothing re-exports it); `None` when a re-export
    /// names it from a file the head surface does not hold, which the range left unchanged.
    fn at_head(
        &self,
        path: &str,
        before: &[&ApiItem],
        head: &Side<'a>,
    ) -> Option<Vec<&'a ApiItem>> {
        let own = head.items(path);
        if !own.is_empty() {
            return Some(own.to_vec());
        }
        let edges = before
            .first()
            .and_then(|item| self.by_file.get(item.file.as_str()));
        let segments: Vec<&str> = path.split('.').collect();
        edges
            .into_iter()
            .flatten()
            .find_map(|edge| through(edge, &segments, head))
            .map_or(Some(Vec::new()), |found| match found {
                Reexported::Items(items) => Some(items),
                Reexported::Unread => None,
            })
    }
}

/// What a re-export gives a path: the head items it names, or a name from a file the head
/// surface does not hold.
enum Reexported<'a> {
    Items(Vec<&'a ApiItem>),
    Unread,
}

/// What `edge` re-exports at the path of `segments`, for the first segment it exports: the head
/// items of its file at the same path from the name it re-exports; `None` when it gives none.
fn through<'a>(edge: &ImportEdge, segments: &[&str], head: &Side<'a>) -> Option<Reexported<'a>> {
    let ImportTarget::File { path: file } = &edge.target else {
        return None;
    };
    let in_file = head.in_file(file);
    segments.iter().enumerate().find_map(|(at, segment)| {
        let name = exported(edge, segment)?;
        if in_file.is_empty() && !edge.names.is_empty() {
            return Some(Reexported::Unread);
        }
        let suffix = [&[name][..], &segments[at + 1..]].concat().join(".");
        let items: Vec<&ApiItem> = in_file
            .iter()
            .copied()
            .filter(|item| item.path == suffix || item.path.ends_with(&format!(".{suffix}")))
            .collect();
        (!items.is_empty()).then_some(Reexported::Items(items))
    })
}

/// The name `edge` re-exports as `local`: itself, the name an alias gives it, or any name for
/// `export *`; `None` when the edge does not export it.
fn exported<'e>(edge: &'e ImportEdge, local: &'e str) -> Option<&'e str> {
    if edge.names.is_empty() {
        return Some(local);
    }
    if let Some(alias) = edge.aliases.iter().find(|alias| alias.local == local) {
        return Some(&alias.name);
    }
    let aliased = edge.aliases.iter().any(|alias| alias.name == local);
    (edge.names.iter().any(|name| name == local) && !aliased).then_some(local)
}

/// How a path's items changed across the range.
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
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
}

/// How a path present at the base changed at head: removed when it is gone, changed when an
/// item of the base is missing, extended when head only adds items or parameters a caller may
/// leave out (ADR-174); `None` when unchanged.
fn compared(before: &[&ApiItem], after: &[&ApiItem]) -> Option<Change> {
    if after.is_empty() {
        return Some(Change::Removed);
    }
    let mut remaining = shown(after);
    let mut extended = false;
    for item in shown(before) {
        let same = remaining.iter().position(|candidate| *candidate == item);
        let index = same.or_else(|| {
            extended = true;
            remaining.iter().position(|candidate| {
                candidate.kind == item.kind && extends(&item.signature, &candidate.signature)
            })
        });
        match index {
            Some(index) => {
                remaining.remove(index);
            }
            None => return Some(Change::Changed),
        }
    }
    (extended || !remaining.is_empty()).then_some(Change::Extended)
}

/// Whether `after` is `before` with trailing parameters a caller may leave out: each optional
/// (`_?`), defaulted (`= …`), or variadic (`...`, `*`), and the rest of the signature the same.
fn extends(before: &str, after: &str) -> bool {
    let (Some((had, rest)), Some((has, now))) = (parameters(before), parameters(after)) else {
        return false;
    };
    rest == now
        && has.len() > had.len()
        && has.starts_with(&had)
        && has[had.len()..]
            .iter()
            .all(|parameter| omittable(parameter))
}

/// The parameters of a signature's first parenthesized list, split at its top-level commas,
/// and what follows the list; `None` without one.
fn parameters(signature: &str) -> Option<(Vec<&str>, &str)> {
    let mut found = Vec::new();
    let mut start = None;
    for (at, c, depth) in nesting(signature) {
        match (c, depth, start) {
            ('(', 0, None) => start = Some(at + 1),
            (',', 1, Some(from)) => {
                found.push(signature[from..at].trim());
                start = Some(at + 1);
            }
            (')', 0, Some(from)) => {
                found.push(signature[from..at].trim());
                found.retain(|parameter| !parameter.is_empty());
                return Some((found, &signature[at + 1..]));
            }
            _ => {}
        }
    }
    None
}

/// Whether a caller may leave `parameter` out: optional, variadic, or defaulted, with a `=` at
/// its top level that is not part of `=>`, `==`, `!=`, `<=`, or `>=`.
fn omittable(parameter: &str) -> bool {
    parameter.starts_with("_?")
        || parameter.starts_with("...")
        || parameter.starts_with('*')
        || nesting(parameter).any(|(at, c, depth)| {
            c == '='
                && depth == 0
                && !parameter[at + 1..].starts_with(['>', '='])
                && !parameter[..at].ends_with(['=', '!', '<', '>'])
        })
}

/// Each character of `text` outside quotes, with its byte offset and its bracket depth: an
/// opening bracket at the depth it opens from, a closing one at the depth it returns to. The
/// `>` of `=>` and `->` is no bracket.
fn nesting(text: &str) -> impl Iterator<Item = (usize, char, usize)> + '_ {
    let (mut depth, mut quote, mut previous) = (0_usize, None, ' ');
    text.char_indices().filter_map(move |(at, c)| {
        let escaped = previous == '\\';
        previous = c;
        match (quote, c) {
            (Some(q), _) if c == q && !escaped => {
                quote = None;
                None
            }
            (Some(_), _) => None,
            (None, '"' | '\'' | '`') => {
                quote = Some(c);
                None
            }
            (None, '(' | '[' | '{' | '<') => {
                depth += 1;
                Some((at, c, depth - 1))
            }
            (None, '>') if text[..at].ends_with(['=', '-']) => Some((at, c, depth)),
            (None, ')' | ']' | '}' | '>') => {
                depth = depth.saturating_sub(1);
                Some((at, c, depth))
            }
            _ => Some((at, c, depth)),
        }
    })
}

fn shown(items: &[&ApiItem]) -> Vec<Shown> {
    items
        .iter()
        .map(|item| Shown {
            kind: spelling(&item.kind),
            signature: item.signature.clone(),
        })
        .collect()
}

impl StaticApiSurface {
    /// The finding about `path`, located at its first head item, or its first base item when
    /// it is gone (then hinted as located at the base commit).
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
            change: spelling(&change),
            base: shown(before),
            head: shown(after),
        }) {
            Value::Object(fields) => fields,
            _ => Map::new(),
        };
        let (severity, title, summary) = said(change, path, commit);
        let mut hints = Map::new();
        if after.is_empty() {
            hints.insert(LOCATION_COMMIT.to_owned(), Value::String(commit.to_owned()));
        }
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
                hints,
            },
            new_since_baseline: false,
            suppressed: false,
        }
    }
}

/// A finding's severity, title, and action summary for how `path` changed.
fn said(change: Change, path: &str, commit: &str) -> (Severity, String, String) {
    match change {
        Change::Removed => (
            Severity::Warn,
            format!("public `{path}` was removed"),
            format!(
                "Restore `{path}` (see it at {commit}), or keep the old name delegating to the \
                 new one; if the break is intended, say so in review and suppress this."
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
    }
}

fn location(item: &ApiItem) -> Location {
    located(&item.file, &item.span)
}
