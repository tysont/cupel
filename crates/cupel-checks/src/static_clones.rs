// ABOUTME: `static.clones`: near-duplicate functions within a language family, from NormTree subtree
// ABOUTME: fingerprints through MinHash and LSH to exact Jaccard, reported as pairs and clusters (ADR-050).
mod similarity;

use std::collections::{BTreeMap, HashMap};

use cupel_core::config::{ConfigError, Table};
use cupel_core::syntax::{NormNode, NormTree, SymbolDef, SymbolKey, Symbols};
use cupel_core::{
    Action, ActionKind, Check, CheckCtx, CheckId, CheckResult, CheckStatus, CostClass, Event,
    EventSink, Evidence, FactKind, Facts, Finding, FindingId, FindingKindSpec, Location, Manifest,
    MetricSpec, Plan, Result, Scope, Score, Severity, SiteHash, Stability, Symbol, SymbolHash,
    Tier,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Map, Value};

use similarity::{Fingerprints, Similar, combine, mix, search};

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

const KEYS: [&str; 3] = ["threshold", "min_lines", "min_nodes"];

impl Settings {
    /// The configured settings over the defaults; unknown keys, a threshold outside (0, 1],
    /// and counts that are not non-negative integers are configuration errors.
    fn read(check: &CheckId, config: &Table) -> Result<Self> {
        let error = |key: &str, reason: String| ConfigError::CheckSetting {
            check: check.to_string(),
            key: key.to_owned(),
            reason,
            valid: KEYS.iter().map(|key| (*key).to_owned()).collect(),
        };
        if let Some(unknown) = config.keys().find(|key| !KEYS.contains(&key.as_str())) {
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
                findings: vec![
                    kind::<ClonePair>(
                        PAIR,
                        "Two symbols whose normalized trees are at least `threshold` similar \
                         and similar to no third symbol.",
                    ),
                    kind::<CloneCluster>(
                        CLUSTER,
                        "Three or more symbols connected by pairs at least `threshold` similar.",
                    ),
                ],
                metrics: vec![
                    metric("pairs", "Pairs of symbols at least `threshold` similar."),
                    metric("clusters", "Groups of three or more similar symbols."),
                    metric("max_jaccard", "The highest similarity of any pair."),
                ],
                stability: Stability::Experimental,
                summary: "Type 1, 2, and 3 clones: functions whose normalized syntax trees share \
                          most subtree fingerprints, compared within a language family."
                    .to_owned(),
            },
        }
    }
}

impl Check for StaticClones {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        Settings::read(&self.manifest.id, &ctx.config)?;
        let targets = scope.symbols.as_ref().map_or(0, |keys| keys.len() as u64);
        Ok(Plan::estimate(self.manifest.cost, targets))
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        let settings = Settings::read(&self.manifest.id, &ctx.config)?;
        let trees: HashMap<&SymbolKey, (&SymbolHash, &NormNode)> = facts
            .family::<NormTree>()?
            .into_iter()
            .flat_map(|fact| &fact.symbols)
            .map(|entry| (&entry.symbol, (&entry.hash, &entry.root)))
            .collect();
        let mut compared: Vec<Compared<'_>> = facts
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
        for group in &groups {
            let in_scope = group
                .members
                .iter()
                .any(|index| ctx.scope.includes(&compared[*index].symbol.key()));
            if in_scope {
                findings += 1;
                sink.emit(Event::Finding(Box::new(report.finding(group))));
            }
        }
        Ok(CheckResult {
            id: self.manifest.id.clone(),
            status: if findings == 0 {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            tier: self.manifest.tier,
            cost: self.manifest.cost,
            score: Some(Score {
                value: f64::from(u32::try_from(pairs.len()).unwrap_or(u32::MAX)),
                unit: "pairs".to_owned(),
                n: compared.len() as u64,
            }),
            delta: None,
            findings,
            duration_ms: 0,
            skipped: None,
            oracle: None,
        })
    }

    /// Pairs add up across families.
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        let first = scores.first()?;
        Some(Score {
            value: scores.iter().map(|score| score.value).sum(),
            unit: first.unit.clone(),
            n: scores.iter().map(|score| score.n).sum(),
        })
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

/// Turns groups into findings.
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
        let evidence = to_value(&ClonePair {
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
        let evidence = to_value(&CloneCluster {
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
    Location {
        file: symbol.file.clone(),
        line: symbol.span.line,
        col: Some(symbol.span.col),
        end_line: symbol.span.end_line,
        end_col: Some(symbol.span.end_col),
    }
}

/// The 64 bits a structural hash's hex digits spell.
fn hash_bits(hash: &SymbolHash) -> u64 {
    u64::from_str_radix(hash.as_str().trim_start_matches("sh_"), 16).unwrap_or(0)
}

/// `value` to three decimals, so evidence is stable across platforms' last-bit differences.
fn rounded(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn to_value<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

fn object(value: &Value) -> Map<String, Value> {
    match value {
        Value::Object(fields) => fields.clone(),
        _ => Map::new(),
    }
}

/// A finding kind mapped to `refactor_extract`, whose evidence is `T`'s JSON Schema (ADR-012).
fn kind<T: JsonSchema>(kind: &str, summary: &str) -> FindingKindSpec {
    let schema = match schemars::schema_for!(T).to_value() {
        Value::Object(schema) => schema,
        _ => Map::new(),
    };
    FindingKindSpec {
        kind: kind.to_owned(),
        action: ActionKind::RefactorExtract,
        summary: summary.to_owned(),
        evidence: schema,
    }
}

fn metric(name: &str, summary: &str) -> MetricSpec {
    MetricSpec {
        name: name.to_owned(),
        summary: summary.to_owned(),
    }
}
