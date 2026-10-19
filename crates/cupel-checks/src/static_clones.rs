// ABOUTME: `static.clones`: near-duplicate functions within a language family, from NormTree subtree
// ABOUTME: fingerprints through MinHash and LSH to exact Jaccard, reported as pairs and clusters (ADR-050).
use std::collections::BTreeMap;

use cupel_core::config::{ConfigError, Table};
use cupel_core::similarity::{Fingerprints, Similar, combine, mix, search};
use cupel_core::syntax::{NormNode, NormTree, SymbolDef, Symbols};
use cupel_core::{
    Action, ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Event, EventSink,
    Evidence, FactKind, Facts, Finding, FindingId, FindingKindSpec, Location, Manifest,
    Measurement, MetricPer, MetricSpec, Plan, Result, Scope, Score, SettingSpec, Severity,
    SiteHash, Stability, Symbol, SymbolHash, Target, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::manifest::{evidence, kind, metric, ranked, setting};
use crate::metrics::{largest, sum, trees_by_symbol};
use crate::outcome;

const ID: &str = "static.clones";
const PAIR: &str = "clone.pair";
const CLUSTER: &str = "clone.cluster";

/// The settings and their defaults (design §6.7).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Settings {
    /// The least Jaccard similarity of two fingerprint sets that makes a clone.
    threshold: f64,
    /// The fewest lines a symbol must span to be compared.
    min_lines: u32,
    /// The fewest `NormTree` nodes a symbol must have to be compared.
    min_nodes: u32,
}

const DEFAULTS: Settings = Settings {
    threshold: 0.82,
    min_lines: 4,
    min_nodes: 20,
};

impl Settings {
    /// Every setting as the manifest declares it, with its default (ADR-111).
    fn specs() -> Vec<SettingSpec> {
        vec![
            setting(
                "threshold",
                Value::from(DEFAULTS.threshold),
                "The least Jaccard similarity of two symbols' fingerprint sets that makes a \
                 clone, greater than 0 and at most 1.",
            ),
            setting(
                "min_lines",
                Value::from(DEFAULTS.min_lines),
                "The fewest lines a symbol must span to be compared.",
            ),
            setting(
                "min_nodes",
                Value::from(DEFAULTS.min_nodes),
                "The fewest normalized-tree nodes a symbol must have to be compared.",
            ),
        ]
    }

    /// The configured settings over the defaults; unknown keys, a threshold outside (0, 1],
    /// and counts that are not non-negative integers are configuration errors.
    fn read(check: &CheckId, config: &Table) -> Result<Self> {
        let keys: Vec<String> = Self::specs().into_iter().map(|spec| spec.name).collect();
        let error = |key: &str, reason: String| ConfigError::CheckSetting {
            check: check.to_string(),
            key: key.to_owned(),
            reason,
            valid: keys.clone(),
        };
        if let Some(unknown) = config.keys().find(|key| !keys.contains(key)) {
            return Err(error(unknown, "not a setting of this check".to_owned()).into());
        }
        let threshold = match config.get("threshold") {
            None => DEFAULTS.threshold,
            Some(value) => value
                .as_f64()
                .filter(|threshold| *threshold > 0.0 && *threshold <= 1.0)
                .ok_or_else(|| {
                    error(
                        "threshold",
                        format!("expected a number greater than 0 and at most 1, got {value}"),
                    )
                })?,
        };
        let count = |key: &str, default: u32| match config.get(key) {
            None => Ok(default),
            Some(value) => value
                .as_u64()
                .and_then(|count| u32::try_from(count).ok())
                .ok_or_else(|| error(key, format!("expected a non-negative integer, got {value}"))),
        };
        Ok(Self {
            threshold,
            min_lines: count("min_lines", DEFAULTS.min_lines)?,
            min_nodes: count("min_nodes", DEFAULTS.min_nodes)?,
        })
    }
}

/// One side of a clone: the symbol, where it is, and how big it is.
#[derive(Serialize, JsonSchema)]
#[schemars(inline)]
#[serde(deny_unknown_fields)]
struct Member {
    /// The symbol's display name.
    symbol: String,
    /// The symbol's structural hash (`sh_…`).
    hash: String,
    lang: String,
    file: String,
    line: u32,
    col: u32,
    end_line: u32,
    end_col: u32,
    /// Lines the symbol spans.
    lines: u32,
    /// Nodes of its normalized tree.
    nodes: u32,
}

/// Evidence of `clone.pair`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ClonePair {
    /// Jaccard similarity of the two symbols' subtree fingerprint sets, to three decimals.
    jaccard: f64,
    /// 2 when the normalized trees are identical (type 1 and type 2 clones: only names,
    /// literals, types, layout, or comments differ); 3 when they differ (statements added,
    /// removed, or changed).
    clone_type: u8,
    threshold: f64,
    left: Member,
    right: Member,
}

