# CUPEL_DECISION_052: `static.deps`: modules, coupling, abstractness, distance, cycles, and findings

**Date:** 2026-10-13 · **Status:** accepted · **Phase:** P2.5

## Context

Design §5.1 says `static.deps` measures "afferent/efferent coupling, instability, abstractness, distance from main sequence, cycles, layering-rule violations from config" and emits `dep.cycle`, `dep.layer-violation`, and `dep.unstable-abstract`; §9.2 names Tarjan's SCC algorithm; plan P2.5 says external and unresolved edges are excluded from coupling but counted in evidence. Martin's metrics are defined for *components*, and the design does not say what a module is for each language, where abstractness comes from, what instability or abstractness are for a module with no couplings or no types, which distance is reported, the finding identity of a finding that concerns several files and no symbol, the severities, the score, or the gate metrics. The check may not branch on a language (plan §2.8), so whatever unit it uses must follow from `ModuleGraph`'s semantics alone.

## Options

**Module unit.** (a) The file, for every language: `ModuleGraph` edges point at files (ADR-028), and a Go package import is already one edge per file of the package (ADR-033), so a package cycle shows up as a cycle between files of the two packages. Go files of one package never import each other, so intra-package coupling is invisible; Python packages and JavaScript barrels are files (`__init__.py`, `index.ts`) and stay files. (b) Directories where "the language's unit is a directory": the check would have to learn which languages those are (forbidden), or infer it from fan-out edges (an import whose specifier yields several files), which is fragile and still encodes a Go convention implicitly. (c) A new `ModuleGraph` field naming each file's module: a fact change across five languages for a refinement nothing needs yet.

**Abstractness source.** `Symbols` holds functions, methods, and closures only (`SymbolKind`), so it cannot say what is abstract. `ApiSurface` items carry `ApiItemKind::Interface` (traits, interfaces, protocols) and `Type`; it lists public items only and does not distinguish abstract classes from concrete ones.

**Undefined ratios.** Instability `Ce / (Ca + Ce)` is undefined for a module with no file edges; abstractness `Na / Nc` is undefined for a module declaring no type. Defaulting both to 0 puts every function-only file that others import at the "zone of pain" corner (D = 1), so `max_distance` would be 1 on nearly every repository and useless as a gate key.

**What `dep.unstable-abstract` reports.** The kind's name is one side of the main sequence: abstract modules nothing depends on (A + I − 1 > 0, the "zone of uselessness"). Reporting the other side (concrete and stable) under the same kind would contradict its name.

**Identity.** A cycle, a violating import, and a module are not symbols, so `FindingId::compute` (which takes a `SymbolHash`) does not apply; `FindingId::for_subject` (ADR-016) takes the structural name of the subject.

## Decision

- **Module = file**, for every language, from `ModuleGraph` as produced; every root's edges join one graph per language. A module is any file that is the source of an edge or the target of a `File` edge. Edges from a file to itself (a Rust `mod tests` importing `super::*`) are ignored.
- **Coupling** counts distinct modules over `File` edges only: efferent Ce = the modules a module imports, afferent Ca = the modules that import it. `External` edges (distinct packages) and `Unresolved` edges are counted per module and appear in `dep.unstable-abstract` evidence, never in Ca or Ce.
- **Abstractness** comes from `ApiSurface`: A = interfaces / (types + interfaces) among the module's public items. `needs = [ModuleGraph, ApiSurface]`.
- **Distance** D = |A + I − 1| is defined only for modules with at least one coupling and at least one public type or interface. `max_distance` (a gate metric) is the largest D over those modules, 0 when there are none.
- **`dep.unstable-abstract`** fires when A + I − 1 > `distance_max` (default 0.7, a number in [0, 1] under `[checks."static.deps"]`). Evidence: `module`, `afferent`, `efferent`, `external`, `unresolved`, `instability`, `abstractness`, `distance`, `distance_max` (ratios rounded to three decimals). Location: the module's line 1. Action `review`.
- **Cycles** are the strongly connected components of more than one module (Tarjan, iterative), one `dep.cycle` per component. Evidence: `cycle`, the shortest cycle through the component's smallest module, in import order; `modules`, every member sorted; `size`. Location: the first module's import of the second. Action `break_cycle`.
- **`dep.layer-violation`** follows ADR-053. Action `review`.
- **Identity** is `FindingId::for_subject("static.deps", kind, subject)`: the sorted members joined by newlines for a cycle, `from -> to` for a violation, the module path for an unstable module. The repro is `cupel check --only static.deps`: coupling and cycles need the whole graph, so narrowing the repro to one file would hide what it reproduces.
- All findings are `warn` with confidence 1; a gate escalates them (P2.9). **Metrics:** `cycles`, `max_distance`, `violations`. **Score:** `{value: cycles, unit: "cycles", n: modules}`; per-language scores are summed. **Status:** `fail` when any finding was emitted.
- The graph is the one the producers built for the files in scope: on a narrowed scope, edges from files outside it are absent, so afferent coupling and cycles are complete only for a whole-tree run.

## Consequences

- No language knowledge in the check; a language whose natural module is a directory (Go) is measured per file. Package-level aggregation, if wanted, is an additive `ModuleGraph` field and a check change, not a new check.
- Rust `mod` declarations are edges (ADR-028), so a child module that imports an item defined or re-exported by its parent forms a cycle with it (`lib.rs` ↔ `account.rs`). That is a true file-level cycle, but in Rust it is idiomatic; telling declaration edges from use edges needs a field on `ImportEdge` (a fact change in all five languages). Python packages that re-export from submodules which import the package back are real import cycles and are reported as such.
- Abstract classes count as concrete types until `ApiItemKind` distinguishes them; JavaScript has no interfaces, so every JavaScript module has A = 0 and `dep.unstable-abstract` never fires there.
- Finding IDs change when a module is renamed or moved, as a file-level subject must.
