// ABOUTME: `static.dead-code`: modules nothing imports and no root keeps alive, then symbols nothing names,
// ABOUTME: from Symbols, NormTree, ModuleGraph, Roots, and References (ADR-058, ADR-059, ADR-135).
use std::collections::{BTreeMap, BTreeSet, HashMap};

use cupel_core::syntax::{
    DynamicAccess, DynamicReach, ImportEdge, ImportTarget, ModuleGraph, NormTree, Reference,
    References, RootReason, Roots, SymbolDef, SymbolKey, SymbolKind, Symbols,
};
use cupel_core::{
    ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Event, EventSink, FactKind,
    Facts, Finding, FindingKindSpec, Manifest, MetricSpec, Plan, Result, Scope, Score, Severity,
    Stability, SymbolHash, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::finding::SymbolFinding;
use crate::manifest::{kind, metric};
use crate::metrics::{hashes_by_symbol, sum};
use crate::outcome;
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
/// Confidence that a symbol nothing names is dead (ADR-135).
const UNREFERENCED_CONFIDENCE: f64 = 0.6;
/// The same, when a name computed at run time could reach it.
const DYNAMIC_CONFIDENCE: f64 = 0.3;
/// The most dynamic accesses a finding's evidence lists.
const DYNAMIC_LISTED: usize = 10;

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
    /// What found it unreachable: `module` when no import or root reaches its module,
    /// `references` when its module is reachable but nothing names the symbol.
    reach: Reach,
    /// Where the language computes a name at run time in a way that could reach the symbol,
    /// as `file:line` (at most ten); such access lowers the confidence.
    dynamic: Vec<String>,
}

/// How a finding's symbol was found unreachable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(inline)]
enum Reach {
    Module,
    References,
}

pub struct StaticDeadCode {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(StaticDeadCode::new())));

impl StaticDeadCode {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 2,
                tier: Tier::try_from(0).expect("tier 0 exists"),
                cost: CostClass::Quick,
                needs: vec![
                    FactKind::Symbols,
                    FactKind::NormTree,
                    FactKind::ModuleGraph,
                    FactKind::Roots,
                    FactKind::References,
                ],
                wants: vec![],
                findings: finding_kinds(),
                metrics: metrics(),
                settings: SETTINGS.specs(),
                stability: Stability::Experimental,
                summary: "Symbols and exports in modules that no import reaches and no entry \
                          point, test, or framework hook keeps alive, then symbols that nothing \
                          outside their own body names. Limitation: references match by name, \
                          not by type, so any same-named reference keeps a symbol alive, and a \
                          function only dead code calls is reported once that code is gone; an \
                          export of an unimported module is only `dead.export` (info, low \
                          confidence) (ADR-058, ADR-135)."
                    .to_owned(),
            },
        }
    }
}

/// The finding kinds this check reports, with their actions and evidence.
fn finding_kinds() -> Vec<FindingKindSpec> {
    vec![
        kind::<Unreachable>(
            "dead.symbol",
            ActionKind::Review,
            "A symbol in a module that nothing imports, that exports nothing, and \
             that holds no entry point, test, or framework hook; or a symbol that is \
             no root and that no reference outside its own body names.",
        ),
        kind::<Unreachable>(
            "dead.export",
            ActionKind::Review,
            "An exported symbol in a module that nothing in the repository imports \
             and that holds no entry point, test, or framework hook (info: it may \
             be a library's public API).",
        ),
    ]
}

/// The metrics this check reports: gate keys, then what `survey` ranks.
fn metrics() -> Vec<MetricSpec> {
    vec![
        metric(
            "dead_symbols",
            "The number of dead.symbol findings in scope.",
        ),
        metric(
            "dead_exports",
            "The number of dead.export findings in scope.",
        ),
    ]
}