/// Evidence of `clone.cluster`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CloneCluster {
    /// Every symbol in the cluster, in file and line order.
    members: Vec<Member>,
    /// Pairs of members at least `threshold` similar; at most n·(n−1)/2.
    pairs: u32,
    min_jaccard: f64,
    max_jaccard: f64,
    threshold: f64,
}

pub struct StaticClones {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(StaticClones::new())));

impl StaticClones {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(0).expect("tier 0 exists"),
                cost: CostClass::Quick,
                needs: vec![FactKind::Symbols, FactKind::NormTree],
                wants: vec![],
                findings: finding_kinds(),
                metrics: metrics(),
                settings: Settings::specs(),
                stability: Stability::Experimental,
                summary: "Type 1, 2, and 3 clones: functions whose normalized syntax trees share \
                          most subtree fingerprints, compared within a language family. \
                          Limitation: type 1 is reported as type 2, because NormTree keeps no \
                          identifier or literal text, so two functions that differ only in \
                          literals (two lookup tables of the same shape) are reported as clones \
                          (ADR-050; a raw-token digest is a P3 follow-up)."
                    .to_owned(),
            },
        }
    }
}

/// Every symbol of the family long and large enough to compare, with its fingerprints, in
/// symbol-key order.
fn comparable<'a>(facts: &'a Facts, settings: &Settings) -> Result<Vec<Compared<'a>>> {
    let trees = trees_by_symbol(facts.family::<NormTree>()?);
    let mut compared: Vec<Compared<'a>> = facts
        .family::<Symbols>()?
        .into_iter()
        .flat_map(|fact| &fact.symbols)
        .filter(|symbol| lines(symbol) >= settings.min_lines)
        .filter_map(|symbol| {
            let (hash, root) = trees.get(&symbol.key())?;
            Some(Compared {
                symbol,
                hash,
                root,
                prints: Fingerprints::of(root),
            })
        })
        .filter(|compared| compared.prints.nodes >= settings.min_nodes)
        .collect();
    compared.sort_by_key(|compared| compared.symbol.key());
    Ok(compared)
}

/// The finding kinds this check reports, with their actions and evidence.
fn finding_kinds() -> Vec<FindingKindSpec> {
    vec![
        kind::<ClonePair>(
            PAIR,
            ActionKind::RefactorExtract,
            "Two symbols whose normalized trees are at least `threshold` similar \
             and similar to no third symbol.",
        ),
        kind::<CloneCluster>(
            CLUSTER,
            ActionKind::RefactorExtract,
            "Three or more symbols connected by pairs at least `threshold` similar.",
        ),
    ]
}

/// The metrics this check reports: gate keys, then what `survey` ranks.
fn metrics() -> Vec<MetricSpec> {
    vec![
        metric("pairs", "Pairs of symbols at least `threshold` similar."),
        metric("clusters", "Groups of three or more similar symbols."),
        metric("max_jaccard", "The highest similarity of any pair."),
        ranked(
            "cluster_size",
            MetricPer::Group,
            "Symbols in each clone group: 2 for a pair, more for a cluster.",
        ),
    ]
}

impl Check for StaticClones {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        Settings::read(&self.manifest.id, &ctx.config)?;
        Ok(outcome::plan_symbols(&self.manifest, scope))
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        let settings = Settings::read(&self.manifest.id, &ctx.config)?;
        let compared = comparable(facts, &settings)?;
        ctx.cancel.checkpoint()?;
        let sets: Vec<Fingerprints> = compared.iter().map(|c| c.prints.clone()).collect();
        let pairs: Vec<Similar> = search(&sets, settings.threshold)
            .pairs
            .into_iter()
            .filter(|pair| !nested(compared[pair.left].symbol, compared[pair.right].symbol))
            .collect();
        ctx.cancel.checkpoint()?;
        let report = Report {
            check: self,
            compared: &compared,
            settings,
        };
        let groups = groups(compared.len(), &pairs);
        let mut findings = 0;
        let mut reported = Reported::default();
        for group in &groups {
            let in_scope = group
                .members
                .iter()
                .any(|index| ctx.scope.includes(&compared[*index].symbol.key()));
            if in_scope {
                findings += 1;
                sink.emit(Event::Finding(Box::new(report.finding(group))));
                sink.measure(report.size(group));
                reported.include(group);
            }
        }
        Ok(outcome::result(
            &self.manifest,
            outcome::status(findings > 0),
            Some(Score {
                value: f64::from(u32::try_from(pairs.len()).unwrap_or(u32::MAX)),
                unit: "pairs".to_owned(),
                n: compared.len() as u64,
            }),
            reported.metrics(),
            findings,
        ))
    }

    /// Pairs add up across families.
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::summed(scores)
    }

    /// Pairs and clusters add up across families; `max_jaccard` is the largest (ADR-063).
    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        match name {
            "max_jaccard" => largest(name, results),
            _ => sum(name, results),
        }
    }

    /// JavaScript and TypeScript symbols are compared with each other (design §8.4.3).
    fn spans_family(&self) -> bool {
        true
    }
}

