<!-- ABOUTME: Every verb in detail, with captured examples: init, doctor, describe, check, survey, explain, and the rest. -->
<!-- ABOUTME: Part of Cupel's guide, linked from the README; `make readme` rewrites its generated sections and crates/cupel-cli/tests/readme.rs checks them. -->
# Usage

Every verb takes the same global flags (below) and writes the same envelope. Run any verb with `--describe` to see its flags instead of running it.

<!-- generated:verbs -->
| Verb | Does |
| --- | --- |
| `cupel init` | Detect project roots, languages, and test, coverage, and bench commands; write cupel.toml. |
| `cupel doctor` | Verify the binary's version, the configuration, .cupel/ write access, and needed tools. |
| `cupel describe` | Describe the CLI, a verb, a check, a finding kind, a language, a fact, the support matrix, or a schema. |
| `cupel check` | Run checks over a scope at a depth and evaluate the gate. |
| `cupel survey` | Rank a whole tree without a baseline or gate: the worst targets per metric, protect-first. |
| `cupel explain` | Give full context and remediation for one finding or check result. |
| `cupel baseline` | Set, show, diff, export, or import the baseline runs are compared with. |
| `cupel oracle` | Add, list, remove, or verify the oracles differential checks compare with. |
| `cupel tests` | List the tests a scope impacts, why, and the command that runs them. |
| `cupel suppress` | Suppress a finding, with a reason and an optional expiry. |
| `cupel serve` | Run the daemon that answers this repository's invocations with warm caches, or stop it. |
<!-- end:verbs -->

## `cupel init`

`init` detects project roots and languages and chooses test, coverage, and bench commands for each. It understands Cargo crates and workspaces, Go modules and workspaces, npm, pnpm, and yarn packages and workspaces (TypeScript when `tsconfig.json` is present), and Python projects (`pyproject.toml`, `setup.cfg`, `requirements.txt`).

A workspace whose root `package.json` `test` script delegates to its packages (`pnpm -r test`) gets one root per package under `[roots]`, each tested by its own runner and configuration; configured `[roots]` are the project's roots, so editing them narrows what Cupel tests, mutates, and analyzes ([ADR-177](../adr/CUPEL_DECISION_177.md), [ADR-167](../adr/CUPEL_DECISION_167.md), [ADR-173](../adr/CUPEL_DECISION_173.md)), and a language they name no root of runs nothing anywhere, not even at the repository root ([ADR-171](../adr/CUPEL_DECISION_171.md)). A root whose own `test` script runs something other than the runner Cupel would choose and has no packages to split into (a wrapper script) gets no test or coverage command from `init`, and a run uses no runner there unless `[lang.<id>] runner` pins one ([ADR-166](../adr/CUPEL_DECISION_166.md)): a runner's command at that root would not be the project's tests and could reach tiers the project runs apart, such as tests against a deployed environment. `init` reports it as `init.runner-deferred` (a note naming the runner and the rule), and `cupel describe language <id>` lists each runner's `defer` rules ([ADR-162](../adr/CUPEL_DECISION_162.md)). Add `test` and `coverage` under `[lang.<id>]` for the packages Cupel should run.

Every `cupel.toml` it writes starts with a `[gate.changed]` of ratchets, each rule under a comment saying what it holds, so `cupel check --changed --gate` gates a change at once ([ADR-233](../adr/CUPEL_DECISION_233.md)); a rule against `base` compares with the range's base, and is skipped without failing where there is nothing to compare with, such as a change whose files are all new ([ADR-205](../adr/CUPEL_DECISION_205.md)):

```toml
[gate.changed]
# No test is weakened: no assertion removed, matcher loosened, test skipped or deleted, or snapshot rewritten.
"tests.weakening" = "none"
# The mutation score is at least the base's less 0.02.
"tests.mutation.score" = ">= base - 0.02"
# The largest cyclomatic complexity stays within cyclomatic_max, or, past it at the base already, grows by 2 at most.
"static.complexity.cyclomatic_max" = "<= base"
# No more pairs of cloned functions than the base has.
"static.clones.pairs" = "<= base"
# No breaking change to the public API.
"static.api-surface" = "no-breaking"
```

```sh
cupel init                   # preview: the envelope carries unified diffs of what would be written
cupel init --write           # write cupel.toml and .cupel/.gitignore
cupel init --write --force   # replace an existing cupel.toml
```

## `cupel doctor`

`doctor` verifies the binary against `required_version` (see [Configuration](configuration.md); the other verbs refuse to run outside it), the validity of `cupel.toml`, and write access to `.cupel/`, and reports the disk the mutation engine's copies of the repository hold in `.cupel/work/slots/` (`doctor.mutation-slots`, a note). For every tool a configured check needs through its facts (a test runner, for a check that needs test results; a coverage tool, for a language with a `coverage` command), it checks the tool is on `PATH` and reports its version under `run.toolchain`; a `[lang.<id>] runner` that names no runner is a finding listing the valid ones. Every problem is a finding with a `configure` or `install_tool` action. Exit 3 means something required is missing.

A missing tool's finding names the command that installs it, or, for a language toolchain Cupel does not install (`cargo`, `go`, `python3`), what to do by hand. `--fix` runs those commands:

```sh
cupel doctor --fix --install             # install every missing tool, then check again
cupel doctor --fix                       # on a terminal, ask before each install
cupel doctor --fix --install --dry-run   # list the installs in the plan's would_install; run none
```

| Tool | Install command |
|---|---|
| `cargo-nextest` | `cargo install cargo-nextest --locked` |
| `cargo-llvm-cov` | `cargo install cargo-llvm-cov --locked && rustup component add llvm-tools-preview` |
| `pytest`, `pytest-cov`, `coverage` | `python -m pip install <package>`; `uv add --dev` or `poetry add --group dev` when the project uses uv or poetry |
| `vitest`, `@vitest/coverage-v8`, `jest`, `c8` | a dev dependency of the root: `npm install --save-dev`, `pnpm add --save-dev`, or `yarn add --dev` |

Installs run in the repository root with your environment, so the tool lands where your shell would put it: `CARGO_HOME`, the Python `PATH` finds (activate a virtual environment first), or the root's `node_modules`. With `--install` nothing is asked; without it Cupel asks on stderr when stdout is a terminal and installs nothing otherwise, and `--no-install` never installs. Vitest's coverage needs a provider package (`@vitest/coverage-v8` or `@vitest/coverage-istanbul`) that resolves from each root coverage runs in, or from where `vitest` is installed. Without one, `doctor` reports the provider as missing, even though `vitest` itself runs ([ADR-170](../adr/CUPEL_DECISION_170.md)). Likewise a `pytest` command with `--cov` options needs the pytest-cov plugin: `doctor` asks the command's interpreter (the `python` of `python -m pytest`, or the one the `pytest` on `PATH` names in its `#!` line) whether it finds `pytest_cov`, and reports `pytest-cov` as missing (a warning, as for a command's other programs) with its install command when it does not. A launcher such as `uv run` is not asked, since answering may change its environment. After installing, `doctor` resolves and probes again: `install.succeeded` names the version now installed, and `install.failed` (exit 3) carries the command and the tail of its stderr. `install.skipped` says why `--fix` installed nothing. `cupel check` and `cupel survey` follow the same flags for the tools their checks need, before they run; `--dry-run` lists those installs too.

## `cupel describe`

The binary documents itself. Every verb, flag, check, finding kind, language, fact, exit code, and schema is described by the same data that drives the CLI, so the documentation cannot drift from the behavior.

