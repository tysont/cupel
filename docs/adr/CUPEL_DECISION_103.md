# CUPEL_DECISION_103: The `Footprint` fact, its offline producers, and `perf.footprint`

**Date:** 2026-10-16 · **Status:** accepted · **Phase:** P6.2

## Context

Design §5.6: `perf.footprint` measures "binary/bundle size, dependency count, license and vuln deltas", kinds `footprint.grew`, `dep.vuln`, `dep.license`, cost `quick`, wrap. No fact in the v1 catalog (§8.3.2) carries any of it, and adding a kind needs an ADR (§8.3.1, plan §2.5). Plan P6.2 asks for a `Footprint` fact with binary/bundle size, dependency count, licenses, and vulnerabilities, producers per ecosystem, and a statement of what is measured without network access. Open: what each ecosystem's producer reads; where license and vulnerability data come from when Cupel must not reach the network (an agent's sandbox often has none, and a grade must not change because a registry did); which artifacts' sizes are measured, since a `quick` check cannot build anything; and what a "delta" is compared with, when the check cannot read history or git (§8.3.1) and the base side of a range (ADR-056) holds only the changed source files, never a lockfile.

## Options

**What is read.** (a) The package managers' own commands (`cargo tree`, `npm ls`, `pip list`, `go list -m all`): exact, but most resolve or download when a lockfile is stale, and `pip list` describes the interpreter, not the project. (b) The lockfiles and manifests on disk (`Cargo.lock`, `package-lock.json`, `uv.lock`, `poetry.lock`, `requirements*.txt`, `go.mod`): offline, deterministic, and what the project's build will install. (b), with one exception below for Rust licenses.

