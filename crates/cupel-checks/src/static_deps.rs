// ABOUTME: `static.deps`: file-level import cycles (Tarjan SCC), Martin's coupling, instability,
// ABOUTME: abstractness, and distance, and layering rules from config, over ModuleGraph and ApiSurface.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use cupel_core::config::{ConfigError, Table};
use cupel_core::syntax::{ApiItemKind, ApiSurface, EdgeKind, ImportTarget, ModuleGraph};
use cupel_core::{
    Action, ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Event, EventSink,
    Evidence, FactKind, Facts, Finding, FindingId, FindingKindSpec, Location, Manifest,
    Measurement, MetricPer, MetricSpec, Plan, Result, Scope, Score, SettingSpec, Severity,
    Stability, Target, Tier,
};
use globset::{GlobBuilder, GlobMatcher};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::manifest::{evidence, kind, metric, ranked, setting};
use crate::metrics::{largest, sum};
use crate::outcome;

const ID: &str = "static.deps";

/// `dep.unstable-abstract` fires when A + I − 1 exceeds this (ADR-052).
const DISTANCE_MAX: f64 = 0.7;

/// The settings this check accepts, with their defaults (ADR-052, ADR-053, ADR-111).
fn settings() -> Vec<SettingSpec> {
    vec![
        setting(
            "distance_max",
            Value::from(DISTANCE_MAX),
            "The most A + I − 1 an abstract module may reach before dep.unstable-abstract, from 0 \
             to 1.",
        ),
        setting(
            "layers",
            Value::Null,
            "Layers as order (names, lowest first) and map (a glob per layer); a module importing \
             a higher layer is dep.layer-violation. No layering rule when unset.",
        ),
    ]
}

/// Evidence of `dep.cycle`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ImportCycle {
    /// The shortest cycle through the component's smallest module, in import order; the last
    /// module imports the first.
    cycle: Vec<String>,
    /// Every module of the strongly connected component, sorted.
    modules: Vec<String>,
    /// How many modules the component has.
    size: usize,
}

/// Evidence of `dep.layer-violation`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct LayerViolation {
    /// The importing module.
    from: String,
    /// The imported module, in a higher layer.
    to: String,
    from_layer: String,
    to_layer: String,
    /// The first violating import as written.
    specifier: String,
    /// The configured layers, lowest first.
    order: Vec<String>,
}

/// Evidence of `dep.unstable-abstract`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct UnstableAbstract {
    module: String,
    /// Ca: modules in the graph that import this one.
    afferent: u32,
    /// Ce: modules in the graph this one imports.
    efferent: u32,
    /// Distinct external packages imported; not part of Ce.
    external: u32,
    /// Imports that resolve to nothing; not part of Ce.
    unresolved: u32,
    /// I = Ce / (Ca + Ce).
    instability: f64,
    /// A = interfaces / (types + interfaces) among the module's public items.
    abstractness: f64,
    /// D = |A + I − 1|.
    distance: f64,
    distance_max: f64,
}

pub struct StaticDeps {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(StaticDeps::new())));

impl StaticDeps {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(0).expect("tier 0 exists"),
                cost: CostClass::Quick,
                needs: vec![FactKind::ModuleGraph, FactKind::ApiSurface],
                wants: vec![],
                findings: finding_kinds(),
                metrics: metrics(),
                settings: settings(),
                stability: Stability::Experimental,
                summary: "Module graph: import cycles, afferent and efferent coupling, \
                          instability, abstractness, distance from the main sequence, and \
                          layering rules from configuration."
                    .to_owned(),
            },
        }
    }
}

/// The finding kinds this check reports, with their actions and evidence.
fn finding_kinds() -> Vec<FindingKindSpec> {
    vec![
        kind::<ImportCycle>(
            "dep.cycle",
            ActionKind::BreakCycle,
            "Modules that import each other, directly or through others.",
        ),
        kind::<LayerViolation>(
            "dep.layer-violation",
            ActionKind::Review,
            "A module imports a module in a higher layer than its own \
             ([checks.\"static.deps\".layers]).",
        ),
        kind::<UnstableAbstract>(
            "dep.unstable-abstract",
            ActionKind::Review,
            "An abstract module that little depends on: A + I − 1 exceeds \
             distance_max.",
        ),
    ]
}

