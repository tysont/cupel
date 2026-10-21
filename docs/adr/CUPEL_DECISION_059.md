# CUPEL_DECISION_059: `static.dead-code` findings: severities, confidence, needs, and score

**Date:** 2026-10-13 · **Status:** accepted; its needs, evidence, and the confidence of a symbol-level `dead.symbol` amended by [135](CUPEL_DECISION_135.md) · **Phase:** P2.8

## Context

Design §5.1 gives `static.dead-code` the kinds `dead.symbol` and `dead.export`; plan P2.8 says `confidence` reflects uncertainty (dynamic dispatch, reflection, `getattr`). ADR-058 fixes what the check can see. Still open: when an unimported export is dead (a library's public API is imported by nothing in its own repository, by design), what each finding's severity, confidence, action, and evidence are, which facts the check declares, and its metrics and score.

## Options

**Library or application.** (a) Read `ProjectInfo` (a `wants`): it lists build files, members, and skip directories, but not whether a root publishes a library (`[lib]`, `main`/`exports`, an importable package) — no fact does, and the check may not read manifests. (b) Report unimported exports everywhere at `warn`: every library's API becomes warnings. (c) Report them as `info` with low confidence, saying in the finding that a library's API is expected to look like this.

**Per-language confidence.** A baseline per language (Python's `getattr`, JavaScript's computed `require`) would need the check to name languages, which the matrix rule forbids (plan §2.8); a per-language default in configuration has the same problem. Confidence can only come from evidence in the facts.

**Needs.** The step text lists `Symbols`, `ModuleGraph`, `Roots`, and `ApiSurface`. Finding IDs and repros need the structural hash, which only `NormTree` carries (as for `static.complexity`, ADR-042). `ApiSurface` adds nothing ADR-058 uses: `Roots`' `export` reason is the same public set for symbols, and the items only `ApiSurface` lists (types, constants, modules) have no structural hash to identify a finding.

## Decision

- **Manifest:** `static.dead-code`, version 1, tier 0, cost `quick`, needs `Symbols`, `NormTree`, `ModuleGraph`, `Roots`; no wants; no settings (any key under `[checks."static.dead-code"]` is a usage error).
- **`dead.symbol`**, severity `warn`, action `review`: a symbol in a module with no exports that is neither entered nor rooted (ADR-058). Confidence **0.8**: the module is unreachable through every import and root the facts carry, and only code outside symbols or loading by computed name could reach it.
- **`dead.export`**, severity `info`, action `review`: an exported symbol in a module that is neither entered nor rooted. Confidence **0.4**: nothing in the repository imports it, but whether it is a library's public API is not in any fact.
- **Suspect imports.** When an `Unresolved` or `External` import anywhere in the graph has a specifier one of whose segments (split on `/`, `.`, `:`, `\`, and `@`) equals the file's stem, the import may be the one that loads the file (a path the resolver could not follow, a dynamic import, a local module misread as a package). Confidence drops to **0.2** for `dead.symbol` and **0.1** for `dead.export`, and the specifiers are listed in evidence.
- **Action** `review`: Cupel does not delete code (design §7.2's `fixes` are empty), and deleting is a judgment about intent. The summary names the symbol and file, and says to delete it or import it where it is meant to be used; the finding's `detail` states exactly what was checked and what could not be seen.
- **Evidence** (both kinds): `file`, `module` (the module namespace), `module_files` (every file of the module, none imported), and `suspect_imports` (the matching specifiers, usually empty).
- **Identity:** `FindingId::compute("static.dead-code", kind, symbol hash, None)`, repro `cupel check --only static.dead-code --symbols <hash>`, location the symbol's span.
- **Metrics** (gate keys): `dead_symbols` and `dead_exports`, the number of findings of each kind in scope.
- **Score:** `{value: dead symbols, unit: "dead_symbols", n: symbols in scope}`; per-language scores combine by summing values and `n`. Status `fail` when a `dead.symbol` was reported, else `pass` (`info` findings do not fail a check, ADR-041).

## Consequences

- A library's untested public API appears as `info` findings with confidence 0.4, which an agent can sort below everything else or suppress per finding; an application's dead exports appear the same way. A future fact that marks published entry points (a `ProjectInfo` field per root) would let `dead.export` split the two, and would let `dead.symbol` report the private symbols of an unimported application module that also exports something, which it leaves alone today.
- Gate rules can bound `dead_symbols` (e.g. `static.dead-code.dead_symbols <= 0`) without counting exports.
- Fixture files that other checks add without importing them (`complexity`, `twins`) produce `dead.export` findings in this check's goldens. That is the true statement "nothing in the repository imports this file"; each is reviewed with the golden.
