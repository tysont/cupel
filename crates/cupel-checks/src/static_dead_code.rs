// ABOUTME: `static.dead-code`: modules nothing imports and no root keeps alive, from Symbols, NormTree,
// ABOUTME: ModuleGraph, and Roots; reports their private symbols and exports (ADR-058, ADR-059).
use std::collections::{BTreeMap, BTreeSet, HashMap};

use cupel_core::syntax::{
    ImportTarget, ModuleGraph, NormTree, RootReason, Roots, SymbolDef, SymbolKey, SymbolKind,
    Symbols,
};
use cupel_core::{
    ActionKind, Check, CheckCtx, CheckId, CheckResult, CheckStatus, CostClass, Event, EventSink,
    FactKind, Facts, Finding, FindingKindSpec, Manifest, MetricSpec, Plan, Result, Scope, Score,
    Severity, Stability, SymbolHash, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::finding::SymbolFinding;
use crate::settings::Limits;

const ID: &str = "static.dead-code";

/// The check has no settings; any key is a configuration error.
const SETTINGS: Limits = Limits::new(&[]);

/// Confidence that an unreachable module's private symbol is dead (ADR-059).
const SYMBOL_CONFIDENCE: f64 = 0.8;
/// Confidence that an unimported export is dead: it may be a library's public API.
const EXPORT_CONFIDENCE: f64 = 0.4;
/// The same, when an import the resolver could not follow names the file.
const SUSPECT_SYMBOL_CONFIDENCE: f64 = 0.2;
const SUSPECT_EXPORT_CONFIDENCE: f64 = 0.1;

/// Characters that separate the segments of an import specifier, in any language.
const SPECIFIER_SEPARATORS: &[char] = &['/', '.', ':', '\\', '@'];

/// Evidence of `dead.symbol` and `dead.export`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Unreachable {
    /// The file that defines the symbol.
    file: String,
    /// The namespace of the module the file belongs to.
    module: String,
    /// Every file of that module; no file outside the module imports any of them.
    module_files: Vec<String>,
    /// Unresolved or external imports whose specifier names this file, which may load it.
    suspect_imports: Vec<String>,
}

pub struct StaticDeadCode {
    manifest: Manifest,
}

impl StaticDeadCode {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(0).expect("tier 0 exists"),
                cost: CostClass::Quick,
                needs: vec![
                    FactKind::Symbols,
                    FactKind::NormTree,
                    FactKind::ModuleGraph,
                    FactKind::Roots,
                ],
                wants: vec![],
                findings: vec![
                    kind(
                        "dead.symbol",
                        "A symbol in a module that nothing imports, that exports nothing, and \
                         that holds no entry point, test, or framework hook.",
                    ),
                    kind(
                        "dead.export",
                        "An exported symbol in a module that nothing in the repository imports \
                         and that holds no entry point, test, or framework hook (info: it may \
                         be a library's public API).",
                    ),
                ],
                metrics: vec![
                    metric(
                        "dead_symbols",
                        "The number of dead.symbol findings in scope.",
                    ),
                    metric(
                        "dead_exports",
                        "The number of dead.export findings in scope.",
                    ),
                ],
                stability: Stability::Experimental,
                summary: "Symbols and exports in modules that no import reaches and no entry \
                          point, test, or framework hook keeps alive."
                    .to_owned(),
            },
        }
    }
}