/// The metrics this check reports: gate keys, then what `survey` ranks.
fn metrics() -> Vec<MetricSpec> {
    vec![
        metric("cycles", "Import cycles (strongly connected components)."),
        metric(
            "max_distance",
            "The largest distance from the main sequence of any module that \
             declares a type and has a coupling.",
        ),
        metric("violations", "Imports that break a layering rule."),
        ranked(
            "distance",
            MetricPer::Module,
            "Distance from the main sequence per module that declares a type and \
             has a coupling.",
        ),
        ranked(
            "coupling",
            MetricPer::Module,
            "Afferent plus efferent coupling per module.",
        ),
        ranked(
            "cycle_size",
            MetricPer::Group,
            "Modules in each import cycle.",
        ),
    ]
}

impl Check for StaticDeps {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        Settings::read(&self.manifest.id, &ctx.config)?;
        Ok(outcome::plan_files(&self.manifest, scope))
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        let settings = Settings::read(&self.manifest.id, &ctx.config)?;
        let graph = Graph::of(facts.get::<ModuleGraph>()?);
        let types = TypeCounts::of(facts.get::<ApiSurface>()?);
        ctx.cancel.checkpoint()?;
        let mut findings = Vec::new();
        let components = graph.cycles();
        for members in &components {
            let (finding, size) = self.cycle(&graph, members);
            findings.push(finding);
            sink.measure(size);
        }
        let mut violations = 0;
        if let Some(layers) = &settings.layers {
            let found = self.violations(&graph, layers);
            violations = found.len();
            findings.extend(found);
        }
        ctx.cancel.checkpoint()?;
        let mut max_distance: f64 = 0.0;
        for module in &graph.modules {
            if let Some(distance) = measure_module(&graph, &types, module, sink) {
                max_distance = max_distance.max(distance);
            }
            if let Some(finding) = self.unstable_abstract(&graph, &types, module, &settings) {
                findings.push(finding);
            }
        }
        let count = findings.len() as u64;
        for finding in findings {
            sink.emit(Event::Finding(Box::new(finding)));
        }
        Ok(outcome::result(
            &self.manifest,
            outcome::status(count > 0),
            Some(Score {
                value: f64::from(u32::try_from(components.len()).unwrap_or(u32::MAX)),
                unit: "cycles".to_owned(),
                n: graph.modules.len() as u64,
            }),
            BTreeMap::from([
                ("cycles".to_owned(), as_value(components.len())),
                ("max_distance".to_owned(), max_distance),
                ("violations".to_owned(), as_value(violations)),
            ]),
            count,
        ))
    }

    /// Cycles are counts, so per-language scores add up (ADR-052).
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::summed(scores)
    }

    /// Cycles and violations add up across languages; `max_distance` is the largest (ADR-063).
    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        match name {
            "max_distance" => largest(name, results),
            _ => sum(name, results),
        }
    }
}

/// Reports `module`'s coupling and, where defined, its distance from the main sequence for
/// `survey`'s rankings (ADR-062); the distance, rounded as in evidence.
fn measure_module(
    graph: &Graph,
    types: &TypeCounts,
    module: &str,
    sink: &mut dyn EventSink,
) -> Option<f64> {
    let coupling = graph.coupling(module);
    let target = Target {
        display: module.to_owned(),
        symbol: None,
        location: Some(Location {
            file: module.to_owned(),
            line: 1,
            col: None,
            end_line: 1,
            end_col: None,
        }),
    };
    sink.measure(Measurement {
        metric: "coupling".to_owned(),
        value: f64::from(coupling.afferent + coupling.efferent),
        target: target.clone(),
    });
    let distance = rounded(Martin::of(&coupling, types.module(module))?.distance);
    sink.measure(Measurement {
        metric: "distance".to_owned(),
        value: distance,
        target,
    });
    Some(distance)
}

#[allow(
    clippy::cast_precision_loss,
    reason = "cycle and violation counts stay far below 2^52, where f64 is exact"
)]
fn as_value(n: usize) -> f64 {
    n as f64
}