impl Check for StaticDeadCode {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        SETTINGS.read(&self.manifest.id, &ctx.config)?;
        Ok(outcome::plan_symbols(&self.manifest, scope))
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
        let hashes = hashes_by_symbol(facts.get::<NormTree>()?);
        let roots = facts.get::<Roots>()?;
        let references = facts.get::<References>()?;
        let reachability = Reachability::of(
            &symbols.symbols,
            roots,
            imports(facts.get::<ModuleGraph>()?, references),
        );
        let usage = Usage::of(references, roots);
        let mut tally = Tally::default();
        let mut ordered: Vec<&SymbolDef> = symbols.symbols.iter().collect();
        ordered.sort_by_key(|symbol| symbol.key());
        for symbol in ordered {
            let key = symbol.key();
            if !ctx.scope.includes(&key) {
                continue;
            }
            ctx.cancel.checkpoint()?;
            tally.in_scope += 1;
            // A symbol without a structural hash cannot be identified; every language
            // producing NormTree hashes every symbol.
            if let (Some(verdict), Some(hash)) =
                (reachability.judge(symbol, &usage), hashes.get(&key))
            {
                tally.count(verdict.kind);
                sink.emit(Event::Finding(Box::new(
                    self.report(symbol, hash, &verdict),
                )));
            }
        }
        Ok(outcome::result(
            &self.manifest,
            outcome::status(tally.dead_symbols > 0),
            Some(Score {
                value: f64::from(tally.dead_symbols),
                unit: "dead_symbols".to_owned(),
                n: tally.in_scope,
            }),
            BTreeMap::from([
                ("dead_symbols".to_owned(), f64::from(tally.dead_symbols)),
                ("dead_exports".to_owned(), f64::from(tally.dead_exports)),
            ]),
            tally.findings(),
        ))
    }

    /// Dead symbols add up across languages (ADR-059).
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::summed(scores)
    }

    /// Both metrics are counts, so they add up across languages (ADR-063).
    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        sum(name, results)
    }
}

impl StaticDeadCode {
    fn report(&self, symbol: &SymbolDef, hash: &SymbolHash, verdict: &Verdict<'_>) -> Finding {
        let (kind, severity, title, detail) = wording(symbol, verdict);
        let evidence = Unreachable {
            file: symbol.file.clone(),
            module: verdict.module.to_owned(),
            module_files: verdict.files.iter().map(|f| (*f).to_owned()).collect(),
            suspect_imports: verdict.suspects.clone(),
            reach: verdict.reach,
            dynamic: verdict.dynamic.clone(),
        };
        let mut finding = SymbolFinding::new(&self.manifest, kind, symbol, hash, evidence)
            .described(title, action(symbol, verdict))
            .at(severity)
            .finding();
        finding.confidence = verdict.confidence();
        finding.detail = Some(detail);
        finding
    }
}

/// How many symbols were in scope and how many of each kind were reported.
#[derive(Default)]
struct Tally {
    in_scope: u64,
    dead_symbols: u32,
    dead_exports: u32,
}

impl Tally {
    fn count(&mut self, kind: DeadKind) {
        match kind {
            DeadKind::Symbol => self.dead_symbols += 1,
            DeadKind::Export => self.dead_exports += 1,
        }
    }

    fn findings(&self) -> u64 {
        u64::from(self.dead_symbols) + u64::from(self.dead_exports)
    }
}

