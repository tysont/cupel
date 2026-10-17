# Cupel — Technical Design

**Status:** Draft v1 · **Date:** 2026-10-10 · **Owner:** Tyson

A *cupel* is the porous cup used in fire assay to separate precious metal from base. Cupel is a verification suite that grades what a coding agent produced.

---

## 1. Purpose

Agents build the right thing when they have broad and deep ways to verify what they are doing. Everyone uses functional tests. Cupel adds the other ways — structural metrics, test-suite strength, generative testing, concurrency checks, formal specs, performance regression — behind one CLI with one output contract, designed so an agent can learn the tool from the tool, run anything without side effects first, and always get back a ranked list of what to do next.

Cupel is inspired by Robert C. Martin's `crapper`, `dryer`, `mutator`, `scrap`, and `uml-viewer`, consolidated into a single binary and redesigned for agent consumption.

## 2. Goals and non-goals

**Goals**

- One binary, zero runtime dependencies, drops into any repo an agent is working in.
- One output envelope for every verb; findings carry stable IDs, an action, and a reproduction command.
- Every verb, check, flag, finding kind, exit code, and schema is self-described by the binary.
- `--dry-run` on every verb; nothing written outside `.cupel/` without `--write`.
- Diff-scoped by default, differential against a baseline, cached by content hash.
- Modular along three independent axes — techniques, languages, external tools — joined through a small vocabulary of language-neutral facts (§8.3). A new check works for every language that already produces the facts it needs; a new language lights up every check whose facts it can produce; a new tool is one producer. The support matrix is computed and self-described, never hand-maintained.
- Launch languages: JavaScript/TypeScript, Rust, Go, Python — every check that applies to a language works for all four from the first release.
- Fast enough to live in an agent's inner loop: static checks in seconds, diff-scoped mutation in minutes.

**Non-goals**

- Cupel does not fix code. It grades; the agent fixes.
- Cupel does not replace tools that are already best-in-class (PIT, Stryker, Hypothesis, TLC, Elle). It harnesses and normalizes them.
- Cupel is not a CI product or dashboard. It emits SARIF/JSON; others render.
- No interactive mode. Ever.

## 3. Background: the Uncle Bob tools

All three share one architecture: a Python CLI that walks a tree, auto-detects six languages (Clojure, Java, Go, TypeScript, Rust, Python), analyzes per-function, and writes `.metrics/*.edn` snapshots that `uml-viewer` joins on `namespace` + function name.

| Tool | What it does | Key mechanics |
|---|---|---|
| **crapper** | CRAP score `CC² × (1 − coverage)³ + CC` per function | Runs each language's coverage tool itself; `--no-coverage`, `--use-existing-coverage`, `--changed`, `--threshold N` (exit 2). Identity is positional — a rename is a new entry. |
| **dryer** | Clone detection | Normalizes each function's AST (drops locals, fields, literals, keywords; keeps call names, operators, tree shape), builds structural fingerprints per subtree, scores pairs by Jaccard. Default threshold 0.82, min 4 lines / 20 nodes. Same-language only. |
| **mutator** | Mutation testing | Small operator set (boolean flips, relational/equality swaps, `+/-`, `*//`, `&&/\|\|`, `0/1`). Coverage-gated: uncovered sites are not run. Workers get a private copy of the mutated file, rest of tree symlinked. Differential: reruns only survivors and functions whose text hash changed. Exit 0 all killed / 2 baseline red / 3 survivors. Timeout 10× baseline. |
| **scrap** | Test-code quality (SCRAP = CRAP for tests) | Size, branching, mocking, weak assertions, duplicated scaffolding; reports a refactor pressure, remediation mode, and *actionability class for an AI assistant*. |

**Keep:** `--changed` scoping; differential reruns against a snapshot; coverage-gating mutants; semantic exit codes; refusing to score when the baseline suite is red; per-function join key; SCRAP's "remediation mode / actionability" framing.

**Drop:** EDN; three-repo coupling; positional function identity; text-first output; auto-installing tooling by default; Python runtime.

## 4. Core concepts