impl StaticDeps {
    /// The cycle's finding, and its size for `survey`'s rankings (ADR-062).
    fn cycle(&self, graph: &Graph, members: &[String]) -> (Finding, Measurement) {
        let cycle = graph.shortest_cycle(members);
        let line = graph.line(&cycle[0], &cycle[1 % cycle.len()]);
        let path = cycle
            .iter()
            .chain(cycle.first())
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" → ");
        let finding = self.report(Report {
            kind: "dep.cycle",
            subject: members.join("\n"),
            file: cycle[0].clone(),
            line,
            title: format!("Import cycle of {} modules: {path}", members.len()),
            summary: format!(
                "Break the cycle {path}: move what the modules share into a module that \
                 imports none of them, or invert one import through an interface."
            ),
            evidence: evidence(&ImportCycle {
                size: members.len(),
                modules: members.to_vec(),
                cycle,
            }),
        });
        let size = Measurement {
            metric: "cycle_size".to_owned(),
            value: as_value(members.len()),
            target: Target {
                display: path,
                symbol: None,
                location: finding.location.clone(),
            },
        };
        (finding, size)
    }

    fn violations(&self, graph: &Graph, layers: &Layers) -> Vec<Finding> {
        let mut findings = Vec::new();
        for (from, targets) in &graph.imports {
            let Some(from_layer) = layers.of(from) else {
                continue;
            };
            for (to, import) in targets {
                let Some(to_layer) = layers.of(to) else {
                    continue;
                };
                if from_layer >= to_layer {
                    continue;
                }
                let (from_name, to_name) = (&layers.names[from_layer], &layers.names[to_layer]);
                findings.push(self.report(Report {
                    kind: "dep.layer-violation",
                    subject: format!("{from} -> {to}"),
                    file: from.clone(),
                    line: import.line,
                    title: format!("`{from}` (layer {from_name}) imports `{to}` (layer {to_name})"),
                    summary: format!(
                        "Remove the import of `{to}` from `{from}`: layer {from_name} must not \
                         depend on layer {to_name} (order: {}). Move the code down to \
                         {from_name}, or invert the dependency through an interface declared \
                         in {from_name}.",
                        layers.names.join(", ")
                    ),
                    evidence: evidence(&LayerViolation {
                        from: from.clone(),
                        to: to.clone(),
                        from_layer: from_name.clone(),
                        to_layer: to_name.clone(),
                        specifier: import.specifier.clone(),
                        order: layers.names.clone(),
                    }),
                }));
            }
        }
        findings
    }

    fn unstable_abstract(
        &self,
        graph: &Graph,
        types: &TypeCounts,
        module: &str,
        settings: &Settings,
    ) -> Option<Finding> {
        let coupling = graph.coupling(module);
        let martin = Martin::of(&coupling, types.module(module))?;
        if martin.abstractness + martin.instability - 1.0 <= settings.distance_max {
            return None;
        }
        Some(self.report(Report {
            kind: "dep.unstable-abstract",
            subject: module.to_owned(),
            file: module.to_owned(),
            line: 1,
            title: format!(
                "`{module}` is abstract but unstable: distance {:.2} from the main sequence \
                 (max {:.2})",
                martin.distance, settings.distance_max
            ),
            summary: format!(
                "Review `{module}`: its abstractions have {} importers but it imports {} \
                 modules. Move its imports into the implementations, or merge the abstractions \
                 into the modules that use them.",
                coupling.afferent, coupling.efferent
            ),
            evidence: evidence(&UnstableAbstract {
                module: module.to_owned(),
                afferent: coupling.afferent,
                efferent: coupling.efferent,
                external: coupling.external,
                unresolved: coupling.unresolved,
                instability: rounded(martin.instability),
                abstractness: rounded(martin.abstractness),
                distance: rounded(martin.distance),
                distance_max: settings.distance_max,
            }),
        }))
    }

    /// A `warn` finding about a module-level subject, with the action its manifest maps it to.
    fn report(&self, report: Report) -> Finding {
        let action = self
            .manifest
            .findings
            .iter()
            .find(|spec| spec.kind == report.kind)
            .map_or(ActionKind::Review, |spec| spec.action);
        let id = FindingId::for_subject(&self.manifest.id, report.kind, &report.subject);
        let evidence = match report.evidence {
            Value::Object(fields) => fields,
            _ => Map::new(),
        };
        Finding {
            suppress: format!("cupel suppress {id} --reason '…'"),
            repro: format!("cupel check --only {}", self.manifest.id),
            id,
            check: self.manifest.id.clone(),
            kind: report.kind.to_owned(),
            severity: Severity::Warn,
            confidence: 1.0,
            tier: self.manifest.tier,
            symbol: None,
            location: Some(Location {
                file: report.file,
                line: report.line,
                col: None,
                end_line: report.line,
                end_col: None,
            }),
            title: report.title,
            detail: None,
            evidence: Evidence(evidence),
            action: Action {
                kind: action,
                priority: 2,
                summary: report.summary,
                hints: Map::new(),
            },
            new_since_baseline: false,
            suppressed: false,
        }
    }
}