/// A finding's kind, severity, title, and detail.
fn wording(symbol: &SymbolDef, verdict: &Verdict<'_>) -> (&'static str, Severity, String, String) {
    let file = &symbol.file;
    let name = &symbol.name;
    match (verdict.kind, verdict.reach) {
        (DeadKind::Symbol, Reach::References) => (
            "dead.symbol",
            Severity::Warn,
            format!("`{}` is never referenced", symbol.display),
            format!(
                "Nothing in the repository's files names `{name}` outside its own body: no \
                 call, value, import, string, or attribute refers to it, and it is not an \
                 entry point, test, framework hook, or export. Cupel matches references by \
                 name, not by type, so any reference with this name would keep it; code \
                 generated at build time, a file in another language, or a framework that \
                 finds it by naming convention could still reach it.{}",
                dynamism(verdict)
            ),
        ),
        (DeadKind::Symbol, Reach::Module) => (
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
        (DeadKind::Export, _) => (
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
    }
}

/// What to do about the symbol: delete it, or use it where it was meant to be.
fn action(symbol: &SymbolDef, verdict: &Verdict<'_>) -> String {
    let file = &symbol.file;
    let name = &symbol.name;
    match verdict.reach {
        Reach::Module => format!(
            "Delete `{name}` from {file} if nothing outside the repository uses it, or import \
             {file} where `{name}` is meant to be called."
        ),
        Reach::References => format!(
            "Delete `{name}` from {file} if nothing outside the repository uses it, or call it \
             where it is meant to be used."
        ),
    }
}

fn dynamism(verdict: &Verdict<'_>) -> String {
    verdict
        .dynamic
        .first()
        .map(|first| {
            format!(
                " The language computes names at run time where it could reach it, first at \
                 {first} (the evidence lists up to ten such places)."
            )
        })
        .unwrap_or_default()
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
    reach: Reach,
    module: &'a str,
    files: &'a BTreeSet<&'a str>,
    suspects: Vec<String>,
    dynamic: Vec<String>,
}

impl Verdict<'_> {
    fn confidence(&self) -> f64 {
        if self.reach == Reach::References {
            return if self.dynamic.is_empty() {
                UNREFERENCED_CONFIDENCE
            } else {
                DYNAMIC_CONFIDENCE
            };
        }
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

/// The scope's import edges (`ModuleGraph`) and the language's (`References`), so an importer
/// outside a narrowed scope enters the module it imports (ADR-144).
fn imports<'a>(graph: &'a ModuleGraph, references: &'a References) -> Vec<&'a ImportEdge> {
    graph
        .roots
        .iter()
        .flat_map(|root| &root.edges)
        .chain(&references.imports)
        .collect()
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
    fn of(symbols: &'a [SymbolDef], roots: &Roots, edges: Vec<&'a ImportEdge>) -> Self {
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
        for edge in edges {
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
            reach: Reach::Module,
            module: namespace,
            files: &module.files,
            suspects: self.suspects(&symbol.file),
            dynamic: Vec::new(),
        })
    }

    /// Whether `symbol` is reported, by the module pass or else by references.
    fn judge(&self, symbol: &SymbolDef, usage: &Usage<'_>) -> Option<Verdict<'_>> {
        self.verdict(symbol)
            .or_else(|| self.unreferenced(symbol, usage))
    }

    /// Whether `symbol`, which the module pass keeps, is reported because nothing names it
    /// (ADR-135). Each early return is a rule that suppresses a finding.
    fn unreferenced(&self, symbol: &SymbolDef, usage: &Usage<'_>) -> Option<Verdict<'_>> {
        let key = symbol.key();
        if usage.rooted.contains(&key) || self.nested.contains(&key) || constructor(symbol) {
            return None;
        }
        if usage.referenced(symbol) {
            return None;
        }
        let (namespace, module) = self
            .module_of
            .get(symbol.file.as_str())
            .and_then(|namespace| Some((*namespace, self.modules.get(namespace)?)))?;
        Some(Verdict {
            kind: DeadKind::Symbol,
            reach: Reach::References,
            module: namespace,
            files: &module.files,
            suspects: Vec::new(),
            dynamic: usage.reaching(symbol),
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

/// Which symbols are named where, which are roots, and where names are computed at run time.
struct Usage<'a> {
    /// Resolved references by the symbols they resolve to.
    resolved: HashMap<&'a SymbolKey, Vec<&'a Reference>>,
    /// Unresolved references by name.
    unresolved: HashMap<&'a str, Vec<&'a Reference>>,
    /// Symbols that are roots for any reason.
    rooted: BTreeSet<SymbolKey>,
    dynamic: &'a [DynamicAccess],
}

impl<'a> Usage<'a> {
    fn of(references: &'a References, roots: &Roots) -> Self {
        let mut resolved: HashMap<&SymbolKey, Vec<&Reference>> = HashMap::new();
        let mut unresolved: HashMap<&str, Vec<&Reference>> = HashMap::new();
        for reference in &references.references {
            let targets = reference.target.symbols();
            if targets.is_empty() {
                unresolved
                    .entry(reference.name.as_str())
                    .or_default()
                    .push(reference);
            }
            for target in targets {
                resolved.entry(target).or_default().push(reference);
            }
        }
        Self {
            resolved,
            unresolved,
            rooted: roots.roots.iter().map(|root| root.symbol.clone()).collect(),
            dynamic: &references.dynamic,
        }
    }

    /// Whether a reference outside `symbol`'s body resolves to it, or is unresolved and has its
    /// name. A reference inside its body is recursion.
    fn referenced(&self, symbol: &SymbolDef) -> bool {
        let own = symbol.body.unwrap_or(symbol.span);
        let outside = |reference: &&Reference| {
            reference.file != symbol.file || !own.contains(&reference.span)
        };
        let key = symbol.key();
        self.resolved
            .get(&key)
            .is_some_and(|references| references.iter().any(outside))
            || self
                .unresolved
                .get(symbol.name.as_str())
                .is_some_and(|references| references.iter().any(outside))
    }

    /// The dynamic accesses that could reach `symbol`, as `file:line`: every lookup by name,
    /// and every computed member access when it is a method.
    fn reaching(&self, symbol: &SymbolDef) -> Vec<String> {
        self.dynamic
            .iter()
            .filter(|access| {
                access.reach == DynamicReach::Names || symbol.kind == SymbolKind::Method
            })
            .take(DYNAMIC_LISTED)
            .map(|access| format!("{}:{}", access.file, access.span.line))
            .collect()
    }
}

/// A method named like the type that encloses it: a constructor, which a private declaration
/// may exist only to forbid instantiating (ADR-135).
fn constructor(symbol: &SymbolDef) -> bool {
    symbol.kind == SymbolKind::Method
        && separator(symbol).is_some_and(|separator| {
            !separator.is_empty()
                && symbol.namespace.rsplit(separator).next() == Some(symbol.name.as_str())
        })
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
