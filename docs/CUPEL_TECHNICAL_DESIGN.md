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
- Modular: new techniques plug in as native checks or external-process plugins without touching the core.
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
| **Adapter** | Wrapper over an external tool that produces a shared intermediate (coverage map, test results, benchmark samples). Checks consume adapters; adapters are DAG prerequisites. |
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
| `static.crap` | `CC² × (1 − coverage)³ + CC`; requires `tests.coverage` adapter or `--no-coverage` (CC only) | `crap.high` (≥30), `crap.watch` (5–30) | build |
| `static.clones` | Type 1/2/3 clones via normalized-AST subtree fingerprints + Jaccard; MinHash bucketing before pairwise | `clone.pair`, `clone.cluster` | build |
| `static.deps` | Module graph: afferent/efferent coupling, instability, abstractness, distance from main sequence, cycles, layering-rule violations from config | `dep.cycle`, `dep.layer-violation`, `dep.unstable-abstract` | build |
| `static.api-surface` | Public API diff vs base: removed/changed signatures, visibility changes | `api.breaking`, `api.widened` | build + wrap (`cargo-semver-checks`, `api-extractor` where present) |
| `static.dead-code` | Unreachable/unused symbols and exports | `dead.symbol`, `dead.export` | build |
| `static.analyze` | Runs the ecosystem's type checker at strictest config, linters, CodeQL/Semgrep if configured; normalizes to findings | `analyze.<tool>.<rule>` | wrap |

### 5.2 `tests.*` — test-suite strength (tier 2–3)

| ID | Measures | Finding kinds | Cost | Build/wrap |
|---|---|---|---|---|
| `tests.coverage` | Line/branch coverage; MC/DC where tooling supports; **per-test coverage map** when available | `coverage.uncovered-symbol`, `coverage.uncovered-branch` | standard | wrap (adapters) |
| `tests.mutation` | Killed / (killed + survived) over covered sites; coverage-gated; differential; per-test selection; schemata for compiled langs | `mutant.survived`, `mutant.timeout`, `mutant.uncovered`, `mutant.equivalent?` (low confidence) | standard (changed symbols) / deep (touched files) / exhaustive (tree) | build (Py, Go, Rust, TS) + wrap (PIT for JVM, Stryker optional) |
| `tests.quality` | SCRAP-style: assertion density, weak assertions, mock density, logic in tests, duplicated setup | `test.weak-assertion`, `test.no-assertion`, `test.mock-heavy`, `test.logic`, `test.duplicate-setup` | quick | build |
| `tests.weakening` | **Diff of the test suite itself** vs base: removed assertions, skipped/disabled tests, loosened matchers, rewritten snapshots, widened `any`-style matchers | `assertion.removed`, `test.skipped`, `matcher.loosened`, `snapshot.rewritten`, `test.deleted` | quick | build |
| `tests.flaky` | Rerun impacted tests N times with shuffled order, varied seeds, parallelism | `test.flaky`, `test.order-dependent` | standard | build over runner adapters |

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
cupel doctor      verify toolchain, adapters, oracles, config; emit install_tool/configure actions
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
--no-install | --install           # auto-install adapter tooling (default: --no-install unless TTY)
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

**`doctor`** — Checks binary version vs `required_version`, every adapter tool's presence and version, oracle reachability (`oracle verify --all`), config validity, write access to `.cupel/`. Each problem is a finding with `action.kind: install_tool | configure`. Exit 3 if anything required for the configured checks is missing; 0 otherwise. `--fix --install` attempts installs.

**`describe`**

```
cupel describe                          # CLI: verbs, global flags, exit codes, schema URLs, config precedence
cupel describe <verb>                   # flags with types/defaults/enums, examples with expected output
cupel describe <check-id>               # what it measures, tier, cost, languages, prereqs, operators/params,
                                        #   finding kinds + how to act on each, known limitations, stability
cupel describe finding <kind>           # fields, evidence shape, action kinds it produces
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
    "scope": {"mode": "changed", "files": 4, "symbols": 11, "roots": ["."]},
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
cupel-core       scope resolution; task DAG + scheduler; budget; cache keys; Envelope/Finding types (schemars)
cupel-lang       tree-sitter frontends; LanguageAdapter trait; grammars compiled in
cupel-store      SQLite fact store (rusqlite, WAL); migrations; queries
cupel-plugin     plugin protocol types; external-process host; manifest validation
cupel-adapt-*    coverage parsers (lcov, cobertura, jacoco-xml, go-coverprofile, istanbul-json),
                 runner adapters (nextest, cargo test, go test -json, pytest, vitest/jest, mvn/gradle, clj),
                 bench adapters (criterion, benchstat, hyperfine)
cupel-check-*    one crate per native check
cupel-serve      daemon, Unix socket protocol, MCP server, file watcher
cupel-fixtures   per-language fixture repos + golden envelopes (dev-dependency)
```