/// What one finding says, before identity and the common fields are added.
struct Report {
    kind: &'static str,
    /// The structural name the finding ID is derived from (ADR-052).
    subject: String,
    file: String,
    line: u32,
    title: String,
    summary: String,
    evidence: Value,
}

/// The first import of one module by another.
struct Import {
    line: u32,
    specifier: String,
    /// Some edge between the two uses the module rather than only declaring it (ADR-068).
    uses: bool,
}

/// One language's file-level import graph (ADR-052).
#[derive(Default)]
struct Graph {
    /// Every module: a source of an edge or the target of a `File` edge.
    modules: BTreeSet<String>,
    /// `File` edges between distinct modules, by importer then imported, at the first import.
    imports: BTreeMap<String, BTreeMap<String, Import>>,
    /// Distinct external packages per module.
    external: BTreeMap<String, BTreeSet<String>>,
    /// Unresolved imports per module.
    unresolved: BTreeMap<String, u32>,
}

impl Graph {
    fn of(fact: &ModuleGraph) -> Self {
        let mut graph = Self::default();
        let mut edges: Vec<_> = fact.roots.iter().flat_map(|root| &root.edges).collect();
        edges.sort_by_key(|edge| (&edge.from, edge.line));
        for edge in edges {
            graph.modules.insert(edge.from.clone());
            match &edge.target {
                ImportTarget::File { path } => {
                    graph.modules.insert(path.clone());
                    if *path != edge.from {
                        graph
                            .imports
                            .entry(edge.from.clone())
                            .or_default()
                            .entry(path.clone())
                            .or_insert_with(|| Import {
                                line: edge.line,
                                specifier: edge.specifier.clone(),
                                uses: false,
                            })
                            .uses |= edge.edge_kind != EdgeKind::Declaration;
                    }
                }
                ImportTarget::External { package } => {
                    graph
                        .external
                        .entry(edge.from.clone())
                        .or_default()
                        .insert(package.clone());
                }
                ImportTarget::Unresolved => {
                    *graph.unresolved.entry(edge.from.clone()).or_default() += 1;
                }
            }
        }
        graph
    }

    fn line(&self, from: &str, to: &str) -> u32 {
        self.imports
            .get(from)
            .and_then(|targets| targets.get(to))
            .map_or(1, |import| import.line)
    }

    fn coupling(&self, module: &str) -> Coupling {
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        Coupling {
            afferent: count(
                self.imports
                    .values()
                    .filter(|targets| targets.contains_key(module))
                    .count(),
            ),
            efferent: count(self.imports.get(module).map_or(0, BTreeMap::len)),
            external: count(self.external.get(module).map_or(0, BTreeSet::len)),
            unresolved: self.unresolved.get(module).copied().unwrap_or(0),
        }
    }