/// A symbol large enough to compare, with its normalized tree and fingerprints.
struct Compared<'a> {
    symbol: &'a SymbolDef,
    hash: &'a SymbolHash,
    root: &'a NormNode,
    prints: Fingerprints,
}

fn lines(symbol: &SymbolDef) -> u32 {
    symbol.span.end_line.saturating_sub(symbol.span.line) + 1
}

/// Whether one symbol lies inside the other, as a closure inside its function does: their
/// trees overlap by construction, so they are never reported as clones of each other.
fn nested(a: &SymbolDef, b: &SymbolDef) -> bool {
    let start = |s: &SymbolDef| (s.span.line, s.span.col);
    let end = |s: &SymbolDef| (s.span.end_line, s.span.end_col);
    a.file == b.file
        && ((start(a) <= start(b) && end(b) <= end(a))
            || (start(b) <= start(a) && end(a) <= end(b)))
}

/// Symbols connected by similar pairs, with the pairs among them.
struct Group {
    /// Indexes into the compared symbols, ascending.
    members: Vec<usize>,
    pairs: Vec<Similar>,
}

/// The connected components of the similarity graph with at least two members, ordered by
/// their first member.
fn groups(count: usize, pairs: &[Similar]) -> Vec<Group> {
    let mut parent: Vec<usize> = (0..count).collect();
    for pair in pairs {
        let (a, b) = (root(&mut parent, pair.left), root(&mut parent, pair.right));
        parent[a.max(b)] = a.min(b);
    }
    let mut groups: BTreeMap<usize, Group> = BTreeMap::new();
    for pair in pairs {
        let at = root(&mut parent, pair.left);
        groups
            .entry(at)
            .or_insert_with(|| Group {
                members: Vec::new(),
                pairs: Vec::new(),
            })
            .pairs
            .push(*pair);
    }
    for index in 0..count {
        let at = root(&mut parent, index);
        if let Some(group) = groups.get_mut(&at) {
            group.members.push(index);
        }
    }
    groups.into_values().collect()
}

/// The representative of `index`'s component in a union-find forest, halving paths on the way.
fn root(parent: &mut [usize], mut index: usize) -> usize {
    while parent[index] != index {
        parent[index] = parent[parent[index]];
        index = parent[index];
    }
    index
}

/// The `scope` metrics over the groups reported (ADR-063).
#[derive(Default)]
struct Reported {
    pairs: usize,
    clusters: usize,
    max_jaccard: f64,
}

impl Reported {
    fn include(&mut self, group: &Group) {
        self.pairs += group.pairs.len();
        if group.members.len() > 2 {
            self.clusters += 1;
        }
        for pair in &group.pairs {
            self.max_jaccard = self.max_jaccard.max(rounded(pair.jaccard));
        }
    }

    #[allow(
        clippy::cast_precision_loss,
        reason = "pair and cluster counts stay far below 2^52, where f64 is exact"
    )]
    fn metrics(&self) -> BTreeMap<String, f64> {
        BTreeMap::from([
            ("pairs".to_owned(), self.pairs as f64),
            ("clusters".to_owned(), self.clusters as f64),
            ("max_jaccard".to_owned(), self.max_jaccard),
        ])
    }
}

/// Turns groups into findings and measurements.
struct Report<'a> {
    check: &'a StaticClones,
    compared: &'a [Compared<'a>],
    settings: Settings,
}