Cargo workspace; the CLI binary links all native checks. `cupel-core` owns the types; JSON Schemas are generated from them with `schemars` at build time and embedded, so `describe schema` and the serializer cannot drift.

### 8.3 Language layer

```rust
pub trait LanguageAdapter: Send + Sync {
    fn id(&self) -> LangId;
    fn extensions(&self) -> &[&str];
    fn grammar(&self) -> tree_sitter::Language;

    /// Functions/methods: display name, namespace, visibility, span, body node.
    fn symbols(&self, tree: &Tree, src: &[u8]) -> Vec<Symbol>;
    /// Decision points for cyclomatic and cognitive complexity.
    fn decision_points(&self, body: Node, src: &[u8]) -> Vec<Decision>;
    /// Normalized tree: locals/fields/literals/keywords → generic markers; call names, operators, shape kept.
    fn normalize(&self, body: Node, src: &[u8]) -> NormTree;
    /// Mutation sites with operator id and replacement text(s).
    fn mutation_sites(&self, body: Node, src: &[u8]) -> Vec<Site>;
    /// Test cases and their assertions (for tests.quality / tests.weakening).
    fn test_frames(&self, tree: &Tree, src: &[u8]) -> Vec<TestCase>;
    /// Project detection: build file, default test/coverage/bench commands, test dirs to skip.
    fn project(&self, root: &Path) -> Option<ProjectInfo>;
    /// Schemata rewrite for compiled languages (None → file-copy strategy).
    fn schemata(&self, body: Node, src: &[u8], sites: &[Site]) -> Option<SchemataRewrite>;
}
```

`Symbol.hash` = `blake3(lang ∥ serialize(NormTree))`. This is the join key for metrics, coverage, mutants, baselines, and finding IDs.

Launch languages: Rust, Python, TypeScript/TSX (native, full trait). Go, Java, Clojure: full trait too but mutation may delegate to wrapped tools where better (PIT for Java). Adding a language = implementing the trait against the fixture repo.

Per-language decision-point and mutation-operator tables follow crapper/mutator's definitions (§3) as the default operator set `default`; an `extended` set adds statement deletion, return-value replacement, constant replacement, boundary (`+1/-1`), and negation. `extended` is opt-in because it increases equivalent-mutant rate.

### 8.4 Check trait (native)

```rust
pub trait Check: Send + Sync {
    fn manifest(&self) -> &Manifest;               // id, tier, cost, languages, prereqs, flags,
                                                   // finding kinds + action mapping, stability, docs
    fn plan(&self, ctx: &Ctx, scope: &Scope) -> Result<Plan>;     // targets + cost estimate (== --dry-run)
    fn run(&self, ctx: &Ctx, plan: &Plan, sink: &mut dyn EventSink) -> Result<CheckResult>;
}
```

`Ctx` provides: fact store handle, resolved config, adapter outputs by key (`coverage`, `per_test_map`, `test_results`), toolchain info, cancellation token, workspace scratch dir under `.cupel/work/<run>/`. `EventSink` receives findings and progress; the core serializes to `--events` and accumulates the envelope.

Checks never spawn test runners directly; they request a runner adapter via `ctx.runner(lang)` so the scheduler can account for cost and the fact store can record per-test outcomes.

### 8.5 Plugin protocol (external process)

Discovery: entries under `[plugins]` in `cupel.toml`, plus any `cupel-check-*` executable on `$PATH` (disabled by `--no-path-plugins`).

```
<plugin> describe            → stdout: Manifest JSON (same schema as native)
<plugin> plan   < RunRequest → stdout: Plan JSON
<plugin> run    < RunRequest → stdout: JSONL events (same as §7.4) ending with {"ev":"result", …CheckResult…}
```

`RunRequest` carries: protocol version, scope (files + symbols with hashes and spans), depth, budget, seed, resolved check config, paths to adapter outputs (LCOV file, per-test map JSON, test results JSON), scratch dir, toolchain info. Plugins must be side-effect-free outside the scratch dir unless `write: true` is in the request.

The host validates the manifest against the schema, enforces the budget (SIGTERM then SIGKILL), and rejects findings whose `kind` is not declared in the manifest. A plugin that exits non-zero without a `result` event yields `check.status: error`, never `pass`.

Tier 3 (WASM via wasmtime, same protocol over WASI stdio) is reserved for pure-analysis plugins; not v1.

### 8.6 Adapters