    /// The modules `module` uses: a cycle follows uses, never a module declaration alone,
    /// which is structure (a Rust parent declaring the child that imports it, ADR-068).
    fn used<'a>(&'a self, module: &str) -> impl Iterator<Item = &'a String> {
        self.imports
            .get(module)
            .into_iter()
            .flat_map(|targets| targets.iter())
            .filter(|(_, import)| import.uses)
            .map(|(target, _)| target)
    }

    /// Every strongly connected component of more than one module, members sorted, in order
    /// of their smallest member.
    fn cycles(&self) -> Vec<Vec<String>> {
        let modules: Vec<&String> = self.modules.iter().collect();
        let index: BTreeMap<&String, usize> = modules
            .iter()
            .enumerate()
            .map(|(position, module)| (*module, position))
            .collect();
        let successors: Vec<Vec<usize>> = modules
            .iter()
            .map(|module| self.used(module).map(|target| index[target]).collect())
            .collect();
        let mut cycles: Vec<Vec<String>> = strongly_connected(&successors)
            .into_iter()
            .filter(|component| component.len() > 1)
            .map(|component| {
                let mut members: Vec<String> =
                    component.into_iter().map(|i| modules[i].clone()).collect();
                members.sort();
                members
            })
            .collect();
        cycles.sort();
        cycles
    }

    /// The shortest cycle through `members[0]` (the smallest) that stays inside `members`,
    /// found breadth-first in sorted order so ties resolve the same way every run.
    fn shortest_cycle(&self, members: &[String]) -> Vec<String> {
        let inside: BTreeSet<&String> = members.iter().collect();
        let start = &members[0];
        let mut parent: BTreeMap<&String, &String> = BTreeMap::new();
        let mut queue = VecDeque::from([start]);
        while let Some(module) = queue.pop_front() {
            for next in self.used(module) {
                if next == start {
                    let mut cycle = vec![module.clone()];
                    let mut at = module;
                    while let Some(previous) = parent.get(at) {
                        cycle.push((*previous).clone());
                        at = previous;
                    }
                    cycle.reverse();
                    return cycle;
                }
                if inside.contains(next) && !parent.contains_key(next) {
                    parent.insert(next, module);
                    queue.push_back(next);
                }
            }
        }
        members.to_vec()
    }
}

/// Tarjan's strongly connected components of a graph given as successor lists, iteratively so
/// deep import chains cannot overflow the stack. Components come out in reverse topological
/// order; every node is in exactly one.
fn strongly_connected(successors: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut tarjan = Tarjan::new(successors);
    for root in 0..successors.len() {
        if !tarjan.visited(root) {
            tarjan.explore(root);
        }
    }
    tarjan.components
}

/// The state of Tarjan's algorithm over one graph.
struct Tarjan<'a> {
    successors: &'a [Vec<usize>],
    /// The order each node was first visited in; `None` before that.
    index: Vec<Option<usize>>,
    low: Vec<usize>,
    on_stack: Vec<bool>,
    stack: Vec<usize>,
    components: Vec<Vec<usize>>,
    next_index: usize,
}

impl<'a> Tarjan<'a> {
    fn new(successors: &'a [Vec<usize>]) -> Self {
        let n = successors.len();
        Self {
            successors,
            index: vec![None; n],
            low: vec![0; n],
            on_stack: vec![false; n],
            stack: Vec::new(),
            components: Vec::new(),
            next_index: 0,
        }
    }

    fn visited(&self, node: usize) -> bool {
        self.index[node].is_some()
    }

    /// Numbers `node` and pushes it on the component stack.
    fn visit(&mut self, node: usize) {
        self.index[node] = Some(self.next_index);
        self.low[node] = self.next_index;
        self.next_index += 1;
        self.stack.push(node);
        self.on_stack[node] = true;
    }

    /// Depth-first from `root` with an explicit stack of frames, each a node and how many of its
    /// successors have been explored.
    fn explore(&mut self, root: usize) {
        self.visit(root);
        let mut frames = vec![(root, 0)];
        while let Some(&mut (node, ref mut explored)) = frames.last_mut() {
            let Some(&next) = self.successors[node].get(*explored) else {
                frames.pop();
                if let Some(&(parent, _)) = frames.last() {
                    self.low[parent] = self.low[parent].min(self.low[node]);
                }
                self.close(node);
                continue;
            };
            *explored += 1;
            match self.index[next] {
                None => {
                    self.visit(next);
                    frames.push((next, 0));
                }
                Some(index) if self.on_stack[next] => {
                    self.low[node] = self.low[node].min(index);
                }
                Some(_) => {}
            }
        }
    }

    /// Pops `node`'s component when `node` is its root.
    fn close(&mut self, node: usize) {
        if Some(self.low[node]) != self.index[node] {
            return;
        }
        let mut component = Vec::new();
        while let Some(member) = self.stack.pop() {
            self.on_stack[member] = false;
            component.push(member);
            if member == node {
                break;
            }
        }
        self.components.push(component);
    }
}