```sh
cupel describe                      # verbs, global flags, exit codes, schemas, config precedence
cupel describe check                # one verb: its flags with types, defaults, and examples
cupel describe static.clones        # one check: tier, cost, facts it needs, finding kinds, metrics, languages
cupel describe finding dep.cycle    # one finding kind: its check, its action, and its evidence schema
cupel describe matrix               # which checks run in which languages
cupel describe schema config        # a JSON Schema (draft 2020-12): run | finding | events | config | plugin
cupel describe --all                # the CLI, every verb, and every schema as one JSON document
```

Each schema's `$id` is the URL it is published at, under [`https://tysont.github.io/cupel/schema/v1/`](https://tysont.github.io/cupel/): `run.json`, `finding.json`, `events.json`, `config.json`, `plugin.json`, and `fact/<kind>.json` for each fact kind. The `pages` workflow publishes them from `main` with `scripts/schemas.sh` ([ADR-127](../adr/CUPEL_DECISION_127.md)).

## `cupel check`

`check` is the workhorse. It resolves a scope, selects checks, produces the facts those checks need, runs them, applies baselines and suppressions, and evaluates the gate.

**Scope** decides which files and symbols are graded. Scope flags intersect.

| Flag | Scope |
|---|---|
| (none) | The whole tree, minus `[scope] exclude` and always `target`, `node_modules`, `vendor`, and `.cupel`; with `[roots]`, only the files under some root |
| `--changed` | Files changed since the merge base with `default_branch`, including uncommitted work |
| `--base REF [--head REF]` | Files changed in an explicit range; `--head` defaults to the working tree |
| `--paths P` | Files, directories, or globs (gitignore-aware); repeatable |
| `--symbols S` | Symbols by qualified name (`shop.shipping.shipping_cost`) or structural hash (`sh_c7dca337c9ad7dd1`); repeatable |

With `[roots]` configured, every check, the static ones included, reads only files under some root: a file outside all roots is not reported, not counted in any metric or gate bound, and not in a `--changed` or `--base` scope, and a `--paths` value naming only such files is refused ([ADR-177](../adr/CUPEL_DECISION_177.md)). Import resolution still reads past the roots to find an import's target, without reporting on it. Without `[roots]`, the whole repository is in scope.

Under `--changed` or `--base` the symbols in scope are the functions and methods the range's changed lines touch (a file git does not track yet counts whole): a clone group is reported when one of its members was touched, every other per-symbol finding when its symbol was, so the changed gate rules on what the change wrote, and the tree gate grades the rest ([ADR-142](../adr/CUPEL_DECISION_142.md)). Facts are still produced for whole files, so a touched function's clone elsewhere in its file is found. A `--changed` scope with no changes passes with `scope.symbols: 0`. `tests.weakening` compares tests between two revisions, so it runs only with `--changed` or `--base` and is reported as skipped on a tree.

**Selection** decides which checks run.