impl Check for StaticDeadCode {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        SETTINGS.read(&self.manifest.id, &ctx.config)?;
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
        SETTINGS.read(&self.manifest.id, &ctx.config)?;
        let symbols = facts.get::<Symbols>()?;
        let hashes: HashMap<&SymbolKey, &SymbolHash> = facts
            .get::<NormTree>()?
            .symbols
            .iter()
            .map(|entry| (&entry.symbol, &entry.hash))
            .collect();
        let reachability = Reachability::of(
            &symbols.symbols,
            facts.get::<Roots>()?,
            facts.get::<ModuleGraph>()?,
        );
        let mut in_scope = 0;
        let mut dead_symbols = 0;
        let mut findings = 0;
        let mut ordered: Vec<&SymbolDef> = symbols.symbols.iter().collect();
        ordered.sort_by_key(|symbol| symbol.key());
        for symbol in ordered {
            let key = symbol.key();
            if !ctx.scope.includes(&key) {
                continue;
            }
            ctx.cancel.checkpoint()?;
            in_scope += 1;
            let Some(verdict) = reachability.verdict(symbol) else {
                continue;
            };
            // A symbol without a structural hash cannot be identified; every language
            // producing NormTree hashes every symbol.
            let Some(hash) = hashes.get(&key) else {
                continue;
            };
            if verdict.kind == DeadKind::Symbol {
                dead_symbols += 1;
            }
            findings += 1;
            sink.emit(Event::Finding(Box::new(
                self.report(symbol, hash, &verdict),
            )));
        }
        Ok(CheckResult {
            id: self.manifest.id.clone(),
            status: if dead_symbols == 0 {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            tier: self.manifest.tier,
            cost: self.manifest.cost,
            score: Some(Score {
                value: f64::from(dead_symbols),
                unit: "dead_symbols".to_owned(),
                n: in_scope,
            }),
            delta: None,
            findings,
            duration_ms: 0,
            skipped: None,
            oracle: None,
        })
    }

    /// Dead symbols add up across languages (ADR-059).
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        let first = scores.first()?;
        Some(Score {
            value: scores.iter().map(|score| score.value).sum(),
            unit: first.unit.clone(),
            n: scores.iter().map(|score| score.n).sum(),
        })
    }
}

impl StaticDeadCode {
    fn report(&self, symbol: &SymbolDef, hash: &SymbolHash, verdict: &Verdict<'_>) -> Finding {
        let file = &symbol.file;
        let name = &symbol.name;
        let (kind, severity, title, detail) = match verdict.kind {
            DeadKind::Symbol => (
                "dead.symbol",
                Severity::Warn,
                format!(
                    "`{}` is unreachable: no import resolves to {file}",
                    symbol.display
                ),
                format!(
                    "No import in the repository resolves to {file} or another file of its \
                     module, and nothing in it is an entry point, test, framework hook, or \
                     export, so no root reaches `{name}`. Cupel sees imports between files, \
                     not calls, so this holds for the whole file; module-level code, a script \
                     run directly, or loading by computed name (reflection, `getattr`, \
                     string-keyed registries, dynamic imports) could still reach it.{}",
                    suspicion(verdict)
                ),
            ),
            DeadKind::Export => (
                "dead.export",
                Severity::Info,
                format!(
                    "`{}` is exported but no import resolves to {file}",
                    symbol.display
                ),
                format!(
                    "No import in the repository resolves to {file} or another file of its \
                     module, and nothing in it is an entry point, test, or framework hook. If \
                     `{name}` is part of a library's public API this is expected; in an \
                     application it is dead, along with whatever only it calls.{}",
                    suspicion(verdict)
                ),
            ),
        };
        let action = self
            .manifest
            .findings
            .iter()
            .find(|spec| spec.kind == kind)
            .map_or(ActionKind::Review, |spec| spec.action);
        let mut finding = SymbolFinding {
            check: &self.manifest.id,
            kind,
            symbol,
            hash,
            severity,
            tier: self.manifest.tier,
            title,
            evidence: Unreachable {
                file: file.clone(),
                module: verdict.module.to_owned(),
                module_files: verdict.files.iter().map(|f| (*f).to_owned()).collect(),
                suspect_imports: verdict.suspects.clone(),
            },
            action,
            summary: format!(
                "Delete `{name}` from {file} if nothing outside the repository uses it, or \
                 import {file} where `{name}` is meant to be called."
            ),
        }
        .finding();
        finding.confidence = verdict.confidence();
        finding.detail = Some(detail);
        finding
    }
}