/// A module's couplings (ADR-052): Ca and Ce over `File` edges; the rest only reported.
struct Coupling {
    afferent: u32,
    efferent: u32,
    external: u32,
    unresolved: u32,
}

/// Public types and interfaces per module, from `ApiSurface`.
struct TypeCounts(BTreeMap<String, (u32, u32)>);

impl TypeCounts {
    fn of(surface: &ApiSurface) -> Self {
        let mut counts: BTreeMap<String, (u32, u32)> = BTreeMap::new();
        for item in &surface.items {
            let abstract_item = match item.kind {
                ApiItemKind::Interface => true,
                ApiItemKind::Type => false,
                _ => continue,
            };
            let (types, interfaces) = counts.entry(item.file.clone()).or_default();
            *types += 1;
            *interfaces += u32::from(abstract_item);
        }
        Self(counts)
    }

    /// (types including interfaces, interfaces) the module declares.
    fn module(&self, module: &str) -> (u32, u32) {
        self.0.get(module).copied().unwrap_or_default()
    }
}

/// Martin's metrics for one module, when both ratios are defined (ADR-052).
#[derive(Debug, PartialEq)]
struct Martin {
    instability: f64,
    abstractness: f64,
    distance: f64,
}

impl Martin {
    /// `None` when the module has no coupling (I undefined) or declares no type (A undefined).
    fn of(coupling: &Coupling, (types, interfaces): (u32, u32)) -> Option<Self> {
        let couplings = coupling.afferent + coupling.efferent;
        if couplings == 0 || types == 0 {
            return None;
        }
        let instability = f64::from(coupling.efferent) / f64::from(couplings);
        let abstractness = f64::from(interfaces) / f64::from(types);
        Some(Self {
            instability,
            abstractness,
            distance: (abstractness + instability - 1.0).abs(),
        })
    }
}

/// The check's settings, read and validated together (ADR-052, ADR-053).
struct Settings {
    distance_max: f64,
    layers: Option<Layers>,
}

/// Configured layers, lowest first, each with its compiled glob.
struct Layers {
    names: Vec<String>,
    globs: Vec<GlobMatcher>,
}

impl Layers {
    /// The index of the first layer whose glob matches `module`.
    fn of(&self, module: &str) -> Option<usize> {
        self.globs.iter().position(|glob| glob.is_match(module))
    }
}

impl Settings {
    fn read(check: &CheckId, config: &Table) -> Result<Self> {
        let keys: Vec<String> = settings().into_iter().map(|spec| spec.name).collect();
        let error = |key: &str, reason: String| -> cupel_core::Error {
            ConfigError::CheckSetting {
                check: check.to_string(),
                key: key.to_owned(),
                reason,
                valid: keys.clone(),
            }
            .into()
        };
        if let Some(unknown) = config.keys().find(|key| !keys.contains(key)) {
            return Err(error(unknown, "not a setting of this check".to_owned()));
        }
        let distance_max = match config.get("distance_max") {
            None => DISTANCE_MAX,
            Some(value) => value
                .as_f64()
                .filter(|max| (0.0..=1.0).contains(max))
                .ok_or_else(|| {
                    error(
                        "distance_max",
                        format!("expected a number from 0 to 1, got {value}"),
                    )
                })?,
        };
        let layers = config
            .get("layers")
            .map(|layers| Layers::read(layers, &error))
            .transpose()?;
        Ok(Self {
            distance_max,
            layers,
        })
    }
}