| Flag | Selects |
|---|---|
| `--depth quick\|standard\|deep\|exhaustive` | How much evidence to gather; default `standard`, which adds the checks that run the test suite (`gen.property` and `conc.race` among them) to the `quick` ones; `deep` adds `gen.differential`, `gen.metamorphic`, `gen.fuzz`, `perf.bench`, and `conc.linearizability`; `exhaustive` adds `conc.dst`, `spec.trace`, `conc.model-check`, and `spec.prove` |
| `--only ID`, `--skip ID` | Checks by ID (`static.clones`) or group (`static`, `tests`); repeatable |
| `--tier N+` | Only checks at or above evidence tier N, e.g. `--tier 2+` |
| `--budget DURATION` | A budget of awake time (`250ms`, `20s`, `10m`): run the most valuable selected checks that fit and skip the rest; `CUPEL_BUDGET` or `budget` in `cupel.toml` also set it |
| `--jobs N` | How many threads and mutation workers run at once; default the cores (and, for mutation, the language's `mutation.jobs`) |
| `--oracle NAME` | The oracle checks that compare against one use; default the first verified (see [`cupel oracle`](#cupel-oracle)) |
| `--cases N` | The case budget of generative checks: how many inputs `gen.differential` and `gen.metamorphic` generate per entry point or relation (default 100), and how many generated cases each property test runs and how many inputs each fuzz target runs on (see [Property tests](generative.md#property-tests) and [Fuzzing](generative.md#fuzzing); default 1 000 at `standard`, ten times more at each deeper depth) |
| `--scaffold` | Also write a property-test stub for each `property.missing` finding; without `--write` the envelope's `plan.would_write` shows each file's diff and nothing is written |

**Budget.** `--budget` chooses among the checks the depth and the selection flags already chose; it never adds deeper ones. Each check costs its own run plus the facts it needs that nothing has made yet: the baseline test run, a coverage report, the mutation engine. A fact two checks share is paid once. A cost is the median of the last ten recorded runs of the same kind of work on this repository (the same check, language, and order of magnitude of targets; the same producer and language, run or read from cache), once three are recorded in `.cupel/cupel.db`; before that it comes from the cost class (a `quick` producer 100 ms, `standard` 30 s, `deep` 5 min). A check's value is its evidence tier plus one, doubled when it has never produced a result here, halved when its last result was made at this commit on a clean tree, and doubled again inside a `--changed` or `--base` scope. Checks are taken greedily by value per millisecond while they fit; ties are broken by `--seed`. The budget counts from the start of the invocation, as `--timeout` does, and both count only the time the machine is awake: a laptop that sleeps through a run does not spend its budget asleep, and `duration_ms` reports the same awake time ([ADR-217](../adr/CUPEL_DECISION_217.md)). A check is not started when its estimate exceeds what is left, and work still running at the deadline is stopped (its processes are killed), so a run overshoots its budget only by the moment that takes; a mutation run stopped that way reports `partial` with the score and findings of the mutants it finished, counts the rest as `not_run`, and stores what it finished, so the next run reuses those and runs the rest ([ADR-172](../adr/CUPEL_DECISION_172.md)). Anything the budget left out is `skipped` with `reason: budget` and `est_ms`, its estimate, the run is `partial` (exit 5), and `next_actions` offers the command that runs it alone. On a small Python project with a test suite and no recorded runs, one second leaves out everything that needs the suite:

```console
$ cupel check --budget 1s --format text
cupel check: partial (exit 5)
…
checks
  static.clones      pass     tier 0 · quick · score 0 pairs (n 1) · metrics clusters 0, max_jaccard 0, pairs 0 · 0 findings · 0 ms
  static.complexity  pass     tier 0 · quick · score 3 cyclomatic_max (n 4) · metrics cognitive_max 3, cyclomatic_max 3, length_max 5, nesting_max 1, params_max 2 · 0 findings · 0 ms
  static.crap        skipped  tier 0 · quick · 0 findings · 0 ms · skipped: budget (lang python, est 30.0 s)
  static.dead-code   pass     tier 0 · quick · score 0 dead_symbols (n 4) · metrics dead_exports 0, dead_symbols 0 · 0 findings · 256 ms
  static.deps        pass     tier 0 · quick · score 0 cycles (n 3) · metrics cycles 0, max_distance 0, violations 0 · 0 findings · 0 ms
  tests.coverage     skipped  tier 2 · standard · 0 findings · 0 ms · skipped: budget (lang python, est 30.4 s)
  tests.flaky        skipped  tier 2 · standard · 0 findings · 0 ms · skipped: budget (lang python, est 60.4 s)
  tests.mutation     skipped  tier 3 · standard · 0 findings · 0 ms · skipped: budget (lang python, est 90.4 s)
  tests.quality      pass     tier 2 · quick · score 1 assertion_density (n 2) · metrics assertion_density 1, no_assertion 0, weak_ratio 0 · 0 findings · 137 ms
  tests.weakening    skipped  tier 2 · quick · 0 findings · 0 ms · skipped: needs-range (lang python, see `cupel check --changed --only tests.weakening`)
…
next actions
  1. rerun: Run `static.crap`, which the budget cut short on python.
     $ cupel check --only static.crap
…
  4. rerun: Run `tests.mutation`, which the budget cut short on python.
     $ cupel check --only tests.mutation
```

`--dry-run` makes the same choice without running anything: the checks it would skip are under `would skip` with their estimates, and every planned check's `est_ms` counts the facts it would make, so `est_total_ms` is what the run is expected to take. With a minute, the same project plans the coverage report once for `static.crap` and `tests.coverage` and still leaves out mutation and flakiness, which would add the suite run and the engine:

```console
$ cupel check --dry-run --budget 1m --format text
cupel check: plan (exit 0)

plan      est 30440 ms
would run
  static.clones      4 targets · est 4 ms · needs symbols, norm-tree
  static.complexity  4 targets · est 4 ms · needs symbols, decisions, norm-tree
  static.crap        4 targets · est 4 ms · needs symbols, decisions, norm-tree, coverage-map, test-conventions, coverage-map(run)
  static.dead-code   4 targets · est 4 ms · needs symbols, norm-tree, module-graph, roots, references
  static.deps        4 targets · est 4 ms · needs module-graph, api-surface
  tests.coverage     4 targets · est 30400 ms · needs symbols, coverage-map, norm-tree, test-conventions, coverage-map(run)
  tests.quality      4 targets · est 20 ms · needs test-frames, symbols, norm-tree, decisions
would skip
  tests.weakening  needs-range (lang python, see `cupel check --changed --only tests.weakening`)
  tests.flaky      budget (lang python, est 60.4 s)
  tests.mutation   budget (lang python, est 60.4 s)
…
```

Without `--budget`, checks run in the order `cupel describe --all` lists them; with one, the most valuable per millisecond run first, and the envelope keeps the usual order.

**Grading** flags decide what the result is compared with.

| Flag | Effect |
|---|---|
| `--gate`, `--no-gate` | Evaluate `[gate.changed]` for range scopes or `[gate.tree]` otherwise. On by default when the table exists; `--gate` without the table is exit 2 |
| `--against baseline\|REF\|run:ID` | What `new_since_baseline` and each check's `delta` are computed against |
| `--dry-run` | Return the plan (checks, targets, facts, estimates, files it would write) and run nothing |
| `--format json\|jsonl\|text\|sarif` | JSON when stdout is not a terminal, text when it is |

Here is a small Python repository. An agent's branch added a shipping calculator, loosened one test's assertion, and skipped another test. The gate fails:

```console
$ cupel check --changed --depth standard --gate --format text
cupel check: fail (exit 1)

gate      fail (gate.changed)
  fail  tests.weakening none  actual 2
  fail  static.complexity none  actual 1
  pass  static.clones none  actual 0

summary   39 findings · fail 7 · warn 4 · info 28 · suppressed 0 · new and resolved need a baseline (`cupel baseline set`)

checks
  conc.race                 skipped  tier 4 · standard · 0 findings · 0 ms · skipped: unsupported (fact race-reports, lang python, see `cupel describe matrix`)
  gen.property              pass     tier 4 · standard · score 0 falsified (n 0) · metrics falsified 0, missing 1, properties 0 · 1 finding · 50 ms
  meta.oracle-independence  pass     tier 2 · quick · score 0 independently tested share (n 0) · metrics independent 0, self_authored_only 0 · 0 findings · 0 ms · note: tests that existed before the change exercise 0 of the 0 changed functions tests run; an oracle's independence is its verification (`cupel oracle verify`) (python)
  meta.traceability         pass     tier 2 · standard · 0 findings · 0 ms · note: no requirements are configured: list their IDs as `requirements` under [checks."meta.traceability"] (python)
  perf.footprint            pass     tier 0 · quick · score 0 dependencies (n 0) · metrics artifact_bytes 0, copyleft 0, dependencies 0, vulnerable 0 · 0 findings · 13 ms · note: artifact sizes not measured: set `[lang.<id>] artifacts` to their paths or globs (python) · note: no lockfile or manifest found (python) · note: vulnerabilities not measured: set `[lang.<id>] advisories` to a directory of OSV advisories (python)
  spec.contracts            pass     tier 5 · standard · 0 findings · 0 ms · note: tool.icontract: no project root declares the contract library it reads, so no contract was checked (python)
  static.analyze            fail     tier 1 · standard · score 5 diagnostics (n 2) · metrics diagnostics 5, errors 5 · 5 findings · 830 ms
  static.api-surface        pass     tier 0 · quick · score 0 breaking (n 3) · metrics breaking 0, widened 1 · 1 finding · 0 ms
  static.clones             pass     tier 0 · quick · score 0 pairs (n 1) · metrics clusters 0, max_jaccard 0, pairs 0 · 0 findings · 0 ms
  static.complexity         fail     tier 0 · quick · score 18 cyclomatic_max (n 2) · metrics cognitive_max 30, cyclomatic_max 18, length_max 26, nesting_max 3, params_max 5 · 1 finding · 4 ms
  static.crap               fail     tier 0 · quick · score 342 max (n 1) · metrics max 342, mean 342 · 1 finding · 0 ms
  static.dead-code          pass     tier 0 · quick · score 0 dead_symbols (n 2) · metrics dead_exports 1, dead_symbols 0 · 1 finding · 14 ms
  static.deps               pass     tier 0 · quick · score 0 cycles (n 2) · metrics cycles 0, max_distance 0, violations 0 · 0 findings · 0 ms
  tests.assertion-density   pass     tier 2 · quick · score 1 assertions_per_test (n 1) · metrics assertions_per_public_function 0, assertions_per_test 1, sparse_files 0, sparse_tests 0, tested_public_functions 0 · 0 findings · 0 ms
  tests.coverage            fail     tier 2 · standard · score 0 line_ratio (n 22) · metrics line 0, lines 22, uncovered 1 · 1 finding · 0 ms · note: branches: unavailable (python)
  tests.flaky               pass     tier 2 · standard · score 0 unstable (n 2) · metrics flaky 0, order_dependent 0 · 0 findings · 779 ms
  tests.mutation            pass     tier 3 · standard · score 0 killed_ratio (n 25) · metrics build_ms 0, equivalent 0, executed 0, fallback 0, killed 0, out_of_memory 0, plan_ms 0, reused 0, rewrite_ms 0, run_ms 0, schemata 0, score 0, survived 0, sync_ms 0, timeout 0, uncovered 25, unviable 0, validate_ms 0 · 25 findings · 64 ms
  tests.quality             fail     tier 2 · quick · score 1 assertion_density (n 1) · metrics assertion_density 1, no_assertion 0, weak_ratio 1 · 1 finding · 0 ms
  tests.weakening           fail     tier 2 · quick · score 2 weakened (n 2) · metrics weakened 2 · 2 findings · 0 ms

fail (7)
  static.analyze
    mypy no-untyped-def: Function is missing a type annotation
      fnd_478bce · analyze.mypy.no-untyped-def · tier 1 · confidence 1
      at src/shop/shipping.py:1:1-26:26
      evidence: level "error" · message "Function is missing a type annotation" · rule "no-untyped-def" · tool "mypy" (full: cupel explain fnd_478bce)
      action fix_bug (priority 1): Fix what mypy reports at src/shop/shipping.py:1 (no-untyped-def), or configure mypy if the rule does not apply to this project.
      repro: cupel check --only static.analyze --paths src/shop/shipping.py
      suppress: cupel suppress fnd_478bce --reason '…'
    mypy no-untyped-def: Function is missing a return type annotation
      fnd_677648 · analyze.mypy.no-untyped-def · tier 1 · confidence 1
      …
    …
  tests.weakening
    `test_discount_reduces_the_total` replaced 1 exact assertion(s) with weaker ones
      fnd_0c6fc5 · matcher.loosened · tier 2 · confidence 1
      at tests/test_pricing.py:6:1-7:47
      evidence: strong_base 1 · strong_head 0 · test "test_discount_reduces_the_total" · weak_base 0 · weak_head 1 (full: cupel explain fnd_0c6fc5)
      action restore_test (priority 1): Restore the exact expected values in `test_discount_reduces_the_total` instead of truthiness, presence, or inequality checks.
      repro: cupel check --only tests.weakening --base 8b01e28e053946898459f4337ff51f760c976fb7
      suppress: cupel suppress fnd_0c6fc5 --reason '…'
    `test_discount_rejects_a_negative_percent` is now skipped
      fnd_2b25cf · test.skipped · tier 2 · confidence 1
      …

warn (4)
  static.complexity
    `shop.shipping.shipping_cost` is too complex: cyclomatic 18 (max 10), cognitive 30 (max 15)
      fnd_002c9a · complexity.high · tier 0 · confidence 1
      at src/shop/shipping.py:1:1-26:26 in shop.shipping.shipping_cost (python, sh_7cfabd3ccaf75503)
      evidence: cognitive 30 · cognitive_max 15 · cyclomatic 18 · cyclomatic_max 10 (full: cupel explain fnd_002c9a)
      action reduce_complexity (priority 2): Split `shipping_cost` into smaller functions or replace conditional chains with data, until cyclomatic is at most 10 and cognitive at most 15.
        deepest_block: branch > ternary > ternary at src/shop/shipping.py:5:39-5:66, nesting 2
      repro: cupel check --only static.complexity --symbols sh_7cfabd3ccaf75503
      suppress: cupel suppress fnd_002c9a --reason '…'
  static.crap
    `shop.shipping.shipping_cost` has CRAP 342.0 (high at 30): cyclomatic 18 with 0% coverage
      …
  tests.coverage
    `shop.shipping.shipping_cost` never ran under test: none of its 22 instrumented lines
      …
  tests.quality
    `tests.test_pricing.test_discount_reduces_the_total` asserts only weakly: 1 weak assertion(s), none pinning a value
      …

info (28)
  gen.property
    `shop.shipping.shipping_cost` has no property test
      …
  static.api-surface
    public `shop.shipping.shipping_cost` was added
      …
  static.dead-code
    `shop.shipping.shipping_cost` is exported but no import resolves to src/shop/shipping.py
      …
  tests.mutation
    No test runs src/shop/shipping.py:2; its mutant `<=` → `<` was not run
      …
    …

run       run_01M66ESWMTBB8N6E3H2KDWCNZ8 · started 2026-10-30T13:08:21Z · 2652 ms · seed 8293632077924340691
git       fc5a540a3e2ca9a11ed2ff63e8cbf1774f1bbeca · branch agent/shipping · base 8b01e28e053946898459f4337ff51f760c976fb7 · clean
scope     changed · 2 files · 2 symbols · roots . · languages python
depth     standard
suite     python full · 2 rerun · 0 reused
…

next actions
  1. restore_test: Restore the exact expected values in `test_discount_reduces_the_total` instead of truthiness, presence, or inequality checks. (finding fnd_0c6fc5)
  2. restore_test: Remove the skip marker from `test_discount_rejects_a_negative_percent` and fix what made it fail. (finding fnd_2b25cf)
  3. fix_bug: Fix what mypy reports at src/shop/shipping.py:1 (no-untyped-def), or configure mypy if the rule does not apply to this project. (finding fnd_478bce)
  4. fix_bug: Fix what mypy reports at tests/test_pricing.py:6 (no-untyped-def), or configure mypy if the rule does not apply to this project. (finding fnd_677648)
  …
```

The same check as JSON, trimmed to two checks, one finding, and one action:

```json
{
  "cupel": { "version": "0.2.9", "schema": "https://tysont.github.io/cupel/schema/v1/run.json" },
  "run": {
    "id": "run_01M66ESZ81EZ7HN1Z8NM3YHJY0", "verb": "check",
    "started": "2026-10-30T13:08:23Z", "duration_ms": 1303,
    "git": { "sha": "fc5a540a3e2ca9a11ed2ff63e8cbf1774f1bbeca", "dirty": false,
             "base": "8b01e28e053946898459f4337ff51f760c976fb7", "branch": "agent/shipping" },
    "scope": { "mode": "changed", "files": 2, "symbols": 2, "roots": ["."], "languages": ["python"] },
    "depth": "standard", "seed": 5896116977636554219, "…": "…"
  },
  "status": "fail",
  "exit_code": 1,
  "checks": [
    {
      "id": "static.complexity", "status": "fail", "tier": 0, "cost": "quick",
      "score": { "value": 18.0, "unit": "cyclomatic_max", "n": 2 },
      "metrics": { "cognitive_max": 30.0, "cyclomatic_max": 18.0, "length_max": 26.0, "nesting_max": 3.0, "params_max": 5.0 },
      "delta": null, "findings": 1, "duration_ms": 4, "skipped": null, "oracle": null
    },
    {
      "id": "tests.weakening", "status": "fail", "tier": 2, "cost": "quick",
      "score": { "value": 2.0, "unit": "weakened", "n": 2 },
      "metrics": { "weakened": 2.0 },
      "delta": null, "findings": 2, "duration_ms": 0, "skipped": null, "oracle": null
    }
  ],
  "findings": [
    {
      "id": "fnd_002c9a", "check": "static.complexity", "kind": "complexity.high",
      "severity": "warn", "confidence": 1.0, "tier": 0,
      "symbol": { "display": "shop.shipping.shipping_cost", "hash": "sh_7cfabd3ccaf75503", "lang": "python" },
      "location": { "file": "src/shop/shipping.py", "line": 1, "col": 1, "end_line": 26, "end_col": 26 },
      "title": "`shop.shipping.shipping_cost` is too complex: cyclomatic 18 (max 10), cognitive 30 (max 15)",
      "detail": null,
      "evidence": { "cognitive": 30, "cognitive_max": 15, "cyclomatic": 18, "cyclomatic_max": 10 },
      "action": {
        "kind": "reduce_complexity", "priority": 2, "hints": {},
        "summary": "Split `shipping_cost` into smaller functions or replace conditional chains with data, until cyclomatic is at most 10 and cognitive at most 15."
      },
      "repro": "cupel check --only static.complexity --symbols sh_7cfabd3ccaf75503",
      "suppress": "cupel suppress fnd_002c9a --reason '…'",
      "new_since_baseline": false,
      "suppressed": false
    }
  ],
  "summary": {
    "findings": { "fail": 7, "warn": 4, "info": 28 },
    "new": 0, "resolved": 0, "suppressed": 0,
    "gate": {
      "result": "fail", "evaluated": "gate.changed",
      "rules": [
        { "rule": "tests.weakening none", "actual": 2.0, "result": "fail" },
        { "rule": "static.complexity none", "actual": 1.0, "result": "fail" },
        { "rule": "static.clones none", "actual": 0.0, "result": "pass" }
      ]
    }
  },
  "next_actions": [
    { "priority": 1, "finding": "fnd_0c6fc5", "kind": "restore_test", "command": null,
      "summary": "Restore the exact expected values in `test_discount_reduces_the_total` instead of truthiness, presence, or inequality checks." }
  ],
  "truncated": false,
  "total_findings": 37
}
```

Every object has a fixed key set; an absent value is `null`, never a missing key (a check result's `notes`, omitted when empty, is the one exception). `status` is one of `pass`, `fail`, `partial`, `error`, or `plan`. A check result in `error` says why in its `notes`, such as a tool that could not run. `--format jsonl` writes the envelope header and then one finding per line. `cupel describe schema run` and `cupel describe schema finding` are the full schemas.

`--format sarif` writes the envelope as a [SARIF 2.1.0](https://docs.oasis-open.org/sarif/sarif/v2.1.0/) log for code-scanning tools and editors, from every verb. Each finding is a result: its kind is the `ruleId`, its severity the `level` (`fail` is `error`, `warn` is `warning`, `info` is `note`), its location a region relative to the `SRCROOT` base (the repository root), and its ID `partialFingerprints["cupel/findingId"]`; a suppressed finding carries an `accepted` suppression, and `fixes` is always empty. The rules are the finding kinds of the checks that ran, described as `cupel describe finding` describes them. The commit is in `versionControlProvenance` and the exit code in the invocation. What SARIF has no place for is kept in `properties`: a result's holds the finding's check, tier, confidence, evidence, action, and repro; the invocation's holds the envelope's `run`; the run's holds the rest of the envelope (checks, summary and gate, `next_actions`, and each verb's own payload).

## `cupel survey`

`survey` is the first command to run on an unfamiliar repository. It takes the same scope and selection flags as `check`, never gates, and adds `rankings`: the ten worst targets for every per-symbol, per-module, and per-group metric (`worst` says which end ranks first: `lowest` for the protection metrics `tests.mutation.mutation_score`, `tests.mutation.tests`, and `tests.coverage.line_coverage`, `highest` for every other), and `targets`, protect-first.

`targets` are the symbols a finding asks to protect (`write_test`, `strengthen_assertion`) or to simplify (`reduce_complexity`, `refactor_extract`), each with its `recommend` (`protect`, `simplify`, or `both`), its `protection` (`mutation_score`, `coverage`, and `tests`; null where nothing measured them), its value and rank in every dimension, and the findings that recommend it. Refactoring code no test protects is unsafe, so targets to protect come first, the weakest protection (the mutation score, else coverage) first among them; equal protection falls back to the target's best rank in any other dimension. `recommended` counts them all; `targets` lists the first ten. `survey`'s `next_actions` follow the same order: an action to protect each target, then one to simplify each, then the rest. Protection is measured from `--depth standard`, where `tests.mutation` and `tests.coverage` run; at `--depth quick` it is null and only the findings say what to do. Suppressed findings recommend nothing.

```console
$ cupel survey --depth quick --format text
cupel survey: pass (exit 0)

summary   5 findings · fail 0 · warn 3 · info 2 · suppressed 0 · new and resolved need a baseline (`cupel baseline set`)

checks
  …
  tests.weakening    skipped  tier 2 · quick · 0 findings · 0 ms · skipped: needs-range (lang python, see `cupel check --changed --only tests.weakening`)

rankings  top 10 per dimension
  static.clones.cluster_size  group · 1 measured · Symbols in each clone group: 2 for a pair, more for a cluster.
     1. 2  shop.pricing.apply_discount and 1 other  python · src/shop/pricing.py:5:1-8:41
  static.complexity.cyclomatic  symbol · 6 measured · Modified cyclomatic complexity per function (a match counts once).
     1. 15  shop.shipping.shipping_cost  python · src/shop/shipping.py:1:1-27:16 · sh_c7dca337c9ad7dd1
     2. 3  shop.pricing.apply_discount  python · src/shop/pricing.py:5:1-8:41 · sh_1daf7938d546f072
     3. 3  shop.shipping.apply_shipping_discount  python · src/shop/shipping.py:30:1-33:41 · sh_1daf7938d546f072
     …
  static.deps.coupling  module · 2 measured · Afferent plus efferent coupling per module.
     1. 1  src/shop/pricing.py  python · src/shop/pricing.py:1
     2. 1  tests/test_pricing.py  python · tests/test_pricing.py:1
  …
  targets  2 recommended, protect first
     1. simplify  shop.shipping.shipping_cost  mutation - · coverage - · tests -  python · src/shop/shipping.py:1:1-27:16 · sh_c7dca337c9ad7dd1
        static.complexity.cyclomatic 15 (#1) · static.complexity.cognitive 19 (#1) · …
     2. simplify  shop.pricing.apply_discount  mutation - · coverage - · tests -  python · src/shop/pricing.py:5:1-8:41 · sh_1daf7938d546f072
        static.complexity.cyclomatic 3 (#2) · … · fnd_c5c4da

warn (3)
  static.clones
    `shop.pricing.apply_discount` and `shop.shipping.apply_shipping_discount` (src/shop/shipping.py:30) are type-2 clones (jaccard 1.00)
      fnd_c5c4da · clone.pair · tier 0 · confidence 1
      …
```

On the tree, `survey` finds the clone the changed-scope `check` above did not: `check --changed` builds facts only for the changed files, so a new function that copies an unchanged one is visible to `survey` and to tree-scope `check`, not to `--changed`.

## `cupel explain`

`explain` reads the fact store and never re-runs anything. Given a finding ID it returns the stored finding with a source excerpt, its full evidence, and its remediation hint in full: for `clone.cluster` the largest subtree every member shares (its place in each member and the names and literals that differ, the extracted function's parameters), for `complexity.high` the most deeply nested block (its place, the constructs on the path to it, and the complexity it carries), and for `dep.cycle` the cheapest import to cut (the names it carries, and whether removing it breaks every cycle among the modules). The checks compute these hints when they report the finding, so `check` and `survey` carry them in `action.hints` too. For what a range removed (a deleted test, a removed public item), the excerpt is the file as the range's base had it, named with the commit, since the working tree's lines at that place are other code. Given a check ID it returns that check's result, findings, and metrics from the last run that included it.

```console
$ cupel explain fnd_c5c4da --format text
cupel explain: pass (exit 0)

explain   finding fnd_c5c4da from run_01M4X8VRAYVSC12KFEYGD2BX8A
  src/shop/pricing.py
    2 |     return sum(item.price * item.quantity for item in items)
    3 |
    4 |
  > 5 | def apply_discount(total, percent):
  > 6 |     if percent < 0 or percent > 100:
  > 7 |         raise ValueError("percent must be between 0 and 100")
  > 8 |     return total * (100 - percent) / 100
  evidence
    {
      "clone_type": 2,
      "jaccard": 1.0,
      "left": { "symbol": "shop.pricing.apply_discount", "file": "src/shop/pricing.py", "line": 5, "end_line": 8, "lines": 4, "nodes": 34, … },
      "right": { "symbol": "shop.shipping.apply_shipping_discount", "file": "src/shop/shipping.py", "line": 30, "end_line": 33, "lines": 4, "nodes": 34, … },
      "threshold": 0.82
    }
…
```

## `cupel baseline`

A baseline is a stored run that later runs are compared with. Findings absent from the baseline are marked `new_since_baseline`, and each check reports a `delta` against it. Writing a baseline changes only `.cupel/`, but `set` and `import` still preview until you pass `--write`. A baseline advances only on a passing run on the default branch ([ADR-182](../adr/CUPEL_DECISION_182.md), [ADR-195](../adr/CUPEL_DECISION_195.md)): `set` and `import` refuse a run that failed its gate, was cut short, or errored, and a run recorded on another branch or a detached `HEAD`. A run that is `partial` only because its `--budget` left checks out or cut them short at the deadline is accepted; the baseline then holds no result for those checks, so a rule against `base` on one of them is skipped with `baseline.missing` until a later baseline has it ([ADR-216](../adr/CUPEL_DECISION_216.md)). The default branch is `default_branch` in `cupel.toml`, else the branch `origin/HEAD` names, else `main` or `master`. The run before the first baseline passes when its other rules do, since a rule against `base` with no reference is skipped rather than failed ([ADR-198](../adr/CUPEL_DECISION_198.md)); a run recorded outside a git work tree has no branch and is accepted.

```sh
cupel baseline set --write            # the latest check or survey run becomes the baseline
cupel baseline set --from run:ID --write
cupel baseline show                   # the baseline run as an envelope
cupel baseline diff                   # the latest run against the baseline: new and resolved findings
cupel baseline diff --against main    # ... or against the latest run at a git ref, or run:ID
cupel baseline export baseline.json   # a committable copy
cupel baseline import baseline.json --write
```

`--name NAME` keeps several named baselines. Here the baseline was set on `main`, then the agent's branch was checked:

```console
$ cupel baseline diff --format text
cupel baseline: pass (exit 0)

summary   5 findings · fail 0 · warn 3 · info 2 · new 5 · resolved 0 · suppressed 0

checks
  static.clones      fail     tier 0 · quick · score 1 pairs (n 3) · metrics clusters 0, max_jaccard 1, pairs 1 · delta +1 vs baseline · 1 finding · 0 ms
  static.complexity  fail     tier 0 · quick · score 15 cyclomatic_max (n 6) · metrics cognitive_max 19, cyclomatic_max 15, length_max 27, nesting_max 2, params_max 5 · delta +12 vs baseline · 1 finding · 4 ms
  …

warn (3)
  static.clones
    `shop.pricing.apply_discount` and `shop.shipping.apply_shipping_discount` (src/shop/shipping.py:30) are type-2 clones (jaccard 1.00)
      fnd_c5c4da · clone.pair · tier 0 · confidence 1 · new
      …
```

A finding's ID is derived from its function's structure, so renaming or moving a function keeps it, but editing the function changes it. Under `--changed` or `--base`, the functions the range removed are matched to the ones it added by how much of their structure they share: a rename with an edit, a move, a function split in two, or two merged into one. A finding on the old function with a finding of the same check and kind on any of its successors is neither new nor resolved. Each check's `delta` lists these matches as `sets`: the functions on each side, how similar they are, the findings carried across, and each per-function metric over each side (the sum of a count such as `cyclomatic`, the size-weighted mean of a score such as `crap`). `tests.weakening` matches a renamed or split test the same way instead of reporting it deleted ([ADR-131](../adr/CUPEL_DECISION_131.md)).

A baseline finding is resolved only when the run compared with it could have reported it again: its check ran to completion (it passed or failed, rather than being skipped, erroring, or cut short), and, when the run did not cover the whole tree, the finding is in a file the run considered and not on a function the run left out. Under `--changed`, a function the change did not touch is left out, so `check --changed` against a baseline set over the whole tree resolves only the findings the change removed, and `baseline diff` after it lists only those. A finding with no file is resolved only by a whole-tree run. A check's `delta` likewise compares scores only for checks that ran to completion in both runs. Each non-tree run records the files it considered (`run.scope.paths`) and the functions it left out (`run.scope.symbols_left_out`), so a stored run compares the same way later ([ADR-206](../adr/CUPEL_DECISION_206.md)).

## `cupel oracle`

An oracle is something a check compares against: an earlier version at a git ref, a command that computes the reference answer, or a specification. Oracles are defined in `cupel.toml` as `[oracles.<name>]` tables, so they are reviewed and committed like the rest of the configuration; whether each one is verified is recorded in the fact store. `add` and `remove` change only that table, keeping the rest of the file as it was, and preview the diff until you pass `--write`.

```sh
cupel oracle add release --kind ref --ref v1.2.0 --write
cupel oracle add sum --kind command --cmd "python3 sum.py {input}" --input "1 2 3" --input "40 2" --write
cupel oracle add model --kind spec --spec specs/cart.qnt --lang quint --write
cupel oracle list                     # every oracle with its state
cupel oracle verify sum               # run its smoke comparison; --all verifies every oracle
cupel oracle remove sum --write       # drop the table and the verification
```

`verify` runs a smoke comparison for each kind. A ref must resolve to a commit. A command runs twice on each known input (`--input`, or one empty input; `{input}` is replaced by the shell-quoted input) and must exit 0 and print the same both times. A spec must be accepted by its language's parser: `quint parse`, `sany` for TLA+, or `p compile`. A pass records the oracle's identity hash (its definition, plus the commit its ref resolves to or the spec file's content) with the run that verified it. The oracle stays `verified` while that identity holds. Editing its table, moving its ref, or changing its spec file makes it `changed`. A ref or spec file that is gone makes it `unreachable`. A failed `verify` revokes the verification. An oracle that cannot be reached exits 3 (`oracle.unreachable`); one that answers differently each time exits 1 (`oracle.smoke-failed`):

```console
$ cupel oracle verify --all --format text
cupel oracle: fail (exit 1)

summary   1 finding · fail 1 · warn 0 · info 0 · suppressed 0

oracles  3
  clock    command `python3 -c 'import time; print(time.time())'` · unverified
  release  ref v1.2.0 · verified · recorded 2026-10-16T21:42:51Z by run_01M53ANXSYM8E0R93GKZRRF5CT
  sum      command `python3 sum.py {input}` · inputs 2 · verified · recorded 2026-10-16T21:42:51Z by run_01M53ANXSYM8E0R93GKZRRF5CT

fail (1)
  oracle
    Oracle `clock` is not verified: the command printed different output twice for the empty input
      fnd_daeb25 · oracle.smoke-failed · tier 0 · confidence 1
      …

$ cupel oracle list --format text     # after adding an input to `sum`
…
oracles  3
  clock    command `python3 -c 'import time; print(time.time())'` · unverified
  release  ref v1.2.0 · verified · recorded 2026-10-16T21:42:51Z by run_01M53ANXSYM8E0R93GKZRRF5CT
  sum      command `python3 sum.py {input}` · inputs 3 · changed · recorded 2026-10-16T21:42:51Z by run_01M53ANXSYM8E0R93GKZRRF5CT
```

The envelope of every `oracle` command carries the oracles it reported in `oracles`: each one's `name`, `definition`, `state`, identity `hash`, `verified_at`, and `verified_run`.

A check that compares against an oracle names it in its result's `oracle` member. It uses `--oracle NAME` on `check` or `survey`, else the first verified oracle, else the first defined one; with no oracle defined, the check is skipped as `unconfigured`. When the run is graded, a result against an oracle that is not verified at that moment is capped at tier 3 and the run carries a `warn` finding, `oracle.unverified` (`cupel describe finding oracle.unverified`), whose action is `verify_oracle`. Only `cupel oracle verify` lifts the cap; suppressing the finding does not. `gen.differential` is the built-in check that compares against an oracle (see [Differential testing](generative.md#differential-testing)); it compares against `ref` and `command` oracles and skips itself, as `unconfigured`, when the run's oracle is a spec. `spec.trace` compares against `spec` oracles (see [Trace validation](specifications.md#trace-validation)): name one with `--oracle` when the repository defines oracles of both kinds.

## `cupel suppress`

`suppress` silences one finding with a reason and an optional expiry date or git ref. It writes `.cupel/suppressions.toml`, the one file under `.cupel/` meant to be committed. A suppressed finding still appears, with `suppressed: true` and severity `info`, and no longer counts against the gate. When the expiry passes, the finding is active again and a `suppression.expired` finding says so.

```console
$ cupel suppress fnd_af9323 --reason "public API; the checkout service imports it" --until 2027-01-31 --format text
cupel suppress: pass (exit 0)

summary   1 finding · fail 0 · warn 0 · info 1 · suppressed 1

info (1)
  static.dead-code
    `shop.shipping.shipping_cost` is exported but no import resolves to src/shop/shipping.py
      fnd_af9323 · dead.export · tier 0 · confidence 0.4 · new · suppressed
      …

$ cat .cupel/suppressions.toml
# Cupel suppressions: written by `cupel suppress <finding-id> --reason TEXT [--until DATE|REF]`.
[[suppression]]
finding = "fnd_af9323"
reason = "public API; the checkout service imports it"
until = "2027-01-31"
created = "2026-10-14"
```

## `cupel tests --impacted`

`tests --impacted` lists the tests a scope impacts, why, and a ready-to-run command per runner. It takes the same scope flags as `check`; with `--changed` or `--base` it reads the changed lines and widens each change to the functions it touches. Where a language has a per-test coverage map (Rust through `cargo llvm-cov` and `cargo nextest`, Go through `go test -coverprofile` per test, Python through pytest-cov's test contexts), a test is impacted when it executes one of those lines. JavaScript, TypeScript, Java, and Clojure have no per-test map, so their impacted tests are the test files that import a changed file, directly or through other files; the envelope says so with `per_test_map: false` and the reason. The same happens for any language whose coverage tool is not installed.

```console
$ cupel tests --impacted --changed --format text    # `add` changed in calc.py
cupel tests: pass (exit 0)

tests  1 impacted · per-test map: yes
  python  per-test map from tool.pytest-cov-per-test (2 measured)
  tests/test_calc.py::test_add  python
    covers calc.add (calc.py:6)
  run in .: python -m pytest tests/test_calc.py::test_add
  …

$ cupel tests --impacted --changed --format text    # `add` changed in add.js
cupel tests: pass (exit 0)

tests  1 impacted · per-test map: no
  javascript  file-level impact: no producer makes per-test-map for this language (see `cupel describe matrix`)
  add.test.js  javascript
    imports add.js
  run in .: npx jest add.test.js
  …
```

In JSON the same report is the envelope's `tests`: `per_test_map`, `impacted` (`id`, `file`, `reason`, `lang`), `commands` (`lang`, `runner`, `dir`, `command`, and `narrowed: false` when the runner cannot run only those tests and the command runs its whole suite), and `languages` (per language, whether a per-test map chose the tests or why not, and how many of the map's tests were `measured` for this run and `reused` from the cache). `cupel describe matrix` shows which languages can have a per-test map here.

Measuring a per-test map costs a test run under coverage: one instrumented `cargo llvm-cov nextest` run for Rust (each test's coverage is told apart by process, and the instrumented build has its own `target/cupel-llvm-cov`, so a configured `cargo llvm-cov` never discards it), one pytest-cov run for Python, and one `go test` per test for Go. Each project root's map is cached under `.cupel/cache/per-test/<producer>/`, with the hash of every file under the root and of every line the tests ran, so a later run reuses it. After a change, only the stale tests are measured again: those that ran a line of a function the change touched, those whose own test file changed (for Go, any test file of their package), and tests the cache does not hold; tests that are gone are dropped, and the kept tests' lines move with the edit. A change outside every function (a new function, an import, a constant, a comment above a function) makes stale every test that covers the file. A change to build or test configuration (`Cargo.toml`, `Cargo.lock`, `go.mod`, `conftest.py`, `pyproject.toml`, and the like) measures every test again, and so does deleting the cache. The text report says how many tests were measured, or `(cached)` when none was.

## `cupel serve`

`serve` runs a daemon for one repository in the foreground. It keeps every file's syntax tree, keyed by a hash of the file's content, and what each fact producer extracted from it, across invocations, so a warm `check --depth quick` neither parses nor re-extracts an unchanged file. While `.cupel/serve.sock` exists and answers, every `cupel` invocation in the repository is answered by the daemon, and prints exactly what it would print without it: the same envelope, the same stderr, the same exit code. `--events stderr` lines stream to the invoking process as the run writes them.

```console
$ cupel serve --watch
cupel: serving /home/me/shop at .cupel/serve.sock (pid 72908, watching)
```

In another terminal, `doctor` reports the daemon, and `serve --stop` stops it:

```console
$ cupel doctor --format text
cupel doctor: pass (exit 0)

summary   1 finding · fail 0 · warn 0 · info 1 · suppressed 0

info (1)
  doctor
    The daemon at .cupel/serve.sock serves this repository (pid 72908)
      fnd_c243c8 · doctor.daemon-running · tier 0 · confidence 1
      evidence: pid 72908 · socket ".cupel/serve.sock" · version "0.2.9" · watching true (full: cupel explain fnd_c243c8)
      …

$ cupel serve --stop --format text
cupel serve: pass (exit 0)

summary   1 finding · fail 0 · warn 0 · info 1 · suppressed 0

info (1)
  serve
    Stopped the daemon at .cupel/serve.sock
      fnd_8ec73e · serve.stopped · tier 0 · confidence 1
      …
```

```console
$ cupel check --format json > envelope.json      # cupel was rebuilt after the daemon started
cupel: ran without the daemon: the daemon at /home/me/shop/.cupel/serve.sock runs another build of cupel (cupel 0.2.9 (binary 56427528-1792134645316655513)) than this client (cupel 0.2.1 (binary 56427528-1792134881357328000)). Restart it from this one: `cupel serve --stop`, then `cupel serve`.
```

- **When the CLI uses it.** Every invocation whose repository root (`--cwd`, else the working directory) has a `.cupel/serve.sock`. It runs in its own process instead when `CUPEL_NO_DAEMON=1` is set, for `serve` itself, when an argument is not UTF-8, when the run could ask a question on the terminal (stdout is a terminal and neither `--install` nor `--no-install` is given), when `--jobs` is given (a process sizes its thread pool once), and when nothing answers on the socket (a daemon that was killed leaves its socket file behind; it is ignored).
- **When it declines.** A daemon runs only invocations from the same build of `cupel` (the same version and the same executable) and with the same `PATH`, since `PATH` decides which tools run. Otherwise the invocation runs in its own process and says why in one line on stderr (unless `--quiet`); rebuilding or reinstalling `cupel` therefore means restarting the daemon. Other variables (`HOME`, `CARGO_HOME`, `VIRTUAL_ENV`) are the daemon's own: start it from the environment the agent runs in.
- **`--watch`** reparses a file the daemon has parsed as soon as it changes, from its previous tree (tree-sitter's incremental parse), so the next invocation finds it ready. Without `--watch`, the next invocation that reads the file parses it. Under `--watch` the daemon also keeps the coverage-instrumented build warm: once a change to a Rust source or manifest settles (2 s) and no command is running, it builds what the next suite run would build first (ADR-155), never while the machine-wide heavy-command lock (`CUPEL_HEAVY_LOCK`, `/tmp/cupel-heavy.lock` by default) is held, and holding it while it builds. Correctness never depends on the watcher: every invocation reads and hashes every file it uses.
- **Lifetime.** The daemon serves until `cupel serve --stop`, its `--timeout`, Ctrl-C, or its socket file being removed. Invocations run concurrently, as separate processes would; one whose client is killed is cancelled. Starting a second daemon where one answers exits 3 with `serve.running`. The socket is readable and writable only by the user who started the daemon. A socket path is limited to about 100 bytes, so a repository whose root path is longer than about 85 characters cannot have a daemon (`serve` exits 3 with `serve.failed`).
- **`--socket PATH`** listens elsewhere; the CLI looks only at `.cupel/serve.sock`, so a daemon elsewhere serves programs that speak its protocol themselves ([ADR-009](../adr/CUPEL_DECISION_009.md): one JSON line per message).

**Performance.** Measured on the Cupel repository itself (333 Rust files, 78k lines, 4133 symbols, release build, 8-core Apple silicon, other builds running: load average 30–60):

| `cupel check --depth quick` | Cold process | Warm daemon |
|---|---|---|
| Wall time, no coverage tool installed | 0.97–1.06 s | 0.28–0.31 s |
| Wall time, `cargo llvm-cov` installed and its cached report fresh | 1.07–1.20 s | 0.42–0.47 s |

**Without the daemon.** A run also keeps what it extracted from each file in `.cupel/cupel.db`, by the file's path and content and this build of `cupel`, deflated ([ADR-188](../adr/CUPEL_DECISION_188.md)). A later process of the same build reads an unchanged file's facts from there instead of parsing it, and prints exactly what it would have printed parsing every file. On the Cupel repository (666 files parsed; release build, load average 20–45), a cold `check --depth quick` spent a median 725 ms in its static checks once the facts were kept, against 2.7 s parsing every file and 0.6 s through a warm daemon; the first run of a build, which writes them (18 MB), took about as long as one that keeps nothing. A rebuilt `cupel` reads none of an older build's facts, and its first write drops them. Each run also notes the files it used, and the facts of a file none of the last 20 such runs used are dropped, so the store holds about the current tree's facts plus recent history ([ADR-201](../adr/CUPEL_DECISION_201.md)). `--verbose` prints how many files were read from the store and how many trees were parsed.

What a warm run still does: walk the tree and hash every file, run the checks over the facts, ask git for the commit, and record the run. A run makes a check's optional facts only within its depth ([ADR-162](../adr/CUPEL_DECISION_162.md)), so `static.crap` at `quick` reads a coverage report only while a fresh cached one exists; after a change to the tree it scores no CRAP until a `standard` run measures coverage again. A `quick` run never runs the test suite.

## MCP

`cupel serve --mcp` is a [Model Context Protocol](https://modelcontextprotocol.io) server on stdin and stdout (JSON-RPC 2.0, one message per line, protocol revision `2025-11-25` or `2025-06-18`), for clients that call tools rather than run commands. Configure it as a stdio server whose working directory, or `--cwd`, is the repository:

```json
{
  "mcpServers": {
    "cupel": {
      "command": "cupel",
      "args": ["serve", "--mcp", "--cwd", "/home/me/shop"]
    }
  }
}
```

The tools, from the server's own `tools/list`:

<!-- generated:mcp-tools -->
| Tool | Runs | What it does |
| --- | --- | --- |
| `check` | `cupel check` | Run checks over a scope at a depth and evaluate the gate. Returns the envelope `cupel check --format json` prints; its exit_code and status say how the run ended. |
| `survey` | `cupel survey` | Rank a whole tree without a baseline or gate: the worst targets per metric, protect-first. Returns the envelope `cupel survey --format json` prints; its exit_code and status say how the run ended. |
| `explain` | `cupel explain` | Give full context and remediation for one finding or check result. Returns the envelope `cupel explain --format json` prints; its exit_code and status say how the run ended. |
| `tests_impacted` | `cupel tests --impacted` | List the tests a scope impacts, why, and the command that runs them. Returns the envelope `cupel tests --impacted --format json` prints; its exit_code and status say how the run ended. |
| `describe` | `cupel describe` | Describe the CLI, a verb, a check, a finding kind, a language, a fact, the support matrix, or a schema. Returns the envelope `cupel describe --format json` prints; its exit_code and status say how the run ended. |
<!-- end:mcp-tools -->

- **Arguments are the verb's flags.** Each tool's input schema is its verb's flags and the global flags as JSON properties: `--only a b` is `"only": ["a", "b"]`, `--no-gate` is `"no_gate": true`, `--limit 5` is `"limit": 5`, and a positional argument is a property too (`explain`'s `target`, `describe`'s `subject`). `format` takes only `json`, and no tool takes `--no-color`, `--events`, `--describe`, `--help`, or `--jobs`.
- **The result is the envelope.** A call runs the command line its arguments spell, in the order given, with `--format json` last unless given, exactly as a cold `cupel` run in the repository would (the result's `_meta` names that command line under `cupel.dev/command`). The envelope is the result's `structuredContent` and, for clients that show only text, its text content. `isError` is true for exit 2, 3, 4, and 6; a failed gate (exit 1) or a partial run (exit 5) is a result. An argument the schema does not have, or one of the wrong type, is a usage envelope (exit 2) naming it.
- **Events are progress.** A call with a `progressToken` receives each event of [the event stream](#global-flags) as a `notifications/progress` whose `message` is the event's JSON line and whose `_meta` holds it under `cupel.dev/event`. What the run writes to stderr arrives as `notifications/message` at level `info` (`logging/setLevel` above `info` silences it).
- **Lifetime.** The server runs until its stdin closes, its `--timeout`, or Ctrl-C; a `notifications/cancelled` stops that call's run, which gets no response. It keeps its own warm syntax trees across calls, as the daemon does, and `--watch` reparses changed files; it is not the socket daemon and runs beside one. Stdout carries only protocol messages: the server's own envelope (`serve.mcp-stopped`) goes to stderr when it ends.

## Global flags

Every verb takes these. A flag's variable sets it too, below the flag and above `cupel.toml` (see [Configuration](configuration.md)); `--no-path-plugins` is described under [Plugins](plugins.md).

<!-- generated:global-flags -->
| Flag | Default | Variable | Meaning |
| --- | --- | --- | --- |
| `--format json\|jsonl\|text\|sarif` | — | `CUPEL_FORMAT` | Output format: json (the envelope), jsonl, text, or sarif (SARIF 2.1.0); json when stdout is not a terminal, text when it is. |
| `--dry-run` | — | — | Preview what would run or be written; change nothing. |
| `--describe` | — | — | Describe this verb instead of running it, and exit 0. |
| `--config PATH` | — | — | The cupel.toml to use instead of the nearest one. |
| `--profile NAME` | — | `CUPEL_PROFILE` | Apply [profile.NAME] from cupel.toml. |
| `--cwd PATH` | — | — | The repository root; defaults to the current directory. |
| `--quiet` | — | — | Write no diagnostics to stderr. |
| `--verbose` | — | — | Write extra diagnostics to stderr. |
| `--events PATH\|stderr` | — | — | Stream progress events as JSON lines to stderr or a file under .cupel/. |
| `--timeout DURATION` | — | — | Stop after this long, keeping partial results. |
| `--seed N` | `random` | — | Seed for generative checks; recorded in the envelope. |
| `--limit N` | — | — | Return at most N findings. |
| `--offset N` | — | — | Skip the first N findings. |
| `--no-color` | — | `NO_COLOR` | Render text without color. |
| `--no-install` | — | — | Never install tools; the default unless stdout is a terminal, where Cupel asks. |
| `--install` | — | — | Install the missing tools producers need, without asking. |
| `--write` | — | — | Permit writes outside .cupel/. |
| `--no-path-plugins` | — | — | Load no cupel-check-* or cupel-produce-* plugins from PATH; [plugins] still loads. |
| `--help` | — | — | Print help and exit. |
<!-- end:global-flags -->