impl Report<'_> {
    fn finding(&self, group: &Group) -> Finding {
        let first = &self.compared[group.members[0]];
        let others = group.members[1..]
            .iter()
            .map(|index| &self.compared[*index]);
        let (kind, title, summary, evidence) = if let [pair] = group.pairs.as_slice() {
            self.pair(pair)
        } else {
            self.cluster(group)
        };
        let site = others.fold(0, |site, other| combine(site, hash_bits(other.hash)));
        let site: SiteHash = format!("{:016x}", mix(site))
            .parse()
            .expect("sixteen hex digits make a valid site hash");
        let id = FindingId::compute(&self.check.manifest.id, kind, first.hash, Some(&site));
        Finding {
            suppress: format!("cupel suppress {id} --reason '…'"),
            repro: format!("cupel check --only {ID} --symbols {}", first.hash),
            id,
            check: self.check.manifest.id.clone(),
            kind: kind.to_owned(),
            severity: Severity::Warn,
            confidence: 1.0,
            tier: self.check.manifest.tier,
            symbol: Some(Symbol {
                display: first.symbol.display.clone(),
                hash: first.hash.clone(),
                lang: first.symbol.lang.clone(),
            }),
            location: Some(location(first.symbol)),
            title,
            detail: None,
            evidence: Evidence(object(&evidence)),
            action: Action {
                kind: ActionKind::RefactorExtract,
                priority: 2,
                summary,
                hints: Map::new(),
            },
            new_since_baseline: false,
            suppressed: false,
        }
    }

    /// How many symbols `group` holds, for `survey`'s largest clone clusters (ADR-062).
    #[allow(
        clippy::cast_precision_loss,
        reason = "group sizes stay far below 2^52, where f64 is exact"
    )]
    fn size(&self, group: &Group) -> Measurement {
        let first = &self.compared[group.members[0]];
        let others = group.members.len() - 1;
        Measurement {
            metric: "cluster_size".to_owned(),
            value: group.members.len() as f64,
            target: Target {
                display: format!(
                    "{} and {others} other{}",
                    first.symbol.display,
                    if others == 1 { "" } else { "s" }
                ),
                symbol: None,
                location: Some(location(first.symbol)),
            },
        }
    }

    fn pair(&self, pair: &Similar) -> (&'static str, String, String, Value) {
        let (left, right) = (&self.compared[pair.left], &self.compared[pair.right]);
        let clone_type = if left.root == right.root { 2 } else { 3 };
        let jaccard = rounded(pair.jaccard);
        let title = format!(
            "`{}` and `{}` ({}:{}) are type-{clone_type} clones (jaccard {jaccard:.2})",
            left.symbol.display, right.symbol.display, right.symbol.file, right.symbol.span.line
        );
        let summary = format!(
            "Extract what `{}` and `{}` share into one function and call it from both.",
            left.symbol.name, right.symbol.name
        );
        let evidence = evidence(&ClonePair {
            jaccard,
            clone_type,
            threshold: self.settings.threshold,
            left: member(left),
            right: member(right),
        });
        (PAIR, title, summary, evidence)
    }

    fn cluster(&self, group: &Group) -> (&'static str, String, String, Value) {
        let first = &self.compared[group.members[0]];
        let similarity = group.pairs.iter().map(|pair| pair.jaccard);
        let min_jaccard = rounded(similarity.clone().fold(1.0, f64::min));
        let max_jaccard = rounded(similarity.fold(0.0, f64::max));
        let title = format!(
            "`{}` and {} other symbols are clones of one another (jaccard {min_jaccard:.2}–\
             {max_jaccard:.2})",
            first.symbol.display,
            group.members.len() - 1
        );
        let summary = format!(
            "Extract the structure `{}` shares with {} other symbols into one function and call \
             it from each.",
            first.symbol.name,
            group.members.len() - 1
        );
        let evidence = evidence(&CloneCluster {
            members: group
                .members
                .iter()
                .map(|index| member(&self.compared[*index]))
                .collect(),
            pairs: u32::try_from(group.pairs.len()).unwrap_or(u32::MAX),
            min_jaccard,
            max_jaccard,
            threshold: self.settings.threshold,
        });
        (CLUSTER, title, summary, evidence)
    }
}

fn member(compared: &Compared<'_>) -> Member {
    let symbol = compared.symbol;
    Member {
        symbol: symbol.display.clone(),
        hash: compared.hash.to_string(),
        lang: symbol.lang.to_string(),
        file: symbol.file.clone(),
        line: symbol.span.line,
        col: symbol.span.col,
        end_line: symbol.span.end_line,
        end_col: symbol.span.end_col,
        lines: lines(symbol),
        nodes: compared.prints.nodes,
    }
}

fn location(symbol: &SymbolDef) -> Location {
    crate::finding::located(&symbol.file, &symbol.span)
}

/// The 64 bits a structural hash's hex digits spell.
fn hash_bits(hash: &SymbolHash) -> u64 {
    u64::from_str_radix(hash.as_str().trim_start_matches("sh_"), 16).unwrap_or(0)
}

/// `value` to three decimals, so evidence is stable across platforms' last-bit differences.
fn rounded(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn object(value: &Value) -> Map<String, Value> {
    match value {
        Value::Object(fields) => fields.clone(),
        _ => Map::new(),
    }
}