impl Layers {
    fn read(value: &Value, error: &dyn Fn(&str, String) -> cupel_core::Error) -> Result<Self> {
        let shape = || {
            error(
                "layers",
                "expected a table with exactly `order` (layer names, lowest first) and `map` \
                 (a glob per layer)"
                    .to_owned(),
            )
        };
        let table = value.as_object().ok_or_else(shape)?;
        if table.len() != 2 {
            return Err(shape());
        }
        let (Some(order), Some(map)) = (table.get("order"), table.get("map")) else {
            return Err(shape());
        };
        let names = Self::order(order).ok_or_else(|| {
            error(
                "layers.order",
                format!("expected a non-empty array of distinct layer names, got {order}"),
            )
        })?;
        let map = map.as_object().ok_or_else(shape)?;
        if let Some(stray) = map.keys().find(|key| !names.contains(key)) {
            return Err(error(
                &format!("layers.map.{stray}"),
                "not a layer named in `layers.order`".to_owned(),
            ));
        }
        let globs = names
            .iter()
            .map(|name| {
                let key = format!("layers.map.{name}");
                let glob = map
                    .get(name)
                    .ok_or_else(|| error(&key, "every layer in `order` needs a glob".to_owned()))?;
                let text = glob
                    .as_str()
                    .ok_or_else(|| error(&key, format!("expected a glob, got {glob}")))?;
                GlobBuilder::new(text)
                    .literal_separator(true)
                    .build()
                    .map(|glob| glob.compile_matcher())
                    .map_err(|reason| error(&key, format!("not a valid glob: {reason}")))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { names, globs })
    }

    fn order(value: &Value) -> Option<Vec<String>> {
        let names: Vec<String> = value
            .as_array()?
            .iter()
            .map(|name| {
                name.as_str()
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned)
            })
            .collect::<Option<_>>()?;
        let distinct: BTreeSet<&String> = names.iter().collect();
        (!names.is_empty() && distinct.len() == names.len()).then_some(names)
    }
}

/// Ratios in evidence are rounded to three decimals, so goldens do not depend on float noise.
fn rounded(ratio: f64) -> f64 {
    (ratio * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn components(successors: &[Vec<usize>]) -> Vec<Vec<usize>> {
        let mut components: Vec<Vec<usize>> = strongly_connected(successors)
            .into_iter()
            .map(|mut component| {
                component.sort_unstable();
                component
            })
            .collect();
        components.sort();
        components
    }

    #[test]
    fn a_dag_has_only_singleton_components() {
        let dag = [vec![1, 2], vec![2], vec![], vec![0]];
        assert_eq!(components(&dag), [vec![0], vec![1], vec![2], vec![3]]);
    }

    #[test]
    fn one_cycle_is_one_component() {
        let cycle = [vec![1], vec![2], vec![0], vec![0]];
        assert_eq!(components(&cycle), [vec![0, 1, 2], vec![3]]);
    }

    #[test]
    fn nested_cycles_sharing_a_node_merge_and_disjoint_cycles_do_not() {
        // 0 → 1 → 0 and 1 → 2 → 3 → 1 share 1; 4 ↔ 5 is separate and feeds 0.
        let graph = [vec![1], vec![0, 2], vec![3], vec![1], vec![5], vec![4, 0]];
        assert_eq!(components(&graph), [vec![0, 1, 2, 3], vec![4, 5]]);
    }

    #[test]
    fn a_self_loop_is_a_singleton_and_a_long_chain_does_not_overflow() {
        assert_eq!(components(&[vec![0]]), [vec![0]]);
        let n = 100_000;
        let mut chain: Vec<Vec<usize>> = (1..n).map(|next| vec![next]).collect();
        chain.push(vec![0]);
        let found = strongly_connected(&chain);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].len(), n);
    }

    fn coupling(afferent: u32, efferent: u32) -> Coupling {
        Coupling {
            afferent,
            efferent,
            external: 0,
            unresolved: 0,
        }
    }

    #[test]
    fn martin_metrics_are_defined_only_with_a_coupling_and_a_type() {
        assert_eq!(
            Martin::of(&coupling(1, 3), (4, 1)),
            Some(Martin {
                instability: 0.75,
                abstractness: 0.25,
                distance: 0.0,
            })
        );
        assert_eq!(
            Martin::of(&coupling(2, 0), (1, 0)),
            Some(Martin {
                instability: 0.0,
                abstractness: 0.0,
                distance: 1.0,
            })
        );
        assert_eq!(Martin::of(&coupling(0, 0), (1, 1)), None);
        assert_eq!(Martin::of(&coupling(1, 1), (0, 0)), None);
    }

    #[test]
    fn evidence_ratios_round_to_three_decimals() {
        let ratios = [rounded(1.0 / 3.0), rounded(2.0 / 3.0)];
        assert_eq!(ratios.map(|ratio| ratio.to_string()), ["0.333", "0.667"]);
    }
}