| Term | Definition |
|---|---|
| **Symbol** | A function or method, identified by `namespace::name` for display and by a **structural hash** (hash of its normalized AST) for identity. Identity survives rename, move, and reformatting. |
| **Check** | One verification technique with a stable namespaced ID (e.g. `tests.mutation`). Declares tier, cost class, languages, prerequisites, flags, and the finding kinds it emits. |
| **Finding** | One actionable observation produced by a check. Has a stable ID, severity, confidence, tier, location, symbol, evidence, action, and repro. |
| **Scope** | The set of files/symbols a run considers: `--changed`, `--paths`, `--symbols`, `--base/--head`, or whole tree. |
| **Depth** | A cost class budget: `quick`, `standard`, `deep`, `exhaustive`. Checks declare their cost class; depth selects which run. `--budget` overrides with a wall-clock knapsack. |
| **Tier** | Evidence grade of a check, 0–6. Higher tiers are stronger evidence of correctness. |
| **Oracle** | Something to compare against: an old ref, a reference implementation, a formal spec. Registered and verified via `cupel oracle`. |
| **Baseline** | A named prior run. Deltas (`new_since_baseline`, `summary.new/resolved`) are computed against it. |
| **Fact** | A typed, versioned, language-neutral intermediate (`Symbols`, `Decisions`, `TestFrames`, `CoverageMap`, `MutationOutcomes`, …). Facts encode semantics, not syntax. Every check consumes only facts; every language, tool, and engine only produces them. The fact vocabulary is the one slow-changing contract that keeps checks × languages × tools from multiplying (§8.3). |
| **Producer** | Anything that makes a fact: a **language producer** (from the syntax tree, mostly declarative), a **tool producer** (from an external tool's output), or an **engine** (from executing something, e.g. the mutation engine). Declares which languages it serves and which facts it needs. |
| **Language** | Grammar(s), a declarative `LanguageSpec` (tree-sitter queries + tables), and the producers it registers. Languages in the same **family** (JavaScript + TypeScript) are comparable for clone detection. |
| **Support matrix** | The computed table of (check, language) → `supported \| unsupported(fact) \| unavailable(tool)`, derived from check needs and registered producers. Rendered by `describe matrix`; enforced by the test suite. |
| **Plugin** | A check implemented outside the core binary, speaking the plugin protocol over stdio (or WASM later). |
| **Gate** | A policy in `cupel.toml` evaluated at the end of a run to produce pass/fail. |
| **Envelope** | The single JSON output shape every verb returns. |

### Evidence tiers

| Tier | Meaning | Example checks |
|---|---|---|
| 0 | Structural/static metric | `static.complexity`, `static.clones`, `static.deps` |
| 1 | Static analysis / type system | `static.analyze` |
| 2 | Coverage and test-suite hygiene | `tests.coverage`, `tests.quality`, `tests.weakening`, `tests.flaky` |
| 3 | Test strength (mutation) | `tests.mutation` |
| 4 | Generative / independent oracle | `gen.property`, `gen.fuzz`, `gen.differential` (with verified oracle), `gen.metamorphic`, `conc.race`, `conc.dst` |
| 5 | Spec-conformant | `spec.contracts`, `spec.trace`, `conc.linearizability`, `conc.model-check` |
| 6 | Proof | `spec.prove` |

An agent deciding whether it is "done" needs to know what *kind* of confidence it has, not just pass/fail. The tier travels with every check result and finding.

## 5. Check catalog

Check IDs are stable. Namespace = group. Groups are valid `--only`/`--skip` targets.

### 5.1 `static.*` — structural quality (no execution; cost `quick`; tier 0–1)

| ID | Measures | Finding kinds | Build/wrap |
|---|---|---|---|
| `static.complexity` | Cyclomatic, cognitive (nesting-weighted), nesting depth, param count, length per symbol | `complexity.high`, `nesting.deep`, `params.many`, `function.long` | build |
| `static.crap` | `CC² × (1 − coverage)³ + CC`; wants `CoverageMap`; `--no-coverage` forces (CC only) | `crap.high` (≥30), `crap.watch` (5–30) | build |
| `static.clones` | Type 1/2/3 clones via normalized-AST subtree fingerprints + Jaccard; MinHash bucketing before pairwise | `clone.pair`, `clone.cluster` | build |
| `static.deps` | Module graph: afferent/efferent coupling, instability, abstractness, distance from main sequence, cycles, layering-rule violations from config | `dep.cycle`, `dep.layer-violation`, `dep.unstable-abstract` | build |
| `static.api-surface` | Public API diff vs base: removed/changed signatures, visibility changes | `api.breaking`, `api.widened` | build + wrap (`cargo-semver-checks`, `api-extractor` where present) |
| `static.dead-code` | Unreachable/unused symbols and exports | `dead.symbol`, `dead.export` | build |
| `static.analyze` | Runs the ecosystem's type checker at strictest config, linters, CodeQL/Semgrep if configured; normalizes to findings | `analyze.<tool>.<rule>` | wrap |

### 5.2 `tests.*` — test-suite strength (tier 2–3)

| ID | Measures | Finding kinds | Cost | Build/wrap |
|---|---|---|---|---|
| `tests.coverage` | Line/branch coverage; MC/DC where tooling supports; **per-test coverage map** when available | `coverage.uncovered-symbol`, `coverage.uncovered-branch` | standard | wrap (coverage producers) |
| `tests.mutation` | Killed / (killed + survived) over covered sites; coverage-gated; differential; per-test selection; schemata for compiled langs | `mutant.survived`, `mutant.timeout`, `mutant.uncovered`, `mutant.equivalent?` (low confidence) | standard (changed symbols) / deep (touched files) / exhaustive (tree) | build (Py, Go, Rust, TS) + wrap (PIT for JVM, Stryker optional) |
| `tests.quality` | SCRAP-style: assertion density, weak assertions, mock density, logic in tests, duplicated setup | `test.weak-assertion`, `test.no-assertion`, `test.mock-heavy`, `test.logic`, `test.duplicate-setup` | quick | build |
| `tests.weakening` | **Diff of the test suite itself** vs base: removed assertions, skipped/disabled tests, loosened matchers, rewritten snapshots, widened `any`-style matchers | `assertion.removed`, `test.skipped`, `matcher.loosened`, `snapshot.rewritten`, `test.deleted` | quick | build |
| `tests.flaky` | Rerun impacted tests N times with shuffled order, varied seeds, parallelism | `test.flaky`, `test.order-dependent` | standard | build over runner producers |

`tests.weakening` findings default to `severity: fail`. Agents routinely make tests pass by making tests weaker; this check watches the agent's own tail.

### 5.3 `gen.*` — generative/dynamic testing (tier 4)

| ID | Measures | Finding kinds | Cost | Build/wrap |
|---|---|---|---|---|
| `gen.property` | Runs existing property tests (Hypothesis, proptest/quickcheck, fast-check, jqwik) with seed and budget; reports counterexamples minimized; can scaffold property stubs from signatures (`--scaffold`, writes only with `--write`) | `property.falsified`, `property.missing` | standard/deep | wrap |
| `gen.fuzz` | Coverage-guided fuzzing (cargo-fuzz, libFuzzer, AFL++, Go native, Jazzer) with time budget; reproduces crashes | `fuzz.crash`, `fuzz.timeout`, `fuzz.target-missing` | deep/exhaustive | wrap |
| `gen.differential` | Old vs new (or impl vs registered oracle) on generated inputs; requires a **verified** oracle for tier 4, else tier 3 | `diff.divergence` (with minimized input, both outputs) | deep | build harness + wrap generators |
| `gen.metamorphic` | User-declared metamorphic relations in config (`sort(x) == sort(shuffle(x))`); runs on generated inputs | `metamorphic.violated` | deep | build |
| `gen.symbolic` | Concolic/search-based input generation targeted at surviving mutants and uncovered branches (KLEE/SymCC/Pynguin/EvoSuite where available) | `symbolic.input-found` (an input that kills mutant X) | exhaustive | wrap |

### 5.4 `conc.*` — concurrency and distributed correctness (tier 4–5)

| ID | Measures | Finding kinds | Cost | Build/wrap |
|---|---|---|---|---|
| `conc.race` | TSan, Go `-race`, Miri, Loom exhaustive interleavings for marked sections | `race.detected`, `ub.detected` | standard/deep | wrap |
| `conc.dst` | Deterministic simulation campaigns (madsim/turmoil/Shuttle harness; Antithesis via adapter) with seed ranges | `dst.failure` (seed + trace) | exhaustive | wrap |
| `conc.linearizability` | Record concurrent op histories, check with Elle/Porcupine against a model | `consistency.violation` (anomaly type, history excerpt) | deep/exhaustive | wrap |
| `conc.model-check` | Bounded model checking on implementation (Kani, CBMC, Stateright) | `model.counterexample` | exhaustive | wrap |

### 5.5 `spec.*` — specification-driven (tier 5–6)

| ID | Measures | Finding kinds | Cost | Build/wrap |
|---|---|---|---|---|
| `spec.contracts` | Runtime pre/post/invariant checking enabled during test and generative runs | `contract.violated` | standard | build (instrumentation hooks) + wrap |
| `spec.trace` | Implementation emits trace; validated as a behavior of a TLA+/Quint/P spec (TLC/Apalache) | `trace.diverged` (step, state, spec action) | exhaustive | wrap |
| `spec.prove` | Deductive verification (Dafny, Verus, Prusti, Creusot, Frama-C, SPARK) | `proof.failed`, `proof.obligation-open` | exhaustive | wrap |

### 5.6 `perf.*` — non-functional regression

| ID | Measures | Finding kinds | Cost | Build/wrap |
|---|---|---|---|---|
| `perf.bench` | Benchmarks vs baseline with statistical test (criterion, benchstat, hyperfine); allocation/syscall counts preferred to wall time | `perf.regression`, `perf.improvement` | deep | wrap |
| `perf.scaling` | Fit input size vs time; flag complexity-class changes | `perf.scaling-worse` | exhaustive | build |
| `perf.footprint` | Binary/bundle size, dependency count, license and vuln deltas | `footprint.grew`, `dep.vuln`, `dep.license` | quick | wrap |

### 5.7 `meta.*` — agent-facing meta-verification

| ID | Measures | Finding kinds | Cost |
|---|---|---|---|
| `meta.traceability` | Each requirement ID in the task spec (config or `--spec FILE`) maps to ≥1 test that fails without the change | `spec.untested-requirement` | standard |
| `meta.oracle-independence` | Labels each passing check by whether its oracle is independent of the change author (reference impl, spec, pre-existing tests) vs self-authored | `evidence.self-authored-only` | quick |

### 5.8 Depth → cost class mapping

| Depth | Wall target | Runs |
|---|---|---|
| `quick` | ~10 s | all `static.*` except `analyze` (if slow), `tests.quality`, `tests.weakening`, `perf.footprint`, `meta.oracle-independence`, impacted-test list |
| `standard` | minutes | + `tests.coverage`, `tests.mutation` (changed symbols only), `gen.property` (existing), `tests.flaky` ×3, `spec.contracts`, `conc.race`, `meta.traceability` |
| `deep` | tens of minutes | + `tests.mutation` (all touched files), `gen.fuzz`/`gen.property` with budgets, `gen.differential`, `gen.metamorphic`, `perf.bench`, `conc.linearizability` |
| `exhaustive` | unbounded | + whole-tree mutation, long fuzz/DST campaigns, `spec.trace`, `spec.prove`, `conc.model-check`, `gen.symbolic`, `perf.scaling` |

`--budget DURATION` overrides depth: the scheduler runs the highest-value work that fits (§9.6) and reports what it skipped.

## 6. CLI specification

### 6.1 Design rules

1. One output envelope, every verb (§7).
2. Non-interactive by construction. A needed decision → exit 2 with a finding naming the resolving flag. Output format auto-detects: TTY → text, pipe → JSON; `--format` overrides.
3. Scope, depth, and selection flags are identical across every analysis verb.
4. `--dry-run` on every verb. For `check`/`survey` it returns the plan with cost estimates; for writing verbs it returns the diff of what would be written.
5. Self-description for every verb, check, flag, finding kind, exit code, and schema. Human help is rendered from the same data.
6. Nothing is written outside `.cupel/` without `--write`.
7. Determinism: every run records seed, toolchain versions, git SHA; every finding's `repro` reproduces it.
8. Never truncate silently: `--limit/--offset` exist, envelope carries `truncated` and `total_findings`.
9. Paths relative to repo root; lines and columns 1-based; `end_line` always present.
10. `--no-install` is the default when stdout is not a TTY.

### 6.2 Verbs

```
cupel init        detect languages, project roots, test/coverage commands; write cupel.toml
cupel doctor      verify toolchain, tool producers, oracles, config; emit install_tool/configure actions
cupel describe    self-description of CLI, verb, check, finding kind, or schema
cupel check       run checks over a scope at a depth; evaluate gate
cupel survey      rank a whole tree without a baseline: hot spots, weakest-tested, clone clusters
cupel explain     full context and remediation for one finding, or one check result
cupel baseline    set | show | diff | export | import
cupel oracle      add | list | remove | verify
cupel tests       --impacted: which tests to run for this scope
cupel suppress    suppress a finding with --reason and --until
cupel serve       daemon + MCP server; warm caches; event streaming
```

Deliberately absent: `fix` (agent fixes), `report` (envelope is the report; `--format sarif|text` renders), per-technique verbs (`--only`).

`survey` is a separate verb from `check` because ranking without a baseline is a different contract from gating against one. `oracle` is a verb (not config-only) because `oracle verify` is required before a differential result may carry tier 4.

### 6.3 Global flags (every verb)

```
--format json|jsonl|text|sarif     # json when stdout is not a TTY
--dry-run                          # plan / preview; zero side effects
--describe                         # emit this verb's self-description, exit 0
--config PATH                      # default: nearest cupel.toml walking up from --cwd
--profile NAME                     # [profile.NAME] in cupel.toml
--cwd PATH                         # repo root; never rely on shell cwd
--quiet | --verbose                # stderr only; stdout shape never changes
--events PATH|stderr               # JSONL progress stream (§7.4)
--timeout DURATION                 # hard wall clock; exit 5 with partial results
--seed N                           # for generative checks; recorded in envelope
--limit N --offset N               # findings paging
--no-color
--no-install | --install           # auto-install producer tooling (default: --no-install unless TTY)
--write                            # permit writes outside .cupel/
```

### 6.4 Scope, depth, selection (shared by `check`, `survey`, `tests`, `explain`)

```
# what
--changed                  # diff vs merge-base with default branch (includes working tree)
--base REF --head REF      # explicit range; --head defaults to working tree
--paths P...               # files, dirs, globs (gitignore-aware)
--symbols S...             # ns::name or structural hash
--since-baseline           # symbols whose structural hash changed since baseline

# how much
--depth quick|standard|deep|exhaustive      # default: standard
--budget DURATION                           # overrides depth

# which
--only ID...   --skip ID...                 # check IDs or groups
--tier N+                                   # checks at or above tier N
--gate | --no-gate                          # default: --gate when a [gate] exists in config
--against baseline|REF|run:ID               # delta reference; default baseline if set
--oracle NAME                               # for gen.differential; default: first verified
--cases N                                   # generative case budget
```

Scope resolution: `--symbols` ⊂ `--paths` ⊂ `--changed`/range ⊂ tree. Multiple scope flags intersect. A `--changed` scope with no changes returns a `pass` envelope with `scope.symbols: 0` and exit 0.

### 6.5 Verb details

**`init`** — Detects project roots (Cargo workspace, go.work, npm/pnpm/yarn workspaces, uv/poetry/pip, Maven/Gradle, deps.edn), languages, test command, coverage command, benchmark command. Writes `cupel.toml` and `.cupel/.gitignore` only with `--write`. `--dry-run` shows the file. Never overwrites an existing `cupel.toml` without `--force`.

**`doctor`** — Checks binary version vs `required_version`, every selected tool producer's presence and version, oracle reachability (`oracle verify --all`), config validity, write access to `.cupel/`. Each problem is a finding with `action.kind: install_tool | configure`. Exit 3 if anything required for the configured checks is missing; 0 otherwise. `--fix --install` attempts installs.

**`describe`**

```
cupel describe                          # CLI: verbs, global flags, exit codes, schema URLs, config precedence
cupel describe <verb>                   # flags with types/defaults/enums, examples with expected output
cupel describe <check-id>               # what it measures, tier, cost, languages, prereqs, operators/params,
                                        #   finding kinds + how to act on each, known limitations, stability
cupel describe finding <kind>           # fields, evidence shape, action kinds it produces
cupel describe language <id>            # extensions, family, facts it produces, which checks apply and which are
                                        #   unsupported and why, known runners/coverage/bench tools, stability
cupel describe languages                # all languages, one line each
cupel describe fact <kind>              # schema, version, producers per language, checks that consume it
cupel describe matrix                   # checks × languages: supported | unsupported(fact) | unavailable(tool)
cupel describe schema run|finding|events|config|plugin   # JSON Schema (draft 2020-12)
cupel describe --agent                  # ~60 lines markdown for CLAUDE.md / AGENTS.md
cupel describe --all                    # everything, one JSON document
```

Every described entity carries `stability: stable | experimental`.

**`check`** — The workhorse. Resolves scope → selects checks by depth/`--only`/`--skip`/`--tier` → builds task DAG → runs under budget → evaluates gate → returns envelope. `--dry-run` returns the plan (§6.8).

**`survey`** — Same scope/depth machinery; no gate, no baseline required. Output adds `rankings`: top-N per dimension (worst CRAP, largest clone clusters, weakest mutation score, most-coupled modules, flakiest tests, dependency cycles). Intended as the first command on an unfamiliar repo.

**`explain`** — `explain <finding-id>` returns the finding plus: source excerpt with context, the mutant diff / counterexample / clone pair side-by-side, covering tests, nearest test file, a test skeleton (for `write_test` actions), and a `repro`. `explain <check-id>` returns the last run's full result for that check including per-symbol scores. Reads from the fact store; never re-runs.

**`baseline`** — `set [--from run:ID] --write` names the current (or given) run as baseline. `show` prints it. `diff [--against run:ID|REF]` compares two runs as an envelope with `summary.new/resolved`. `export FILE`/`import FILE` serialize a baseline as JSON (committable). Multiple named baselines: `--name`.

**`oracle`** — `add NAME --kind ref|command|spec --ref REF | --cmd "…" | --spec FILE --lang tla+|quint|p`. `verify NAME` runs a smoke comparison (ref/command: same output on N known inputs; spec: parses and model-checks a trivial behavior) and records `verified_at` + hash in the fact store. A `gen.differential` or `spec.trace` result against an unverified oracle is capped at tier 3 and carries `warn: oracle.unverified`.

**`tests --impacted`** — From the per-test coverage map and file-level import graph: the minimal test set covering the scope. Output: envelope with `tests: [{id, file, reason}]` and a ready-to-run `command` per runner. Falls back to file-level impact when no per-test map exists and says so.

**`suppress <finding-id> --reason TEXT [--until REF|DATE]`** — Writes to `.cupel/suppressions.toml` (committable). Suppressed findings still appear with `suppressed: true` and `severity: info`. Expired suppressions reactivate and emit `suppression.expired`.

**`serve [--socket PATH] [--mcp] [--watch]`** — Long-lived process holding parsed trees, fact-store connection, coverage map. CLI invocations auto-connect over the Unix socket if present (`CUPEL_NO_DAEMON=1` to bypass). `--mcp` exposes verbs as MCP tools with the same envelope. `--watch` incrementally reparses on file change.

### 6.6 Exit codes (every verb)

| Code | Name | Meaning |
|---|---|---|
| 0 | pass | Ran fully; gate passed or no gate |
| 1 | fail | Ran fully; gate failed |
| 2 | usage | Bad flags/config; envelope names the fix |
| 3 | environment | Missing tool/oracle/toolchain; `doctor` findings attached |
| 4 | unevaluable | Baseline test suite fails, build broken — scoring refused |
| 5 | partial | Timeout or budget hit; envelope has valid partial results |
| 6 | internal | Cupel bug; envelope includes bug-report payload |

Exit 4 is mandatory: if the baseline is red, mutation and coverage scores are meaningless and must not be produced.

### 6.7 Configuration (`cupel.toml`)

Precedence: flags > env `CUPEL_*` (e.g. `CUPEL_DEPTH`, `CUPEL_FORMAT`) > `--profile` > `cupel.toml` > defaults. The envelope echoes the effective config under `run.config`.

```toml
required_version = ">=0.4"
default_branch   = "main"
depth            = "standard"

[roots]                                  # monorepo; omit for single-root
rust   = { path = ".",      lang = "rust" }
web    = { path = "web",    lang = "typescript" }

[lang.rust]
test    = "cargo nextest run --workspace"
coverage = "cargo llvm-cov --lcov --output-path {out}"
bench   = "cargo bench --bench {name} -- --save-baseline {id}"
mutation = { strategy = "schemata", operators = "default" }

[lang.typescript]
test = "pnpm vitest run"
coverage = "pnpm vitest run --coverage --coverage.reporter=lcov"

[checks."static.clones"]
threshold = 0.82
min_lines = 4
min_nodes = 20

[checks."static.deps".layers]            # layering policy
order = ["domain", "app", "infra"]        # infra may depend on app may depend on domain, not reverse
map   = { domain = "src/domain/**", app = "src/app/**", infra = "src/infra/**" }

[checks."tests.mutation"]
timeout_factor = 10
exclude_tags   = ["no-mutate"]

[checks."tests.mutation".lang.python]    # per-language override of any check setting
operators = "extended"

[checks."static.complexity".lang.go]
cyclomatic_max = 15

[oracles.pg]
kind = "command"
cmd  = "psql -X -A -t -c {input}"

[gate.changed]                            # evaluated for --changed scopes
"tests.mutation.score"   = ">= 0.80"
"tests.weakening"        = "none"
"static.crap.max"        = "< 30"
"perf.bench.regression"  = "< 5%"
"static.api-surface"     = "no-breaking"

[gate.tree]                               # evaluated for whole-tree scopes
"tests.mutation.score"   = ">= 0.70"

[profile.ci]
depth = "deep"
format = "sarif"

[profile.loop]
depth = "quick"
budget = "20s"
```

Gate rule keys are `<check-id>.<metric>` or `<check-id>` (presence); operators `>= > <= < == none no-breaking`. Unknown keys are a usage error (exit 2) with the list of valid keys.

Configuration follows the three axes: `[checks."<id>"]` is language-neutral, `[checks."<id>".lang.<lang>]` overrides it for one language, and `[lang.<lang>]` chooses tools and producers for that language (`coverage = "coverage.py"` pins a producer when several exist). A check never reads `[lang.*]`; a producer never reads `[checks.*]`.

### 6.8 Dry run

`cupel check … --dry-run` returns:

```json
{
  "status": "plan",
  "scope": {"mode": "changed", "files": 4, "symbols": 11, "roots": ["."]},
  "would_run": [
    {"id": "static.crap", "targets": 11, "est_ms": 900, "needs": ["coverage(cached)"]},
    {"id": "tests.mutation", "targets": 34, "est_ms": 140000, "needs": ["coverage(fresh)", "per-test-map"],
     "note": "19/34 sites cached as killed from run_01J…; 15 will execute; schemata build ×1"},
    {"id": "gen.differential", "targets": 3, "est_ms": 30000, "oracle": "pg", "cases": 2000}
  ],
  "would_skip": [
    {"id": "conc.dst", "reason": "no harness configured", "see": "cupel describe conc.dst"}
  ],
  "would_install": [],
  "would_write": [".cupel/runs/<id>.json", ".cupel/cache/coverage/rust/lcov.info"],
  "est_total_ms": 171000
}
```

For `init`, `baseline set`, `suppress`, `oracle add`: `would_write` carries a unified diff per file.

### 6.9 Use-case walkthroughs

**Validate a change (main loop)**

```
cupel tests --impacted --changed                   # seconds; run these with the normal runner
cupel check --changed --depth quick                # structure, clones, test-weakening
cupel check --changed --depth standard --gate      # + coverage, diff-scoped mutation
  → act on next_actions[0]; repeat until exit 0
cupel check --changed --depth deep --budget 10m    # before declaring done
cupel check --changed --gate --format text         # PR summary
```

**New or unfamiliar repo**

```
cupel init --dry-run        → review detection
cupel init --write
cupel doctor                → exit 3 with install_tool actions if needed
cupel survey --depth standard
cupel baseline set --write
```

**One flavor, one place, as deep as it goes**

```
cupel check --only tests.mutation --symbols pando::wal::Writer::append --depth exhaustive
cupel check --only gen.differential --paths src/translate --oracle pg --cases 50000 --seed 7
cupel explain fnd_7c2e1a
```

**Gate as done-criterion.** Agent instruction reduces to: *do not stop until `cupel check --changed --depth standard --gate` exits 0.*

## 7. Output contract

### 7.1 Envelope

Every verb returns this shape on stdout (one JSON document; `jsonl` emits one finding per line after a header line).

```json
{
  "cupel": {"version": "0.4.0", "schema": "https://cupel.dev/schema/v1/run.json"},
  "run": {
    "id": "run_01JA…", "verb": "check",
    "started": "2026-10-10T16:31:04Z", "duration_ms": 48211,
    "git": {"sha": "9f1c…", "dirty": false, "base": "a81e…", "branch": "feat/limit-pushdown"},
    "scope": {"mode": "changed", "files": 4, "symbols": 11, "roots": ["."], "languages": ["rust"]},
    "depth": "standard", "budget_ms": null, "seed": 42,
    "toolchain": {"rust": "1.93.0", "cargo-llvm-cov": "0.6.12", "cargo-nextest": "0.9.80"},
    "config": {"effective": {"…": "…"}, "sources": ["flags", "cupel.toml"]},
    "baseline": {"id": "run_01J9…", "name": "default"}
  },
  "status": "fail",
  "exit_code": 1,
  "checks": [
    {"id": "tests.mutation", "status": "fail", "tier": 3, "cost": "standard",
     "score": {"value": 0.71, "unit": "killed_ratio", "n": 34},
     "delta": {"vs": "baseline", "value": -0.12},
     "findings": 6, "duration_ms": 41000, "skipped": null, "oracle": null}
  ],
  "findings": [ "…see 7.2…" ],
  "summary": {
    "findings": {"fail": 6, "warn": 9, "info": 22},
    "new": 4, "resolved": 2, "suppressed": 1,
    "gate": {"result": "fail", "evaluated": "gate.changed",
             "rules": [{"rule": "tests.mutation.score >= 0.80", "actual": 0.71, "result": "fail"}]}
  },
  "next_actions": [
    {"priority": 1, "finding": "fnd_7c2e1a", "kind": "write_test", "summary": "…"},
    {"priority": 2, "kind": "rerun", "summary": "Run differential vs oracle 'pg'",
     "command": "cupel check --changed --depth deep --only gen.differential"}
  ],
  "truncated": false, "total_findings": 37
}
```

`status` ∈ `pass | fail | partial | error | plan`. `partial` and `error` are distinct from `fail`; agents must branch on them differently.

### 7.2 Finding

```json
{
  "id": "fnd_7c2e1a",
  "check": "tests.mutation", "kind": "mutant.survived",
  "severity": "fail", "confidence": 0.95, "tier": 3,
  "symbol": {"display": "exonym::rewrite::Planner::push_limit", "hash": "sh_4b…", "lang": "rust"},
  "location": {"file": "src/rewrite/planner.rs", "line": 142, "col": 21, "end_line": 142, "end_col": 23},
  "title": "Mutant survived: `<=` → `<` at planner.rs:142",
  "detail": "No test distinguishes limit == offset from limit < offset.",
  "evidence": {"mutation": {"from": "<=", "to": "<", "operator": "relational"},
               "tests_run": 118, "covering_tests": ["rewrite::limit_tests::pushes_through_sort"]},
  "action": {"kind": "write_test", "priority": 1,
             "summary": "Add a test where limit == offset asserting the boundary.",
             "hints": {"nearest_test_file": "tests/rewrite/limit_tests.rs",
                       "covering_tests": ["pushes_through_sort"]}},
  "repro": "cupel check --only tests.mutation --symbols sh_4b… --seed 42",
  "suppress": "cupel suppress fnd_7c2e1a --reason '…'",
  "new_since_baseline": true, "suppressed": false
}
```

**Finding ID** = `fnd_` + first 24 bits-hex of `blake3(check ∥ kind ∥ symbol.hash ∥ site_hash)` where `site_hash` is the structural position of the site within the symbol (not the line number). Stable across reformatting and relocation.

**`action.kind`** is a closed enum:

`write_test · strengthen_assertion · restore_test · refactor_extract · reduce_complexity · break_cycle · fix_bug · add_contract · add_property · investigate_flake · rerun · configure · install_tool · verify_oracle · review` (last resort when no mechanical action exists).

**`evidence`** is check-specific but schema'd per finding kind (`describe finding <kind>`). Rules: counterexamples are minimized; both sides of a divergence are included; clone pairs include both spans; traces are bounded to the relevant window with `truncated` markers.

### 7.3 Text and SARIF rendering

`--format text` renders the envelope: status line, gate result, findings grouped by severity then check, each with location, title, action summary, and repro; `next_actions` last. No information present in JSON is omitted except `evidence` bodies (summarized to one line, full via `explain`). `--format sarif` emits SARIF 2.1.0: one `rule` per finding kind, `level` from severity, `partialFingerprints.cupel/findingId`, `fixes` empty (Cupel does not fix), properties carrying tier/confidence/action.

### 7.4 Event stream (`--events`)

JSONL to stderr or a file; stdout stays reserved for the final envelope.

```json
{"ev":"plan","run":"run_01JA…","checks":7,"est_ms":180000}
{"ev":"check.start","id":"tests.mutation","targets":34}
{"ev":"finding", …full finding…}
{"ev":"progress","id":"tests.mutation","done":12,"total":34,"eta_ms":61000}
{"ev":"check.end","id":"tests.mutation","status":"fail","duration_ms":41000}
{"ev":"skip","id":"gen.fuzz","reason":"budget"}
{"ev":"run.end","status":"fail","exit_code":1}
```

Findings stream as discovered so an agent can act before the run completes. `serve` exposes the same stream over the socket/MCP.

## 8. Architecture

### 8.1 Language: Rust

- Single static binary, zero runtime deps — the only thing that reliably works when an agent drops it into an arbitrary repo.
- tree-sitter's canonical bindings and grammar ecosystem are Rust.
- Real parallelism (`rayon` for CPU-bound analysis, `tokio` for process orchestration and the daemon).

Go is a defensible second choice. Python is ruled out (startup, venvs, the thing you least want an agent debugging).

### 8.2 Crate layout

```
cupel-cli        clap (derive) arg parsing; format rendering; exit codes; daemon auto-connect
cupel-core       fact types + Fact/Producer/Check traits + registries + support matrix; scope resolution;
                 fact DAG + scheduler; budget; cache keys; Envelope/Finding types (schemars)
cupel-lang       Language trait; generic query-driven producers; one directory per language holding
                 tree-sitter queries (.scm) and tables (.toml) plus thin Rust glue; grammars compiled in
cupel-store      SQLite fact store (rusqlite, WAL); migrations; queries
cupel-plugin     plugin protocol types; external-process host for check and producer plugins
cupel-adapt      tool producers: coverage parsers (lcov, cobertura, jacoco-xml, go-coverprofile, istanbul-json),
                 runners (nextest, cargo test, go test -json, pytest, vitest/jest, mvn/gradle, clj),
                 bench (criterion, benchstat, hyperfine); the per-language toolchain table
cupel-mutate     mutation engine: a producer of MutationOutcomes (overlays, schemata, gating, selection)
cupel-checks     native checks, one module per check; depends on cupel-core only
cupel-serve      daemon, Unix socket protocol, MCP server, file watcher
cupel-fixtures   per-language fixture repos keyed to per-fact conformance inventories + golden envelopes
                 (dev-dependency)
```

Cargo workspace; the CLI binary links all native checks and producers. `cupel-core` owns the types; JSON Schemas are generated from them with `schemars` at build time and embedded, so `describe schema` and the serializer cannot drift.

The dependency graph enforces the extension model: `cupel-checks` depends on `cupel-core` and nothing else — it cannot name tree-sitter, a language, or a process. `cupel-lang`, `cupel-adapt`, and `cupel-mutate` depend on `cupel-core` and never on `cupel-checks` or each other. Only `cupel-cli` sees everything.

### 8.3 Extension model: facts, producers, checks

Cupel extends along three axes that multiply: verification techniques (checks), languages, and external tools. If any check knew about any language or tool, N checks × M languages × T tools would be the implementation cost, and every addition on one axis would mean work on the others. The architecture collapses the product into a sum by routing everything through a small, slow-changing vocabulary of **facts**.

#### 8.3.1 The rule

- **Checks consume facts.** A check is a pure function from facts and configuration to findings and a score. It never sees a syntax tree, a language, a file system, or a process. `cupel-checks` cannot compile against anything language-specific (§8.2).
- **Producers make facts.** Three kinds: *language producers* (syntactic facts from the CST, mostly declarative), *tool producers* (facts from an external tool's output), and *engines* (facts from executing something — the mutation engine, the flake re-runner). Producers may need other facts.
- **Facts are semantic, typed, versioned, language-tagged, content-addressed, and cached.** A fact encodes what a construct *means*, not how it is spelled: `TestFrames` says `Assertion { kind: Equality, strength: Weak }`, not `assert.equal`. The producer owns the idiom-to-semantics mapping; the check owns what the semantics mean for quality.

Consequences:

| Adding a… | Work | Touches |
|---|---|---|
| check over existing facts | one module in `cupel-checks` + goldens | nothing language- or tool-specific; works for every language that produces its facts |
| language | producers for the facts it can provide + fixtures | `cupel-lang/langs/<id>/`, `cupel-fixtures/repos/<id>/`, one toolchain-table row |
| tool | one producer | `cupel-adapt`, one toolchain-table cell |
| **fact kind** | type + schema + conformance inventory + producers for each language that can supply it | the vocabulary — this is the only operation that costs across axes, and it requires an ADR |

A language that lacks a producer for some fact simply does not support the checks that need it, and the support matrix says so. Nothing breaks; coverage gaps are visible, not silent.

#### 8.3.2 Fact catalog (v1)

Syntactic facts come from language producers and are keyed by file content hash and producer version. Execution facts come from tools and engines and are keyed by their input facts and the toolchain hash.

| Fact | Content (semantic) | Produced by | Consumed by |
|---|---|---|---|
| `Symbols` | functions/methods: display name, namespace, visibility, span, body ref, `lang` | language | everything |
| `Decisions` | per symbol: `Decision { kind: Branch \| Loop \| BooleanOp \| Case \| Catch \| EarlyReturn \| Ternary, nesting }` | language | `static.complexity`, `static.crap` |
| `NormTree` | normalized AST per symbol; yields `symbol.hash` | language | identity everywhere, `static.clones` |
| `MutationSites` | per symbol: `Site { operator: Relational \| Arith \| Boolean \| Literal \| …, set: Default \| Extended, from, to[], site_hash }` | language | mutation engine |
| `TestFrames` | test cases: `Assertion { kind: Equality \| Truthiness \| NotNull \| Throws \| Snapshot \| Custom, strength }`, mocks, skip markers, setup blocks, framework | language | `tests.quality`, `tests.weakening`, `tests.flaky` |
| `ModuleGraph` | resolved import edges per root: `File \| External \| Unresolved`, with relative-import flags | language | `static.deps`, `static.dead-code`, impact fallback, overlays |
| `ApiSurface` | public items with normalized signatures | language | `static.api-surface` |
| `Roots` | symbols live by convention (entry points, exports, tests, framework hooks) | language | `static.dead-code` |
| `TestConventions` | test files for a source file; is-test predicate | language | `write_test` hints, scope |
| `ProjectInfo` | detected roots: path, language, build files, workspace members, dirs to skip — no commands; the CLI joins it with the toolchain table (§8.7, ADR-037) | language | `init`, `doctor`, toolchain selection |
| `TestResults` | per test: outcome, duration | runner tool | baseline/exit 4, flake engine, mutation engine |
| `TestReruns` | per rerun: order, seed, parallelism, executed order; per test per rerun: outcome (ADR-078) | flake engine | `tests.flaky` |
| `CoverageMap` | line + optional branch hits; `branches: Option<_>` says whether the producer measures branches | coverage tool | `tests.coverage`, `static.crap`, mutation engine |
| `PerTestMap` | `test_id → lines` | coverage tool | `tests --impacted`, mutation engine |
| `MutationOutcomes` | per site: killed/survived/timeout/uncovered, killing tests, duration | mutation engine · PIT/Stryker tool | `tests.mutation` |
| `BenchSamples` | per benchmark and unit: samples (costs, lower is better) at head and, under a range, at the base commit, which the producer measures itself (ADR-106) | bench tool | `perf.bench` |
| `PropertyResults` | per property: passed/falsified, minimized counterexample, seed | property tool | `gen.property` |
| `FuzzResults` | per fuzz target, fuzzed with an execution budget and the run's seed: how it ended, the crash input with its reproducer, executions and coverage reached; the roots searched (ADR-107) | fuzz tool | `gen.fuzz` |
| `RaceReports` | data races and undefined behavior a detector saw running the tests, each with its test, masked message, and stacks inside the repository; each root's run (ADR-105) | race tool | `conc.race` |
| `AnalyzerReport` | per analyzer: ran, missing, unconfigured, or failed; its results normalized to SARIF's rule, level, message, location (ADR-102) | linter/type-checker tool | `static.analyze` |
| `DivergenceResults` | per generated input: both outputs, minimized | differential engine | `gen.differential`, `gen.metamorphic` |
| `Footprint` | dependencies from lockfiles and manifests (name, version, direct, license, where listed), whether licenses were read, OSV vulnerabilities and artifact sizes when configured, the range's base dependencies; read offline (ADR-103) | lockfile readers | `perf.footprint` |

P7 adds `Traces`, `Histories`, `ProofResults`, `ContractViolations` under the same rules. Facts carry completeness flags rather than languages carrying capability flags: a `CoverageMap` with `branches: None` makes `tests.coverage` skip branch findings with that reason; whether a language has a per-test map is simply whether a `PerTestMap` producer is registered for it.

#### 8.3.3 Producer contract

```rust
pub trait Fact: Serialize + DeserializeOwned + JsonSchema + Send + Sync {
    const KIND: FactKind;
    const VERSION: u32;                       // bump on incompatible shape change; invalidates downstream cache
}

pub trait Producer<F: Fact>: Send + Sync {
    fn id(&self) -> ProducerId;               // "lang.go", "tool.pytest-cov", "engine.mutation"
    fn languages(&self) -> Langs;             // Any | Only(&[LangId]) | Family(LangFamily)
    fn needs(&self) -> &[FactKind];           // the mutation engine needs MutationSites, CoverageMap, TestResults
    fn cost(&self) -> CostClass;
    fn availability(&self, ctx: &Ctx) -> Availability;   // Ready | MissingTool { name, install } | Unconfigured { key }
    fn produce(&self, ctx: &ProduceCtx, scope: &Scope, sink: &mut dyn EventSink) -> Result<F>;
}
```

The registry maps `(FactKind, LangId) → ordered producers`. Selection order: a config pin in `[lang.<lang>]` > toolchain-table default order > first `Ready`. `doctor` reports every `(fact, lang)` pair required by the configured checks that has no `Ready` producer, with the install or configure action.

#### 8.3.4 Check contract

The check `Manifest` declares `needs: &[FactKind]` (required) and `wants: &[FactKind]` (used for enrichment when present — `tests.mutation` wants `TestConventions` for the nearest-test-file hint). The check's `run` receives a `Facts` accessor that serves exactly the declared kinds; requesting anything else is an error that the matrix test catches by running every check with only its declared facts. `CheckCtx` offers config, a scratch dir, a cancellation token, and read access to the store for history — and no API to spawn a process or read source outside the facts. A check's language applicability is never declared; it is derived (§8.3.5).

#### 8.3.5 Support matrix

For every `(check, lang)`:

- `supported` — every fact in `needs` has at least one registered producer for `lang`.
- `unsupported(fact)` — some needed fact has no producer for `lang`. Permanent until someone writes one.
- `unavailable(tool)` — supported, but the selected producer's tool is missing here. `doctor` fixes this.

`cupel describe matrix` renders the table with reasons; the README's support matrix is generated from it. In an envelope, a check that did not run for a language records `skipped: { reason: "unsupported" | "unavailable" | "budget" | …, fact, lang, see: "cupel describe matrix" }`. Multi-language scopes run each check once per language present, except where the fact is cross-language by nature (`NormTree` comparisons within a family; `ModuleGraph` per root).

#### 8.3.6 Versioning and caching

Fact cache key: `(kind, fact version, producer id + version, input hashes, toolchain_hash, config slice hash)`. Check result cache key: `(check id + version, hashes of consumed facts, config slice hash)`. Bumping a fact's `VERSION` invalidates exactly its downstream. Finding IDs (§7.2) depend on none of these and survive every upgrade.

#### 8.3.7 Conformance per fact kind

Each fact kind owns a conformance inventory `cupel-fixtures/conformance/<fact>.toml` listing the constructs a producer must recognize (`decisions`: `if`, `loop`, `boolean-op`, `early-return`, `case`, `ternary`, `catch`; `mutation-sites`: `relational`, `equality`, `arith-add-sub`, `arith-mul-div`, `boolean-op`, `boolean-literal`, `zero-one`; `test-frames`: `case`, `assertion-equality`, `assertion-truthiness`, `no-assertion`, `skip`, `mock`, `setup`; and so on). A language producer is admitted for fact `F` only when every construct in `F`'s inventory is mapped in the language's `inventory.toml` to code that exhibits it — or to `not-applicable = "<reason>"` — and the producer detects each one.

The **matrix test** enumerates the registries rather than any hand-written list: for every `supported (check, lang)` a golden `golden/<check>/<lang>.json` must exist and pass; for every `unsupported` pair a test asserts the envelope's `skipped` reason. Adding a producer that flips a pair to `supported` therefore fails the build until its golden exists — which is the point.

#### 8.3.8 Plugins across the axes

External processes can be checks (consume facts, emit findings) or producers (emit a fact — a PIT wrapper producing `MutationOutcomes` for Java is a producer plugin, and `tests.mutation` never learns it exists). Both declare the same manifests as native code and enter the same registries; §8.6.

### 8.4 Language layer

A language is a grammar, a declarative `LanguageSpec`, and the set of producers it registers. Adding a language is mostly **writing data** — tree-sitter queries and tables — that generic producers interpret, with Rust code only where a language is genuinely idiosyncratic (module resolution, project detection, normalization quirks, schemata).

#### 8.4.1 The trait

```rust
pub trait Language: Send + Sync {
    fn id(&self) -> LangId;                       // "javascript" | "typescript" | "rust" | "go" | "python" | …
    fn family(&self) -> LangFamily;               // clone comparability across languages (javascript+typescript = ecma)
    fn extensions(&self) -> &[&str];
    fn grammar(&self, path: &Path) -> tree_sitter::Language;   // one language may own several grammars (js/jsx, ts, tsx)
    fn spec(&self) -> &LanguageSpec;              // queries + tables consumed by the generic producers
    fn stability(&self) -> Stability;
    /// Register this language's producers. The generic query-driven producers cover most syntactic facts;
    /// a language registers a hand-written producer only where queries cannot express the fact.
    fn register(&self, reg: &mut ProducerRegistry);
}
```

The trait is deliberately small and closed. Language knowledge that checks need does not arrive as new trait methods; it arrives as new facts with new producers (§8.3.1), so the trait does not grow with the check catalog.

Generic producers in `cupel-lang`, each driven entirely by `LanguageSpec`: `QuerySymbols → Symbols`, `QueryDecisions → Decisions`, `TableNormalize → NormTree`, `QueryMutationSites → MutationSites`, `QueryTestFrames → TestFrames`, `QueryImports` (raw edges feeding `ModuleGraph`), `QueryApi → ApiSurface`, `QueryRoots → Roots`, `GlobTestConventions → TestConventions`. Hand-written per language: `ModuleGraph` resolution (Node resolution with `package.json` `exports` and `tsconfig` `paths`; Rust `mod` tree and workspace members; Go module paths; Python packages), `ProjectInfo`, and a schemata strategy registered with the mutation engine for compiled languages.

`Symbol.hash` = `blake3(lang ∥ serialize(NormTree))`. This is the join key for metrics, coverage, mutants, baselines, and finding IDs. `NormTree` serialization is language-independent so that symbols in the same family can be compared.

#### 8.4.2 Declarative core: `LanguageSpec`

Each language lives in `cupel-lang/src/langs/<id>/`:

```
langs/go/
├── mod.rs              # Language impl: identity, register(), hand-written producers (ModuleGraph, ProjectInfo)
├── queries/
│   ├── symbols.scm     # captures: @symbol.name @symbol.body @symbol.ns @symbol.visibility
│   ├── decisions.scm   # captures: @decision.<kind> — kinds map to weights in tables.toml
│   ├── mutations.scm   # captures: @mut.<operator> — operators map to replacements in tables.toml
│   ├── tests.scm       # captures: @test.case @test.assertion @test.mock @test.skip @test.setup
│   ├── imports.scm     # captures: @import.specifier @import.names
│   └── api.scm         # captures: @api.item @api.signature
└── tables.toml         # decision weights, operator sets (default/extended) with replacements,
                        # normalize rules (node kinds to erase/mark), weak-assertion patterns, test-file globs
```

Queries are embedded with `include_str!` and compiled against the grammar in a test, so a query that no longer matches a grammar version fails at `make test`, not at runtime. Capture names are the contract between a query and its generic producer: `decisions.scm` must capture `@decision.branch`, `@decision.loop`, … and `tables.toml` gives each kind its cyclomatic and cognitive weights. A language replaces a generic producer with a hand-written one only when a construct cannot be expressed as a query (Python's implicit `self` scoping in normalization, Rust macro bodies, Go's method receivers), and the replacement must still pass the same conformance inventory.

There is no per-language capability struct. What a language supports is exactly the set of facts it has producers for, plus the completeness flags inside the facts those producers emit (§8.3.2). `cupel describe language <id>` renders identity, the facts it produces and by which producer, the checks that are therefore supported or unsupported, and the toolchain-table row (§8.7). The envelope's `scope.languages` lists which languages were active in a run.

#### 8.4.3 Launch languages and family rules

| LangId | Family | Grammars | Notes |
|---|---|---|---|
| `javascript` | `ecma` | tree-sitter-javascript (`.js .jsx .mjs .cjs`) | ESM and CJS imports; Node resolution for `resolve_import`/`overlay_deps` |
| `typescript` | `ecma` | tree-sitter-typescript `typescript` (`.ts .mts .cts`), `tsx` (`.tsx`) | Shares queries/tables with `javascript` via a common module; `tsconfig.json` `paths` honored in resolution |
| `rust` | `rust` | tree-sitter-rust | Module tree from `mod` declarations and `Cargo.toml` workspace members; schemata first target |
| `go` | `go` | tree-sitter-go | Package-level imports; `go.mod`/`go.work`; per-test coverage by `-run` narrowing; native fuzz and `-race` |
| `python` | `python` | tree-sitter-python | Package resolution via `__init__.py`, `pyproject`/`setup.cfg` roots; pytest-cov contexts for per-test coverage |

A JavaScript project is a root with `package.json` and no `tsconfig.json`; a TypeScript project has both. Mixed trees get both languages active and clones are detected across them because they share a family. `[lang.javascript]` and `[lang.typescript]` are both valid config sections; where only one is given it applies to the whole `ecma` family.

Java and Clojure followed launch (§12, P6.4: ADR-108, ADR-109) using the same contract; Java mutation can delegate to PIT through a producer plugin for `MutationOutcomes`, pinned with `[lang.java] mutator = "plugin.pit"`, which `tests.mutation` consumes without knowing the difference (ADR-110).

Per-language decision-point and mutation-operator tables follow crapper/mutator's definitions (§3) as the default operator set `default`; an `extended` set adds statement deletion, return-value replacement, constant replacement, boundary (`+1/-1`), and negation. `extended` is opt-in because it increases equivalent-mutant rate.

#### 8.4.4 Adding a language

Conformance is owned by fact kinds (§8.3.7), so a language's fixture repo under `cupel-fixtures/repos/<lang>/` carries one `inventory.toml` mapping every construct from every fact inventory to the file and symbol that exhibits it, or to `not-applicable = "<reason>"` (Go has no ternary; Rust has no `null`). The checklist:

1. Add the grammar crate to `[workspace.dependencies]`.
2. Create `langs/<id>/` with queries and `tables.toml`; implement `Language`; in `register()` attach the generic producers and hand-write `ModuleGraph` and `ProjectInfo`.
3. Create `cupel-fixtures/repos/<id>/` with an `inventory.toml` covering every construct in every fact inventory the language will produce.
4. Add the toolchain-table row (§8.7): runner producers, coverage producer and format, bench/property/fuzz/race tools.
5. Run the conformance suite until green. The support matrix now shows which checks the language supports; the matrix test demands a golden for each — generate and review them.
6. `cupel describe language <id>` and `describe matrix` are accurate by construction; regenerate the README matrix.

No change to `cupel-core`, `cupel-checks`, or the CLI is required. If adding a language *does* require one, that is a defect in the extension model and gets an ADR and a fix before the language lands. Conversely, a language may launch with partial fact coverage (no `ApiSurface` producer yet, say): the checks that need it show `unsupported(ApiSurface)` for that language, and adding the producer later flips them on without touching anything else.

Languages are compiled in. A stdio language plugin is not offered because parsing is per-file and per-keystroke in the daemon, where process round-trips are too slow; dynamically loaded grammars are a possible later addition (§13).

### 8.5 Check trait (native)

```rust
pub trait Check: Send + Sync {
    fn manifest(&self) -> &Manifest;               // id, version, tier, cost, needs: [FactKind], wants: [FactKind],
                                                   // flags, finding kinds + action mapping, metrics (gate keys),
                                                   // stability, docs
    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan>;          // targets + cost estimate (== --dry-run)
    fn run(&self, ctx: &CheckCtx, facts: &Facts, plan: &Plan, sink: &mut dyn EventSink) -> Result<CheckResult>;
}
```

`Facts` serves exactly the kinds declared in `needs` and `wants` (§8.3.4). `CheckCtx` provides: resolved config for this check (with the per-language override already merged), read-only store access for history, cancellation token, scratch dir under `.cupel/work/<run>/`. It has no file-system, process, or language API: a check cannot do anything but compute over facts. `EventSink` receives findings and progress; the core serializes to `--events` and accumulates the envelope.

All execution lives in producers. `tests.mutation` does not run mutants — the mutation engine produces `MutationOutcomes` and the check scores them and writes findings. `tests.flaky` does not re-run tests — a flake engine reruns the baseline's tests and produces `TestReruns` (ADR-078). This is what lets a PIT wrapper or a future Stryker wrapper feed the same check, and what keeps the scheduler able to account for every process spawned.

### 8.6 Plugin protocol (external process)

Discovery: entries under `[plugins]` in `cupel.toml`, plus any `cupel-check-*` or `cupel-produce-*` executable on `$PATH` (disabled by `--no-path-plugins`).

```
<plugin> describe            → stdout: Manifest JSON (same schema as native; kind: check | producer)
<plugin> plan   < RunRequest → stdout: Plan JSON
<plugin> run    < RunRequest → stdout: JSONL events (same as §7.4) ending with
                               {"ev":"result", …CheckResult…}   for a check plugin
                               {"ev":"fact", kind, version, …}  for a producer plugin
```

A **check plugin** declares `needs`/`wants` like a native check and receives the facts it declared as JSON files; it is language-agnostic for free. A **producer plugin** declares `provides: FactKind`, `languages`, and `needs`, and must emit a fact validating against that kind's schema at the declared version; it enters the producer registry beside native producers and is selected by the same rules (§8.3.3).

`RunRequest` carries: protocol version, scope (files + symbols with hashes and spans), depth, budget, seed, resolved config slice, paths to the requested fact files, scratch dir, toolchain info. Plugins must be side-effect-free outside the scratch dir unless `write: true` is in the request.

The host validates the manifest against the schema, enforces the budget (SIGTERM then SIGKILL), rejects findings whose `kind` is not declared in the manifest, and rejects facts that fail schema validation. A plugin that exits non-zero without a `result`/`fact` event yields `status: error`, never `pass`.

Tier 3 (WASM via wasmtime, same protocol over WASI stdio) is reserved for pure-analysis plugins; not v1.

### 8.7 Tool producers and the toolchain table

Tool wrappers are producers (§8.3.3). Each splits into a **format parser** (language-neutral: LCOV is LCOV whoever wrote it) and an **invocation** (how to run the tool for a language and where its report lands), so one parser serves every tool that emits the format:

```rust
pub trait Runner: Producer<TestResults> { fn list(&self, ctx) -> Result<Vec<TestId>>;
                                          fn run(&self, ctx, selection: TestSelection, env: Env, timeout) -> Result<TestResults>; }
pub trait Coverage: Producer<CoverageMap> + Producer<PerTestMap> /* when supported */ { fn parse(&self, path) -> Result<CoverageMap>; }
pub trait Bench: Producer<BenchSamples> {}
```

Engines use the same contract: `cupel-mutate` is `Producer<MutationOutcomes>` with `needs = [MutationSites, CoverageMap, TestResults]` and `wants = [PerTestMap, ModuleGraph]`, and consumes a `Runner` through the registry rather than spawning anything itself.

`CoverageMap` is line- and optionally branch-level (`branches: None` when the tool cannot measure them, as for Go's coverprofile). `PerTestMap` is `test_id → lines`. Sources: nextest + llvm-cov profiles per test (Rust), `go test -coverprofile` per package with `-run` narrowing (Go), pytest-cov `--cov-context=test` (Python), JaCoCo per-test via agent sessions (Java). JS/TS has no cheap per-test map, so no `PerTestMap` producer is registered for `ecma`; `tests --impacted` falls back to `ModuleGraph` impact and says so.

Producers are registered per language in a **toolchain table** in `cupel-adapt`, keyed by `LangId`. Each row lists the runners Cupel knows for that language (with detection rules: lockfile, config file, dependency name), the coverage producers and the report formats they emit, bench producers, and the property/fuzz/race tools. The row is the registry's default order for that language. `init` consults the table to choose defaults; `doctor` consults it to know what to verify; `describe language` renders it. Launch rows:

| LangId | Runners | Coverage producer → format | Bench | Per-test map | Analyzers (ADR-102) |
|---|---|---|---|---|---|
| `javascript` / `typescript` | vitest, jest, mocha, node:test | vitest/jest → Istanbul JSON, c8 → lcov | hyperfine, tinybench | file-level fallback | tsc `--strict` (TypeScript), ESLint (with a flat config) |
| `rust` | cargo test, cargo nextest | cargo llvm-cov → lcov | criterion | nextest + llvm-cov per test | clippy |
| `go` | go test -json | go test -coverprofile → coverprofile | go test -bench + benchstat | `-run` narrowing per test | go vet, staticcheck |
| `python` | pytest, unittest | pytest-cov / coverage.py → lcov, Cobertura XML | pytest-benchmark, hyperfine | `--cov-context=test` | mypy `--strict`, ruff |
| `java` (ADR-108) | Maven Surefire, Gradle → JUnit XML | JaCoCo through Maven or Gradle → JaCoCo XML | — | none (module-graph fallback) | — |
| `clojure` (ADR-109) | `clojure.test` through the Clojure CLI → JUnit XML | Cloverage → lcov | — | none (module-graph fallback) | — |

Producers ingest existing reports when fresher than the sources (mtime and content hash), avoiding regeneration.

### 8.8 Fact store

SQLite at `.cupel/cupel.db`, WAL, single writer (the daemon when present, else the CLI process holding a lock).

```
facts        (kind, version, cache_key PK, producer, lang, run_id, payload JSON/BLOB)   -- every fact, by its §8.3.6 key
symbols      (hash PK, lang, file, name, ns, visibility, start_line, end_line, last_seen_run)
metrics      (symbol_hash, check, key, value REAL, run_id)
coverage     (run_id, file, line, hits, branch_id?, test_id?)
mutants      (symbol_hash, site_hash, operator, outcome, suite_hash, toolchain_hash, run_id, duration_ms)
test_results (run_id, test_id, outcome, duration_ms)
runs         (id PK, verb, started, duration_ms, git_sha, base_sha, depth, seed, config_hash, toolchain_hash, envelope JSON)
findings     (id, run_id, check, kind, symbol_hash, severity, json)
baselines    (name PK, run_id)
oracles      (name PK, kind, spec JSON, verified_at, verified_hash)
suppressions mirrored from .cupel/suppressions.toml
```

`facts` is the generic cache for every fact kind, so a new fact kind needs no migration. The typed tables (`symbols`, `coverage`, `mutants`, `test_results`) are query indexes over the facts that `explain`, `survey`, and the differential mutation path read at fine grain; they are populated from the corresponding facts, never written independently.

Cache validity for a mutant outcome: `(symbol_hash, site_hash, suite_hash, toolchain_hash)` unchanged ⇒ reuse. `suite_hash` = hash of the structural hashes of all test symbols that cover the site (or of the test files when no per-test map). Scores are recomputed from the store on every run, so a cached kill contributes without re-execution.

`.cupel/` is gitignored except `suppressions.toml` and any `baseline export` outputs the user commits.

### 8.9 Scheduler

1. Resolve scope → symbol set.
2. Select checks (depth, `--only/--skip`, `--tier`) and, per language in scope, drop pairs the support matrix marks `unsupported` or `unavailable`, recording each as `skipped` with its reason.
3. Expand to a **fact DAG**: each selected (check, lang) needs facts; each fact needs a producer; each producer may need facts. Shared facts are produced once per cache key. Cached facts (§8.3.6) are leaves.
4. Estimate cost per task from manifest cost class refined by historical durations in the store.
5. Order by value: `tier × novelty × changed_bonus`, where novelty prefers survivors from the last run and symbols never scored; `--budget` performs a greedy knapsack over the DAG respecting dependencies.
6. Execute: CPU-bound tasks on rayon; process-spawning tasks on tokio with a concurrency cap (`--jobs`, default cores). Mutants of one file run concurrently; files sequentially (mirrors mutator; avoids overlay conflicts).
7. On timeout/budget: cancel outstanding, mark `skipped`, emit `partial`.
8. Evaluate gate; compute deltas vs `--against`; write run to store; emit envelope.

Baseline test run happens once, in the real tree, before any mutant; red ⇒ exit 4.

### 8.10 Daemon (`serve`)

Holds parsed trees (content-hash keyed), store connection, coverage map, and the last plan. File watcher (`notify`) triggers incremental tree-sitter reparse and symbol re-hash. CLI connects over `.cupel/serve.sock` and proxies the request; output is byte-identical to a cold run. MCP mode exposes `check`, `survey`, `explain`, `tests_impacted`, `describe` as tools with the envelope as the result and events as progress notifications.

## 9. Performance design

### 9.1 Targets

| Operation | Cold | Warm (daemon) |
|---|---|---|
| CLI startup to first byte | < 50 ms | < 10 ms |
| `check --depth quick`, 500k LOC tree | 2–5 s | < 1 s |
| `check --changed --depth standard` (10 changed symbols, Rust) | ≤ 3 min | ≤ 1 min |
| `tests --impacted` | < 1 s | < 100 ms |

### 9.2 Static path

- Parallel walk via the `ignore` crate; parse with rayon; parse cache keyed by file content hash (unchanged file = one stat + hash).
- Clone detection: per-symbol fingerprint sets → MinHash signatures → LSH bands → pairwise Jaccard only within buckets. Approximately linear.
- Dependency graph built from import/use statements via tree-sitter queries; cycle detection via Tarjan SCC.

### 9.3 Mutation path

Stacked, in order of payoff:

1. **Coverage-gating** — uncovered sites are never executed.
2. **Store-based differential reruns** — unchanged `(symbol, site, suite, toolchain)` reuses outcome.
3. **Per-test selection** — run only tests whose coverage includes the mutated line; typically 5–20× over full-suite runs.
4. **Mutant schemata for compiled languages** — rewrite the function so every site is gated by a runtime switch (`cupel_mutant_id() == N`), compile once, run each mutant as a process with `CUPEL_MUTANT=N`. Replaces N compiles with one. Rust and Go first; the rewrite must preserve types (branch both arms through the same expression type) and is validated by compiling the schemata build before any mutant runs. Falls back to file-copy strategy on compile failure of the schemata build.
5. **Worker overlays** (file-copy strategy): reflink the tree where the filesystem supports it (APFS, btrfs, XFS), else hardlink the tree and copy the mutated file privately. Modules reaching the mutated file by relative import are copied too (Node resolution).
6. **Timeouts per test**, `timeout_factor × baseline_duration(test)`, floor 2 s.
7. **Ordering by information**: last run's survivors, then sites in changed symbols, then the rest.

### 9.4 Test impact analysis

Per-test coverage map inverted to `line → tests`; file-level import graph as fallback. Computed once per coverage run, stored, served from memory in the daemon.

### 9.5 Daemon

Avoids reparse, store open, and coverage-map load. Incremental reparse on change. The CLI must produce byte-identical output with or without the daemon; a fixture test enforces this.

### 9.6 Budget scheduler

Value per task = `tier_weight[tier] × novelty × (changed ? 2 : 1)`; cost from history (p50 of last 10 runs of that task shape, else manifest class default). Greedy by value/cost with dependency closure. Skipped tasks are reported with reason `budget` and an `est_ms` so the agent can decide to rerun with more time.

## 10. Compatibility and distribution

- **Platforms:** Linux x86_64/aarch64 (musl, static), macOS arm64/x86_64, Windows x86_64 native best-effort (WSL first-class). Built with `cargo-zigbuild`/`cross` in CI.
- **Distribution:** GitHub Releases; `cargo-binstall`; Homebrew tap; `curl -fsSL https://cupel.dev/install | sh`; thin `npm` (`npx cupel`) and `pipx` wrappers that fetch the platform binary. `required_version` in `cupel.toml`; `doctor` enforces.
- **Existing tooling:** ingest LCOV, Cobertura, JaCoCo XML, go coverprofile, Istanbul JSON, Stryker JSON, PIT XML, SARIF from other analyzers. Emit SARIF 2.1 and LCOV.
- **Monorepos:** multiple roots from workspace manifests; per-root scoping; single envelope with `scope.roots`.
- **Schema versioning:** `schema: v1`; additive within major; `describe schema` serves the exact version; `stability` on every entity.
- **Toolchain drift:** `doctor` pins versions; envelope echoes them; store invalidates on `toolchain_hash` change.
- **Agent surfaces:** stdio JSON (primary); MCP via `serve --mcp`; `describe --agent` for `CLAUDE.md`/`AGENTS.md`.

## 11. Testing Cupel

- **Fixture repos** per language under `cupel-fixtures/`: known-bad code (high CRAP, clone pairs, surviving mutants, weakened tests, dependency cycles, flaky test) with **golden envelopes**. Tests assert envelope equality modulo `run.id`, timestamps, durations.
- **Fact conformance**: per fact kind, an inventory of constructs every language producer must detect (§8.3.7); a language is admitted as a producer of a fact only when its inventory is complete.
- **Matrix test**: enumerates registered checks × languages; every `supported` pair has a passing golden, every `unsupported` pair has a passing skip assertion, every check runs with only its declared facts. No hand-maintained list of what works where.
- **Isolation**: `cupel-checks` compiles against `cupel-core` alone; `clippy::disallowed_methods` forbids `std::process` and `std::fs` in `cupel-checks`.
- **Protocol conformance suite** for plugins: a reference plugin in each of Rust/Python/TS exercised by the host; property tests on manifest/request/event round-trips.
- **Daemon parity test**: every fixture run cold and via daemon must be byte-identical.
- **Schemata validation**: for each fixture, schemata build output compiled and all mutants executed by both strategies must agree on outcomes.
- **Dogfood**: Cupel runs on itself in CI with `[gate.changed]` (`tests.mutation.score >= 0.80`, `tests.weakening = none`).
- **Performance regression**: `perf.bench` on the fixture tree against a stored baseline; fail CI on > 10% regression at p95.

## 12. Implementation plan

Phases are sequential; each ends with the fixture gate green and `cupel check --changed --gate` passing on Cupel itself.

| Phase | Deliverable |
|---|---|
| **P0 — Skeleton** | Workspace; `cupel-core` types + schemas; `cli` with `describe`, `init`, `doctor`; envelope/exit codes; text + JSON renderers; fixture harness. |
| **P1 — Language layer** | `Language` trait, `LanguageSpec`, generic query-driven producers; JavaScript, TypeScript, Rust, Go, Python each producing every syntactic fact (`Symbols`, `Decisions`, `NormTree`, `MutationSites`, `TestFrames`, `TestConventions`, `ModuleGraph`, `ApiSurface`, `Roots`, `ProjectInfo`); toolchain table; structural hashing; parse cache; parallel walk; per-fact conformance inventories; `describe matrix`. |
| **P2 — Static checks** | `static.complexity`, `static.crap` (CC-only mode), `static.clones` (MinHash/LSH), `static.deps`, `tests.quality`, `tests.weakening`, `static.dead-code`; `survey`; fact store v1; baselines; `suppress`; `explain` for these kinds. |
| **P3 — Coverage + mutation** | `CoverageMap` producers (lcov, Istanbul JSON, Cobertura, go coverprofile) and `TestResults` producers (cargo test/nextest, go test -json, pytest, vitest/jest); per-test maps where available; `tests.coverage`, full `static.crap`; `tests.mutation` with coverage-gating, store-based differential reruns, per-test selection, overlays; `tests --impacted`; exit 4 semantics; `tests.flaky`. |
| **P4 — Schemata + daemon** | Rust and Go schemata strategy with fallback; `serve` with socket, watcher, parity test; budget scheduler with history-based cost. |
| **P5 — Plugin protocol + oracles** | `cupel-plugin` host; reference plugins; `oracle` verb with `verify`; `gen.differential`, `gen.property` (wrapping Hypothesis/proptest/fast-check), `gen.metamorphic`; tier capping for unverified oracles. |
| **P6 — Wrapped tiers** | Producers (native or plugin) for `gen.fuzz`, `conc.race`, `perf.bench`, `static.analyze`, `static.api-surface`, `perf.footprint`; SARIF output; Java and Clojure via the §8.4.4 checklist (PIT wrapping for Java). |
| **P7 — Formal + distributed** | `spec.contracts`, `spec.trace` (TLC/Apalache), `conc.linearizability` (Elle/Porcupine), `conc.dst`, `conc.model-check` (Kani), `spec.prove`; `meta.traceability`; MCP server. |
| **P8 — Distribution** | Cross-platform builds, installers, wrappers, Homebrew, `cargo-binstall`; `describe --agent`; docs generated from `describe --all`. |

## 13. Decisions log

| Decision | Choice | Rationale |
|---|---|---|
| Language | Rust | Static binary, tree-sitter, parallelism |
| Parsing | tree-sitter, grammars compiled in | One CST for all languages; incremental reparse for daemon |
| Extension model | Language-neutral facts between producers and checks | Checks × languages × tools becomes a sum; support matrix is computed; execution lives only in producers |
| Fact vocabulary growth | Adding a fact kind requires an ADR | It is the one change that costs across every language; make it deliberate |
| Language capabilities | Derived from registered producers and fact completeness flags, not declared | A declared capability struct drifts; a registry cannot |
| Languages | Declarative (queries + tables) interpreted by generic producers; per-fact conformance inventories | Adding a language is data + fixtures, not core changes; the suite proves parity |
| Launch languages | JavaScript, TypeScript, Rust, Go, Python | Covers the ecosystems agents most often work in; Java/Clojure follow via the same checklist |
| JS and TS | Separate `LangId`s, shared implementation, one `ecma` family | Correct project detection and config; clones still comparable across the pair |
| Language plugins | Not offered (compiled-in only) | Per-file stdio round-trips are too slow for the daemon; dynamic grammars are a possible later addition |
| Symbol identity | Structural hash of normalized AST | Survives rename/move/reformat; cache key everywhere |
| Store | SQLite (WAL) in `.cupel/` | Content-addressed cache; queryable; single file |
| `survey` vs `check` | Separate verbs | Ranking without baseline ≠ gating against one |
| `oracle` | Verb with `verify` | Unverified oracles must not grant tier 4 |
| Mutation operators | Uncle Bob's small set as `default`; `extended` opt-in | Low equivalent-mutant rate by default |
| Compiled-language mutation | Schemata with file-copy fallback | Eliminates per-mutant compile |
| Auto-install | Off unless TTY | Agents must not mutate someone's toolchain silently |
| Output default | JSON when not a TTY | Agents are the primary consumer |
| Fix capability | None | Cupel grades; agents fix |
| Plugin tier 3 (WASM) | Deferred | Pure-analysis only; not needed for v1 techniques |

## 14. Open questions

1. Per-test coverage for JS/TS: investigate V8 per-test coverage via `NODE_V8_COVERAGE` per test process versus accepting file-level impact permanently.
2. Equivalent-mutant heuristics: whether to ship a low-confidence `mutant.equivalent?` kind in v1 or hold until a classifier exists; default is to emit with `confidence ≤ 0.3` and `severity: info`.
3. Schemata for Go: generic-type edge cases where a rewritten expression changes inference; needs the fixture set before committing to Go in P4 versus P6.
4. `spec.trace` instrumentation API: an attribute/macro per language versus a logging convention; decide when P7 starts.
5. Whether `meta.traceability` requires a spec file format or accepts requirement IDs embedded in test names (`#REQ-12`); lean toward both.
