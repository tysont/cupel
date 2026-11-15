<!-- ABOUTME: Type checkers and linters, dependency and footprint checks, and benchmarks. -->
<!-- ABOUTME: Part of Cupel's guide, linked from the README. -->
# Analyzers, dependencies, and benchmarks

## Analyzers

`static.analyze` runs every analyzer of a language that is installed, in each project root, and keeps the results in files in scope. Type checkers run at their strictest; linters use the project's own configuration.

| Language | Analyzers | Run as |
|---|---|---|
| Rust | clippy | `cargo clippy --all-targets --message-format=json`, with the project's `[lints]` |
| Python | mypy, ruff | `mypy --strict --output json` and `ruff check --output-format json` on the files in scope |
| JavaScript | ESLint | `eslint --format json --no-warn-ignored` (files the config ignores are skipped), only where the root or a directory above it has a flat config (`eslint.config.js` and its siblings), as ESLint finds it |
| TypeScript | tsc, ESLint | `tsc --noEmit --strict` over the root's `tsconfig.json`, else over the files in scope; ESLint as for JavaScript |
| Go | go vet, staticcheck | `go vet -json` and `staticcheck -f json` over the module's packages |

The check runs when at least one of a language's analyzers is installed; each one that is missing, unconfigured (ESLint without a config), or failed (its output could not be read) is a note in the result saying what to install or fix, and the others still run. A failure names the project root it happened in, and the roots where the analyzer did run keep their findings ([ADR-169](../adr/CUPEL_DECISION_169.md)). The finding's evidence holds the `tool`, its `rule`, the `level`, and the `message`; its ID does not change when the line moves. clippy's E0554, a `#![feature]` attribute the stable channel rejects (a nightly-only benchmark, say), is a note, `info`, whatever level rustc gives it: it says which toolchain the target needs, not what is wrong with its code ([ADR-232](../adr/CUPEL_DECISION_232.md)). The gate keys are `static.analyze.errors` and `diagnostics`. Results depend on each tool's version, so pin them where a gate reads them.

## Dependencies and footprint

`perf.footprint` reads, in each project root, the lockfile or manifest the ecosystem keeps, and never the network:

| Language | Dependencies | Licenses |
|---|---|---|
| Rust | `Cargo.lock`, except the workspace's own packages | `cargo metadata --offline --locked`, from the local registry cache |
| Python | `uv.lock`, else `poetry.lock`, else `requirements*.txt` | the package metadata in `.venv/` or `venv/` |
| JavaScript, TypeScript | `package-lock.json`, else the `pnpm-lock.yaml` pnpm installs the root from (its workspace's, for a member: what its importer locks, transitively), else `package.json` (without versions) | the lockfile; for pnpm, the packages installed in `node_modules/.pnpm` |
| Go | `go.mod`'s `require` lines | not read |

A dependency is direct when the project declares it itself. Roots that share a lockfile list each of its packages once ([ADR-168](../adr/CUPEL_DECISION_168.md)). Two measurements are opt-in, per language:

```toml
[lang.rust]
advisories = "advisories"            # a directory of OSV advisories (*.json), such as an osv.dev export
artifacts = ["target/release/app"]   # files or globs whose sizes the artifact_bytes metric adds up
```

A dependency whose name and version an advisory's `affected` entry lists, or falls in its ranges, is `dep.vuln` (`fail`, with the advisory and the fixed version); a license expression with no alternative free of GPL, AGPL, LGPL, SSPL, EUPL, or OSL is `dep.license` (`warn`). Under `--changed` or `--base` the same files are read at the base commit, and only dependencies whose name and version are new are reported; `footprint.grew` (`warn`) lists what was added and removed when the count grew. The gate keys are `perf.footprint.dependencies`, `vulnerable`, `copyleft`, and `artifact_bytes` (a size budget, such as `"perf.footprint.artifact_bytes" = "<= 4000000"`); what was not measured (licenses, advisories, artifacts) is a note, never a silent zero.

## Benchmarks

`perf.bench` compares the project's benchmarks at the range's base commit with the working tree. The producer exports the base commit with `git archive`, measures it, then measures the working tree right after, on the same machine with the same tool; nothing is cached, and everything the tools write goes to `.cupel/cache/bench/`:

| Language | Tool | Run as | Samples |
|---|---|---|---|
| Go | `go test -bench` | `go test -run=^$ -bench=. -benchmem -count=<n> -benchtime=100ms -json` | `ns`, `bytes`, and `allocs` per operation |
| Rust | criterion | `cargo bench --bench <target> -- --noplot` for each bench target of a package that depends on criterion | `ns` per iteration |
| Python | pytest-benchmark | `python3 -m pytest --benchmark-only` with its storage in the cache | `ns` per call |
| JavaScript, TypeScript, Python | hyperfine | `hyperfine --runs <n> --export-json` over the commands `[lang.<id>] bench_commands` lists | `ns` per run |

Each benchmark is measured 5 times per side at `--depth standard` (with `--only`), 10 at `deep`, and 20 at `exhaustive`. Every sample is a cost, so lower is better; allocation counts do not depend on the machine's load, and wall time does. The check compares the two sides of each benchmark and unit with benchstat's method, a two-sided Mann-Whitney U test, and the change of the medians: a change with p below `significance_percent` (5) and at least `threshold_percent` (5) slower is `perf.regression`, as much faster `perf.improvement`. On the Go fixture, after a branch makes `Sum` copy its input:

```text
$ cupel check --only perf.bench --changed --gate --format text
cupel check: fail (exit 1)

gate      fail (gate.changed)
  fail  perf.bench.regression < 5%  actual 1.75065491783758

warn (3)
  perf.bench
    `example.com/units.BenchmarkSum` regressed 100.0% in bytes (p = 0.0079)
    `example.com/units.BenchmarkSum` regressed 175.1% in ns (p = 0.0079)
    `example.com/units.BenchmarkSum` regressed 100.0% in allocs (p = 0.0079)
```

The evidence holds the `benchmark`, its `unit`, both medians, the `change` (a fraction of the base's median; growth from a median of 0 counts as 1), the `p_value`, both sample counts, and the settings it was judged by. The gate keys are `perf.bench.regression`, the largest significant slowdown as a fraction whatever the threshold (so `"perf.bench.regression" = "< 5%"` fails on a significant 5% slowdown), and `benchmarks`, `compared`, `regressed`, and `improved`. On a whole tree the benchmarks run once and nothing is compared; a note says to run under `--changed` or `--base`. The base side is an export, so ignored files (a virtual environment, `node_modules`) are not there: a benchmark that needs them fails on the base side only, with a note. JavaScript and TypeScript have no bench producer but hyperfine, so `perf.bench` is `unconfigured` for them until `bench_commands` is set.