fn suspicion(verdict: &Verdict<'_>) -> String {
    if verdict.suspects.is_empty() {
        String::new()
    } else {
        format!(
            " An import the resolver could not follow names this file ({}), so it may load it.",
            verdict.suspects.join(", ")
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeadKind {
    Symbol,
    Export,
}

/// Why a symbol is reported, and what the evidence says about it.
struct Verdict<'a> {
    kind: DeadKind,
    module: &'a str,
    files: &'a BTreeSet<&'a str>,
    suspects: Vec<String>,
}

impl Verdict<'_> {
    fn confidence(&self) -> f64 {
        match (self.kind, self.suspects.is_empty()) {
            (DeadKind::Symbol, true) => SYMBOL_CONFIDENCE,
            (DeadKind::Symbol, false) => SUSPECT_SYMBOL_CONFIDENCE,
            (DeadKind::Export, true) => EXPORT_CONFIDENCE,
            (DeadKind::Export, false) => SUSPECT_EXPORT_CONFIDENCE,
        }
    }
}

/// One module: the files that share a module namespace (ADR-058).
#[derive(Default)]
struct Module<'a> {
    files: BTreeSet<&'a str>,
    entered: bool,
    rooted: bool,
    exports: bool,
}

/// Which modules imports and roots reach, over every symbol and edge the facts carry.
struct Reachability<'a> {
    /// Each file's module namespace.
    module_of: HashMap<&'a str, &'a str>,
    modules: BTreeMap<&'a str, Module<'a>>,
    /// Symbols that are roots for being exported.
    exported: BTreeSet<SymbolKey>,
    /// Symbols nested inside another symbol of the same file.
    nested: BTreeSet<SymbolKey>,
    /// Specifiers of unresolved and external imports.
    unfollowed: Vec<&'a str>,
}

impl<'a> Reachability<'a> {
    fn of(symbols: &'a [SymbolDef], roots: &Roots, graph: &'a ModuleGraph) -> Self {
        let module_of = module_namespaces(symbols);
        let mut modules: BTreeMap<&str, Module<'_>> = BTreeMap::new();
        for (file, namespace) in &module_of {
            modules.entry(namespace).or_default().files.insert(file);
        }
        let by_key: HashMap<SymbolKey, &SymbolDef> = symbols
            .iter()
            .map(|symbol| (symbol.key(), symbol))
            .collect();
        let mut exported = BTreeSet::new();
        for root in &roots.roots {
            let Some(namespace) = by_key
                .get(&root.symbol)
                .and_then(|symbol| module_of.get(symbol.file.as_str()))
            else {
                continue;
            };
            let module = modules.entry(namespace).or_default();
            if root.reason == RootReason::Export {
                module.exports = true;
                exported.insert(root.symbol.clone());
            } else {
                module.rooted = true;
            }
        }
        let mut unfollowed = Vec::new();
        for edge in graph.roots.iter().flat_map(|root| &root.edges) {
            match &edge.target {
                ImportTarget::File { path } => {
                    let Some(target) = module_of.get(path.as_str()) else {
                        continue;
                    };
                    if module_of.get(edge.from.as_str()) != Some(target) {
                        modules.entry(target).or_default().entered = true;
                    }
                }
                ImportTarget::External { .. } | ImportTarget::Unresolved => {
                    unfollowed.push(edge.specifier.as_str());
                }
            }
        }
        Self {
            module_of,
            modules,
            exported,
            nested: nested(symbols),
            unfollowed,
        }
    }