**Licenses.** They are in `package-lock.json` (npm records each package's `license`), in an installed Python package's `METADATA` (`License-Expression`, else `License`), and for Rust in each package's `Cargo.toml`, which `cargo metadata --offline --locked` reports from the local registry cache without writing or downloading. Go modules declare none outside their `LICENSE` text, which would need a classifier. A fact that says whether licenses were read keeps "no copyleft found" apart from "not looked".

**Vulnerabilities.** Every ecosystem's audit tool (`cargo audit`, `npm audit`, `pip-audit`, `govulncheck`) fetches its database by default. (a) Wrap them and accept network use: grades differ between a laptop and a sandbox. (b) Read a directory of OSV advisories (the format osv.dev and the GitHub advisory database export) that the project keeps or mirrors, named in `[lang.<id>]`: offline, the same for every run of a commit, one matcher for every ecosystem. (b); without the key the fact says vulnerabilities were not measured.

**Sizes.** Building the project to measure its binary is not `quick` and is a test runner's or a bench's job. Measuring files the project names (`[lang.<id>] artifacts = ["target/release/app"]`), when they exist, costs a `stat` each.

**The comparison.** (a) `base_needs = [Footprint]`: the check would be skipped without a range, though most of what it reports (a vulnerable dependency) matters on a whole tree too, and the base checkout has no lockfiles. (b) The run's baseline (`--against`): checks cannot read the store. (c) The producer reads the manifests it read at head again at the range's base commit (`git show`), as the differential engine exports a ref (ADR-093), and puts the base dependencies in the fact; the scheduler tells producers the base commit through `Focus`, beside the changed lines it already passes. (c): the check stays a pure function of one fact, runs on every scope, and compares where a base exists.

## Decision

1. **`footprint` v1** (`FactKind::Footprint`, appended to the catalog after `relation-results`; payload `cupel_core::Footprint`), an execution fact with no conformance inventory, providable by producer plugins:

```text
Footprint {
  manifests: [String],                       // lockfiles and manifests read, relative to the repository root
  dependencies: [Dependency {
    name, version: Option<String>,           // version null when only a range is declared (package.json without a lockfile)
    direct: bool,                            // declared by the project itself, not pulled in by another dependency
    license: Option<String>,                 // SPDX expression or declared text, when the ecosystem records one
    file, line,                              // where the lockfile or manifest lists it
  }],
  licenses: bool,                            // whether licenses were read for these dependencies
  vulnerabilities: Option<[Vulnerability { id, package, version, summary, fixed: Option<String> }]>,  // null: no advisories configured
  artifacts: Option<[Artifact { path, bytes }]>,                                                      // null: none configured
  base: Option<FootprintBase { commit, dependencies: [Dependency] }>,                                 // null outside a range
}
```

2. **`tool.footprint`**, one producer per language, needs `ProjectInfo`, cost `quick`, always ready, reads each project root of its language (none detected: nothing is read). Nothing touches the network or writes in the tree.

| Language | Dependencies from | Direct | Licenses |
|---|---|---|---|
| `rust` | `Cargo.lock`: every package except the workspace's own | named in a member's `dependencies` in the lockfile | `cargo metadata --format-version 1 --offline --locked`; not read when it fails |
| `python` | `uv.lock`, else `poetry.lock`, else `requirements*.txt` (`name==version`; another specifier leaves the version null) | named in `pyproject.toml`'s `[project] dependencies` or `[tool.poetry.dependencies]`; every requirement | `.venv/` or `venv/` `site-packages/*.dist-info/METADATA` |
| `javascript`, `typescript` | `package-lock.json` (`packages`, v2 and v3), else `package.json`'s declared dependencies | in `package.json`'s `dependencies`, `devDependencies`, `optionalDependencies`, `peerDependencies` | the lockfile's `license` |
| `go` | `go.mod` `require` | not marked `// indirect` | not read |

3. **Advisories**: `[lang.<id>] advisories = "<directory>"` (relative to the repository root): every `*.json` file in it is one OSV advisory. A dependency is affected when an `affected[]` entry names its ecosystem (`crates.io`, `PyPI`, `npm`, `Go`) and name (PyPI names compared normalized) and lists its version in `versions` or in a range (`introduced` ≤ version < `fixed`, or ≤ `last_affected`; versions compared by dot-separated numeric parts, then text). `fixed` is the range's first fixed version. An advisory file that does not parse is the producer's error, naming the file. Cupel never downloads advisories.

4. **Artifacts**: `[lang.<id>] artifacts = ["<glob>", …]` (relative to the repository root): every regular file a pattern matches, with its size; patterns that match nothing add nothing.

5. **The base**: `Focus` gains `base: Option<String>`, the commit a range starts from (`--changed`'s merge base or `--base`), set by the scheduler for every producer. Under a range `tool.footprint` reads each manifest it read at head as it was at that commit and records its dependencies (licenses are not read there); a manifest absent at the commit contributes none. Outside a git work tree or a range, `base` is null.

6. **`perf.footprint`**: tier 0, cost `quick`, needs `Footprint`, no settings, stability `experimental`. *Reported* dependencies are those whose `name@version` is not among the base's, or every dependency when there is no base.
   - **`dep.vuln`** (severity `fail`, action `fix_bug`): each vulnerability of a reported dependency; evidence `{package, version, advisory, summary, fixed}`; ID subject `<package>@<version>|<advisory>`.
   - **`dep.license`** (severity `warn`, action `review`): each reported dependency whose license expression has no `OR` alternative free of a copyleft license (identifiers starting `GPL-`, `AGPL-`, `LGPL-`, `SSPL-`, `EUPL-`, `OSL-`); evidence `{package, version, license}`; ID subject `<package>@<version>`.
   - **`footprint.grew`** (severity `warn`, action `review`): with a base, when the dependencies grew in number; evidence `{base, head, added, removed}` (`name@version` lists); ID subject `dependencies`; located at the first added dependency.
   - Findings are located where the lockfile lists the dependency; repro `cupel check --only perf.footprint`, with `--base <commit>` under a range.
   - Score `{value: dependencies, unit: "dependencies", n: dependencies}`; metrics, all summed across languages and always reported: `dependencies`, `vulnerable` (reported dependencies with a vulnerability), `copyleft` (reported dependencies with `dep.license`), `artifact_bytes` (configured artifacts' total). What was not measured (licenses, advisories, artifacts) is a note, so a zero is never mistaken for a measurement. Status `fail` with any finding.

## Consequences

- The vocabulary has a 23rd kind; `describe fact footprint`, the plugin kind table, and the schema table list it.
- Vulnerability findings need advisories on disk. A project that wants osv.dev's data mirrors it (or the subset for its ecosystems) into the directory and refreshes it on its own schedule; the grade then changes when the mirror does, not when the network does.
- `artifact_bytes` is a size budget for a gate (`"perf.footprint.artifact_bytes" = "<= 4000000"`), not a delta: comparing sizes with the base needs the base built, which a `quick` check does not do. A bench-like producer that builds both sides is the follow-up.
- Go licenses, `pnpm-lock.yaml`, `yarn.lock`, `Pipfile.lock`, and `go.sum`-only modules are not read; each is a reader in the producer when needed.
- Rust licenses come from the local registry cache; a crate never fetched there makes `cargo metadata` fail and `licenses` false rather than a guess.
- `Focus.base` is available to every producer; producers that read history beyond the scope (a future API-surface producer wrapping `cargo-semver-checks`) can use it the same way.