```rust
pub trait CoverageAdapter { fn produce(&self, ctx, scope) -> Result<CoverageMap>; fn parse(&self, path) -> Result<CoverageMap>; }
pub trait RunnerAdapter   { fn run(&self, ctx, selection: TestSelection, env: Env, timeout) -> Result<TestResults>; fn list(&self, ctx) -> Result<Vec<TestId>>; }
pub trait BenchAdapter    { fn run(&self, ctx, selection) -> Result<BenchSamples>; }
```

`CoverageMap` is line- and branch-level, optionally per-test (`test_id → lines`). Sources of per-test maps: nextest + llvm-cov profiles per test (Rust), `go test -coverprofile` per package with `-run` narrowing (Go), pytest-cov `--cov-context=test` (Python), JaCoCo per-test via agent sessions (Java). JS/TS lacks a cheap per-test map; fall back to file-level import-graph impact and mark `per_test_map: false` in the envelope.

Adapters ingest existing reports when fresher than the sources (mtime and content hash), avoiding regeneration.

### 8.7 Fact store

SQLite at `.cupel/cupel.db`, WAL, single writer (the daemon when present, else the CLI process holding a lock).

```
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

Cache validity for a mutant outcome: `(symbol_hash, site_hash, suite_hash, toolchain_hash)` unchanged ⇒ reuse. `suite_hash` = hash of the structural hashes of all test symbols that cover the site (or of the test files when no per-test map). Scores are recomputed from the store on every run, so a cached kill contributes without re-execution.

`.cupel/` is gitignored except `suppressions.toml` and any `baseline export` outputs the user commits.

### 8.8 Scheduler

1. Resolve scope → symbol set.
2. Select checks (depth, `--only/--skip`, `--tier`, language applicability).
3. Expand to a task DAG: adapter tasks (coverage, per-test map, baseline test run) are prerequisites; checks declare `needs`.
4. Estimate cost per task from manifest cost class refined by historical durations in the store.
5. Order by value: `tier × novelty × changed_bonus`, where novelty prefers survivors from the last run and symbols never scored; `--budget` performs a greedy knapsack over the DAG respecting dependencies.
6. Execute: CPU-bound tasks on rayon; process-spawning tasks on tokio with a concurrency cap (`--jobs`, default cores). Mutants of one file run concurrently; files sequentially (mirrors mutator; avoids overlay conflicts).
7. On timeout/budget: cancel outstanding, mark `skipped`, emit `partial`.
8. Evaluate gate; compute deltas vs `--against`; write run to store; emit envelope.

Baseline test run happens once, in the real tree, before any mutant; red ⇒ exit 4.

### 8.9 Daemon (`serve`)

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
| **P1 — Language layer** | `cupel-lang` trait; Rust, Python, TypeScript adapters: symbols, decision points, normalize, mutation sites, test frames, project detection; structural hashing; parse cache; parallel walk. |
| **P2 — Static checks** | `static.complexity`, `static.crap` (CC-only mode), `static.clones` (MinHash/LSH), `static.deps`, `tests.quality`, `tests.weakening`, `static.dead-code`; `survey`; fact store v1; baselines; `suppress`; `explain` for these kinds. |
| **P3 — Coverage + mutation** | Coverage adapters (lcov, istanbul, coverage.py) and runner adapters (nextest/cargo test, pytest, vitest); per-test maps where available; `tests.coverage`, full `static.crap`; `tests.mutation` with coverage-gating, store-based differential reruns, per-test selection, overlays; `tests --impacted`; exit 4 semantics; `tests.flaky`. |
| **P4 — Schemata + daemon** | Rust and Go schemata strategy with fallback; `serve` with socket, watcher, parity test; budget scheduler with history-based cost. |
| **P5 — Plugin protocol + oracles** | `cupel-plugin` host; reference plugins; `oracle` verb with `verify`; `gen.differential`, `gen.property` (wrapping Hypothesis/proptest/fast-check), `gen.metamorphic`; tier capping for unverified oracles. |
| **P6 — Wrapped tiers** | Adapters/plugins for `gen.fuzz`, `conc.race`, `perf.bench`, `static.analyze`, `static.api-surface`, `perf.footprint`; SARIF output; Go/Java/Clojure adapters (PIT wrapping for Java). |
| **P7 — Formal + distributed** | `spec.contracts`, `spec.trace` (TLC/Apalache), `conc.linearizability` (Elle/Porcupine), `conc.dst`, `conc.model-check` (Kani), `spec.prove`; `meta.traceability`; MCP server. |
| **P8 — Distribution** | Cross-platform builds, installers, wrappers, Homebrew, `cargo-binstall`; `describe --agent`; docs generated from `describe --all`. |

## 13. Decisions log

| Decision | Choice | Rationale |
|---|---|---|
| Language | Rust | Static binary, tree-sitter, parallelism |
| Parsing | tree-sitter, grammars compiled in | One CST for all languages; incremental reparse for daemon |
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