    /// Whether `symbol` is reported, and as what; `None` when something may reach it.
    fn verdict(&self, symbol: &SymbolDef) -> Option<Verdict<'_>> {
        let key = symbol.key();
        if self.nested.contains(&key) {
            return None;
        }
        let (namespace, module) = self
            .module_of
            .get(symbol.file.as_str())
            .and_then(|namespace| Some((*namespace, self.modules.get(namespace)?)))?;
        if module.entered || module.rooted {
            return None;
        }
        let kind = if self.exported.contains(&key) {
            DeadKind::Export
        } else if module.exports {
            // An export may be a library's API, and may call this symbol (ADR-058).
            return None;
        } else {
            DeadKind::Symbol
        };
        Some(Verdict {
            kind,
            module: namespace,
            files: &module.files,
            suspects: self.suspects(&symbol.file),
        })
    }

    /// Unresolved and external specifiers one of whose segments is `file`'s stem.
    fn suspects(&self, file: &str) -> Vec<String> {
        let name = file.rsplit(['/', '\\']).next().unwrap_or(file);
        let stem = name.split_once('.').map_or(name, |(stem, _)| stem);
        let mut suspects: Vec<String> = self
            .unfollowed
            .iter()
            .filter(|specifier| {
                specifier
                    .split(SPECIFIER_SEPARATORS)
                    .any(|segment| segment == stem)
            })
            .map(|specifier| (*specifier).to_owned())
            .collect();
        suspects.sort();
        suspects.dedup();
        suspects
    }
}

/// Each file's module namespace: the shortest namespace among its symbols, a method's taken
/// without its last segment (the receiver or class).
fn module_namespaces(symbols: &[SymbolDef]) -> HashMap<&str, &str> {
    let mut modules: HashMap<&str, &str> = HashMap::new();
    for symbol in symbols {
        let namespace = module_namespace(symbol);
        modules
            .entry(symbol.file.as_str())
            .and_modify(|current| {
                if (namespace.len(), namespace) < (current.len(), *current) {
                    *current = namespace;
                }
            })
            .or_insert(namespace);
    }
    modules
}

fn module_namespace(symbol: &SymbolDef) -> &str {
    let namespace = symbol.namespace.as_str();
    if symbol.kind != SymbolKind::Method || namespace.is_empty() {
        return namespace;
    }
    match separator(symbol) {
        Some(separator) if !separator.is_empty() => namespace
            .rsplit_once(separator)
            .map_or("", |(parent, _)| parent),
        _ => namespace,
    }
}

/// The separator the language joins namespaces with: `display` is `namespace`, the
/// separator, and `name`.
fn separator(symbol: &SymbolDef) -> Option<&str> {
    symbol
        .display
        .strip_prefix(symbol.namespace.as_str())?
        .strip_suffix(symbol.name.as_str())
}

/// Symbols whose span lies inside another symbol's span in the same file.
fn nested(symbols: &[SymbolDef]) -> BTreeSet<SymbolKey> {
    let mut by_file: HashMap<&str, Vec<&SymbolDef>> = HashMap::new();
    for symbol in symbols {
        by_file
            .entry(symbol.file.as_str())
            .or_default()
            .push(symbol);
    }
    let start = |symbol: &SymbolDef| (symbol.span.line, symbol.span.col);
    let end = |symbol: &SymbolDef| (symbol.span.end_line, symbol.span.end_col);
    let mut nested = BTreeSet::new();
    for file in by_file.values() {
        for inner in file {
            let inside = file.iter().any(|outer| {
                start(outer) != start(inner)
                    && start(outer) <= start(inner)
                    && end(inner) <= end(outer)
            });
            if inside {
                nested.insert(inner.key());
            }
        }
    }
    nested
}

/// A finding kind with the `Unreachable` evidence schema (ADR-012).
fn kind(kind: &str, summary: &str) -> FindingKindSpec {
    let schema = match schemars::schema_for!(Unreachable).to_value() {
        Value::Object(schema) => schema,
        _ => Map::new(),
    };
    FindingKindSpec {
        kind: kind.to_owned(),
        action: ActionKind::Review,
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
