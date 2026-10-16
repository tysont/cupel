# Cupel

Cupel grades the code a coding agent produced: it measures structure and test strength across a repository or a change, and returns a ranked list of what to fix next. One binary, one JSON envelope from every command, and a gate that tells an agent when it is allowed to stop.

A *cupel* is the porous cup used in fire assay to separate precious metal from base.

```sh
cupel init --write && cupel check --changed
```

That detects your project, writes `cupel.toml`, and grades everything that differs from your default branch.

## What Cupel does

Cupel runs seven static checks over JavaScript, TypeScript, Rust, Go, and Python. They measure complexity, CRAP, clones, dependency structure, dead code, test quality, and tests weakened on a branch. Every check works in every language, because checks read language-neutral facts and never see source syntax.

Every command writes one JSON envelope to stdout. It carries the checks that ran, the findings with stable IDs, the gate's verdict, and `next_actions` ranked for an agent to work through. `--format text` renders the same envelope for people.

Cupel never changes your code. It grades; the agent fixes.

- [Technical design](docs/CUPEL_TECHNICAL_DESIGN.md): the specification, covering the CLI, the output contract, the check catalog, and the architecture.
- [Implementation plan](docs/CUPEL_IMPLEMENTATION_PLAN.md): the phased build plan, kept current as work proceeds. Architecture decision records live in `docs/adr/`.

This README describes only what exists.

## Quick start

```sh
cd your-repo
cupel init --write           # detect roots and languages; write cupel.toml and .cupel/.gitignore
cupel doctor                 # verify the setup
cupel survey --depth quick   # the worst hot spots in the whole tree
cupel check --changed        # grade what changed since the merge base with your default branch
```

Add a `[gate.changed]` table to `cupel.toml` (see [Configuration](#configuration)) and `check` becomes a pass/fail verdict with exit code 0 or 1.

## Install

Build from source with a Rust toolchain. The pinned version is in `rust-toolchain.toml`.

```sh
git clone https://github.com/tysont/cupel
cd cupel
make install            # cargo install --path crates/cupel-cli
```

The binary has no runtime dependencies except `git`, which Cupel calls for `--changed` and `--base` scopes.

## The agent loop

Put one rule in your agent's instructions:

> Do not stop until `cupel check --changed --depth standard --gate` exits 0.

When it exits 1, the envelope's `next_actions` lists what to do, most important first. Each action names a closed `kind` (`reduce_complexity`, `restore_test`, `strengthen_assertion`, `refactor_extract`, …), a summary written for the agent, and the finding it came from. Each finding carries a `repro` command that reruns only its check, on only its symbol or range, so the agent can confirm a fix cheaply before rerunning the whole gate.

Every command exits with one of these codes, and the envelope's `exit_code` always matches:

<!-- generated:exit-codes -->
| Code | Name | Meaning |
| --- | --- | --- |
| 0 | pass | Ran fully; gate passed or no gate. |
| 1 | fail | Ran fully; gate failed. |
| 2 | usage | Bad flags or config; the envelope names the fix. |
| 3 | environment | Missing tool, oracle, or toolchain; doctor findings attached. |
| 4 | unevaluable | Baseline test suite fails or build broken; scoring refused. |
| 5 | partial | Timeout or budget hit; the envelope has valid partial results. |
| 6 | internal | Cupel bug; the envelope includes a bug-report payload. |
<!-- end:exit-codes -->

## Usage

Every verb takes the same global flags (below) and writes the same envelope. Run any verb with `--describe` to see its flags instead of running it.

### `cupel init`

`init` detects project roots and languages and chooses test, coverage, and bench commands for each. It understands Cargo crates and workspaces, Go modules and workspaces, npm, pnpm, and yarn packages and workspaces (TypeScript when `tsconfig.json` is present), and Python projects (`pyproject.toml`, `setup.cfg`, `requirements.txt`).

```sh
cupel init                   # preview: the envelope carries unified diffs of what would be written
cupel init --write           # write cupel.toml and .cupel/.gitignore
cupel init --write --force   # replace an existing cupel.toml
```

### `cupel doctor`

`doctor` verifies the binary against `required_version`, the validity of `cupel.toml`, and write access to `.cupel/`. For every tool a configured check needs through its facts (a test runner, for a check that needs test results; a coverage tool, for a language with a `coverage` command), it checks the tool is on `PATH` and reports its version under `run.toolchain`; a `[lang.<id>] runner` that names no runner is a finding listing the valid ones. Every problem is a finding with a `configure` or `install_tool` action. Exit 3 means something required is missing.

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

Installs run in the repository root with your environment, so the tool lands where your shell would put it: `CARGO_HOME`, the Python `PATH` finds (activate a virtual environment first), or the root's `node_modules`. With `--install` nothing is asked; without it Cupel asks on stderr when stdout is a terminal and installs nothing otherwise, and `--no-install` never installs. After installing, `doctor` probes again: `install.succeeded` names the version now installed, and `install.failed` (exit 3) carries the command and the tail of its stderr. `install.skipped` says why `--fix` installed nothing. `cupel check` and `cupel survey` follow the same flags for the tools their checks need, before they run; `--dry-run` lists those installs too.

### `cupel describe`

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

### `cupel check`

`check` is the workhorse. It resolves a scope, selects checks, produces the facts those checks need, runs them, applies baselines and suppressions, and evaluates the gate.

**Scope** decides which files and symbols are graded. Scope flags intersect.

| Flag | Scope |
|---|---|
| (none) | The whole tree, minus `[scope] exclude` and always `target`, `node_modules`, `vendor`, and `.cupel` |
| `--changed` | Files changed since the merge base with `default_branch`, including uncommitted work |
| `--base REF [--head REF]` | Files changed in an explicit range; `--head` defaults to the working tree |
| `--paths P` | Files, directories, or globs (gitignore-aware); repeatable |
| `--symbols S` | Symbols by qualified name (`shop.shipping.shipping_cost`) or structural hash (`sh_c7dca337c9ad7dd1`); repeatable |

A `--changed` scope with no changes passes with `scope.symbols: 0`. `tests.weakening` compares tests between two revisions, so it runs only with `--changed` or `--base` and is reported as skipped on a tree.

**Selection** decides which checks run.

| Flag | Selects |
|---|---|
| `--depth quick\|standard\|deep\|exhaustive` | How much evidence to gather; default `standard`. Every check is `quick`, so all seven run at every depth |
| `--only ID`, `--skip ID` | Checks by ID (`static.clones`) or group (`static`, `tests`); repeatable |
| `--tier N+` | Only checks at or above evidence tier N, e.g. `--tier 2+` |

**Grading** flags decide what the result is compared with.

| Flag | Effect |
|---|---|
| `--gate`, `--no-gate` | Evaluate `[gate.changed]` for range scopes or `[gate.tree]` otherwise. On by default when the table exists; `--gate` without the table is exit 2 |
| `--against baseline\|REF\|run:ID` | What `new_since_baseline` and each check's `delta` are computed against |
| `--dry-run` | Return the plan (checks, targets, facts, estimates, files it would write) and run nothing |
| `--format json\|jsonl\|text` | JSON when stdout is not a terminal, text when it is |

Here is a small Python repository. An agent's branch added a shipping calculator, loosened one test's assertion, and skipped another test. The gate fails:

```console
$ cupel check --changed --depth standard --gate --format text
cupel check: fail (exit 1)

gate      fail (gate.changed)
  fail  tests.weakening none  actual 2
  fail  static.complexity none  actual 1
  pass  static.clones none  actual 0

summary   6 findings · fail 2 · warn 2 · info 2 · new 0 · resolved 0 · suppressed 0

checks
  static.clones      pass     tier 0 · quick · score 0 pairs (n 2) · metrics clusters 0, max_jaccard 0, pairs 0 · 0 findings · 0 ms
  static.complexity  fail     tier 0 · quick · score 15 cyclomatic_max (n 4) · metrics cognitive_max 19, cyclomatic_max 15, length_max 27, nesting_max 2, params_max 5 · 1 finding · 4 ms
  static.crap        pass     tier 0 · quick · 0 findings · 0 ms
  static.dead-code   pass     tier 0 · quick · score 0 dead_symbols (n 4) · metrics dead_exports 2, dead_symbols 0 · 2 findings · 63 ms
  static.deps        pass     tier 0 · quick · score 0 cycles (n 2) · metrics cycles 0, max_distance 0, violations 0 · 0 findings · 0 ms
  tests.quality      fail     tier 2 · quick · score 1 assertion_density (n 2) · metrics assertion_density 1, no_assertion 0, weak_ratio 0.5 · 1 finding · 0 ms
  tests.weakening    fail     tier 2 · quick · score 2 weakened (n 2) · metrics weakened 2 · 2 findings · 0 ms

fail (2)
  tests.weakening
    `test_discount_reduces_the_total` replaced 1 exact assertion(s) with weaker ones
      fnd_0c6fc5 · matcher.loosened · tier 2 · confidence 1
      at tests/test_pricing.py:6:1-7:47
      evidence: strong_base 1 · strong_head 0 · test "test_discount_reduces_the_total" · weak_base 0 · weak_head 1 (full: cupel explain fnd_0c6fc5)
      action restore_test (priority 1): Restore the exact expected values in `test_discount_reduces_the_total` instead of truthiness, presence, or inequality checks.
      repro: cupel check --only tests.weakening --base 18dcc9167bc03aa9f0e682c90347be9425b10751
      suppress: cupel suppress fnd_0c6fc5 --reason '…'
    `test_discount_rejects_a_negative_percent` is now skipped
      fnd_2b25cf · test.skipped · tier 2 · confidence 1
      …

warn (2)
  static.complexity
    `shop.shipping.shipping_cost` is too complex: cyclomatic 15 (max 10), cognitive 19 (max 15)
      fnd_207cfa · complexity.high · tier 0 · confidence 1
      at src/shop/shipping.py:1:1-27:16 in shop.shipping.shipping_cost (python, sh_c7dca337c9ad7dd1)
      evidence: cognitive 19 · cognitive_max 15 · cyclomatic 15 · cyclomatic_max 10 (full: cupel explain fnd_207cfa)
      action reduce_complexity (priority 2): Split `shipping_cost` into smaller functions or replace conditional chains with data, until cyclomatic is at most 10 and cognitive at most 15.
      repro: cupel check --only static.complexity --symbols sh_c7dca337c9ad7dd1
      suppress: cupel suppress fnd_207cfa --reason '…'
  tests.quality
    `tests.test_pricing.test_discount_reduces_the_total` asserts only weakly: 1 weak assertion(s), none pinning a value
      …

info (2)
  static.dead-code
    `shop.shipping.shipping_cost` is exported but no import resolves to src/shop/shipping.py
      …

run       run_01M4X8VR2HBT70ZRQAKXSM7MFV · started 2026-10-14T13:15:38Z · 261 ms · seed 9032483245384408524
git       2c3d599be0141ca3d03368b508e93f4c91068c27 · branch agent/shipping · base 18dcc9167bc03aa9f0e682c90347be9425b10751 · clean
scope     changed · 2 files · 4 symbols · roots . · languages python
…

next actions
  1. restore_test: Restore the exact expected values in `test_discount_reduces_the_total` instead of truthiness, presence, or inequality checks. (finding fnd_0c6fc5)
  2. restore_test: Remove the skip marker from `test_discount_rejects_a_negative_percent` and fix what made it fail. (finding fnd_2b25cf)
  3. strengthen_assertion: Replace the truthiness, presence, or inequality checks in `test_discount_reduces_the_total` with an assertion that pins the expected value. (finding fnd_fe3613)
  4. reduce_complexity: Split `shipping_cost` into smaller functions or replace conditional chains with data, until cyclomatic is at most 10 and cognitive at most 15. (finding fnd_207cfa)
  …
```

The same run as JSON, trimmed to two checks, one finding, and one action:

```json
{
  "cupel": { "version": "0.1.0", "schema": "https://cupel.dev/schema/v1/run.json" },
  "run": {
    "id": "run_01M4X8VR2HBT70ZRQAKXSM7MFV", "verb": "check",
    "started": "2026-10-14T13:15:38Z", "duration_ms": 261,
    "git": { "sha": "2c3d599be0141ca3d03368b508e93f4c91068c27", "dirty": false,
             "base": "18dcc9167bc03aa9f0e682c90347be9425b10751", "branch": "agent/shipping" },
    "scope": { "mode": "changed", "files": 2, "symbols": 4, "roots": ["."], "languages": ["python"] },
    "depth": "standard", "seed": 9032483245384408524, "…": "…"
  },
  "status": "fail",
  "exit_code": 1,
  "checks": [
    {
      "id": "static.complexity", "status": "fail", "tier": 0, "cost": "quick",
      "score": { "value": 15.0, "unit": "cyclomatic_max", "n": 4 },
      "metrics": { "cognitive_max": 19.0, "cyclomatic_max": 15.0, "length_max": 27.0, "nesting_max": 2.0, "params_max": 5.0 },
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
      "id": "fnd_207cfa", "check": "static.complexity", "kind": "complexity.high",
      "severity": "warn", "confidence": 1.0, "tier": 0,
      "symbol": { "display": "shop.shipping.shipping_cost", "hash": "sh_c7dca337c9ad7dd1", "lang": "python" },
      "location": { "file": "src/shop/shipping.py", "line": 1, "col": 1, "end_line": 27, "end_col": 16 },
      "title": "`shop.shipping.shipping_cost` is too complex: cyclomatic 15 (max 10), cognitive 19 (max 15)",
      "detail": null,
      "evidence": { "cognitive": 19, "cognitive_max": 15, "cyclomatic": 15, "cyclomatic_max": 10 },
      "action": {
        "kind": "reduce_complexity", "priority": 2, "hints": {},
        "summary": "Split `shipping_cost` into smaller functions or replace conditional chains with data, until cyclomatic is at most 10 and cognitive at most 15."
      },
      "repro": "cupel check --only static.complexity --symbols sh_c7dca337c9ad7dd1",
      "suppress": "cupel suppress fnd_207cfa --reason '…'",
      "new_since_baseline": false,
      "suppressed": false
    }
  ],
  "summary": {
    "findings": { "fail": 2, "warn": 2, "info": 2 },
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
  "total_findings": 6
}
```

Every object has a fixed key set; an absent value is `null`, never a missing key (a check result's `notes`, omitted when empty, is the one exception). `status` is one of `pass`, `fail`, `partial`, `error`, or `plan`. A check result in `error` says why in its `notes`, such as a tool that could not run. `--format jsonl` writes the envelope header and then one finding per line. `cupel describe schema run` and `cupel describe schema finding` are the full schemas.

### `cupel survey`

`survey` is the first command to run on an unfamiliar repository. It takes the same scope and selection flags as `check`, never gates, and adds `rankings`: the ten worst targets for every per-symbol, per-module, and per-group metric.

```console
$ cupel survey --depth quick --format text
cupel survey: pass (exit 0)

summary   5 findings · fail 0 · warn 3 · info 2 · new 0 · resolved 0 · suppressed 0

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

warn (3)
  static.clones
    `shop.pricing.apply_discount` and `shop.shipping.apply_shipping_discount` (src/shop/shipping.py:30) are type-2 clones (jaccard 1.00)
      fnd_c5c4da · clone.pair · tier 0 · confidence 1
      …
```

On the tree, `survey` finds the clone the changed-scope `check` above did not: `check --changed` builds facts only for the changed files, so a new function that copies an unchanged one is visible to `survey` and to tree-scope `check`, not to `--changed`.

### `cupel explain`

`explain` reads the fact store and never re-runs anything. Given a finding ID it returns the stored finding with a source excerpt and its full evidence. Given a check ID it returns that check's result, findings, and metrics from the last run that included it.

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

### `cupel baseline`

A baseline is a stored run that later runs are compared with. Findings absent from the baseline are marked `new_since_baseline`, and each check reports a `delta` against it. Writing a baseline changes only `.cupel/`, but `set` and `import` still preview until you pass `--write`.

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

### `cupel suppress`

`suppress` silences one finding with a reason and an optional expiry date or git ref. It writes `.cupel/suppressions.toml`, the one file under `.cupel/` meant to be committed. A suppressed finding still appears, with `suppressed: true` and severity `info`, and no longer counts against the gate. When the expiry passes, the finding is active again and a `suppression.expired` finding says so.

```console
$ cupel suppress fnd_af9323 --reason "public API; the checkout service imports it" --until 2027-01-31 --format text
cupel suppress: pass (exit 0)

summary   1 finding · fail 0 · warn 0 · info 1 · new 0 · resolved 0 · suppressed 1

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

### `cupel tests --impacted`

`tests --impacted` lists the tests a scope impacts, why, and a ready-to-run command per runner. It takes the same scope flags as `check`; with `--changed` or `--base` it reads the changed lines and widens each change to the functions it touches. Where a language has a per-test coverage map (Rust through `cargo llvm-cov` and `cargo nextest`, Go through `go test -coverprofile` per test, Python through pytest-cov's test contexts), a test is impacted when it executes one of those lines. JavaScript and TypeScript have no per-test map, so their impacted tests are the test files that import a changed file, directly or through other files; the envelope says so with `per_test_map: false` and the reason. The same happens for any language whose coverage tool is not installed.

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

Measuring a per-test map costs a test run under coverage: one instrumented `cargo llvm-cov nextest` run for Rust (each test's coverage is told apart by process), one pytest-cov run for Python, and one `go test` per test for Go. Each project root's map is cached under `.cupel/cache/per-test/<producer>/`, with the hash of every file under the root, so a later run reuses it. After a change, only the stale tests are measured again: those that cover a changed file or whose own test file changed (for Go, any test file of their package), plus tests the cache does not hold; tests that are gone are dropped. A change to build or test configuration (`Cargo.toml`, `Cargo.lock`, `go.mod`, `conftest.py`, `pyproject.toml`, and the like) measures every test again, and so does deleting the cache. The text report says how many tests were measured, or `(cached)` when none was.

### `cupel serve`

`serve` runs a daemon for one repository in the foreground. It keeps every file's syntax tree, keyed by a hash of the file's content, and what each fact producer extracted from it, across invocations, so a warm `check --depth quick` neither parses nor re-extracts an unchanged file. While `.cupel/serve.sock` exists and answers, every `cupel` invocation in the repository is answered by the daemon, and prints exactly what it would print without it: the same envelope, the same stderr, the same exit code. `--events stderr` lines stream to the invoking process as the run writes them.

```console
$ cupel serve --watch
cupel: serving /home/me/shop at .cupel/serve.sock (pid 72908, watching)
```

In another terminal, `doctor` reports the daemon, and `serve --stop` stops it:

```console
$ cupel doctor --format text
cupel doctor: pass (exit 0)

summary   1 finding · fail 0 · warn 0 · info 1 · new 0 · resolved 0 · suppressed 0

info (1)
  doctor
    The daemon at .cupel/serve.sock serves this repository (pid 72908)
      fnd_c243c8 · doctor.daemon-running · tier 0 · confidence 1
      evidence: pid 72908 · socket ".cupel/serve.sock" · version "0.1.0" · watching true (full: cupel explain fnd_c243c8)
      …

$ cupel serve --stop --format text
cupel serve: pass (exit 0)

summary   1 finding · fail 0 · warn 0 · info 1 · new 0 · resolved 0 · suppressed 0

info (1)
  serve
    Stopped the daemon at .cupel/serve.sock
      fnd_8ec73e · serve.stopped · tier 0 · confidence 1
      …
```

```console
$ cupel check --format json > envelope.json      # cupel was rebuilt after the daemon started
cupel: ran without the daemon: the daemon at /home/me/shop/.cupel/serve.sock runs another build of cupel (cupel 0.1.0 (binary 56427528-1792134645316655513)) than this client (cupel 0.1.0 (binary 56427528-1792134881357328000)). Restart it from this one: `cupel serve --stop`, then `cupel serve`.
```

- **When the CLI uses it.** Every invocation whose repository root (`--cwd`, else the working directory) has a `.cupel/serve.sock`. It runs in its own process instead when `CUPEL_NO_DAEMON=1` is set, for `serve` itself, when an argument is not UTF-8, when the run could ask a question on the terminal (stdout is a terminal and neither `--install` nor `--no-install` is given), and when nothing answers on the socket (a daemon that was killed leaves its socket file behind; it is ignored).
- **When it declines.** A daemon runs only invocations from the same build of `cupel` (the same version and the same executable) and with the same `PATH`, since `PATH` decides which tools run. Otherwise the invocation runs in its own process and says why in one line on stderr (unless `--quiet`); rebuilding or reinstalling `cupel` therefore means restarting the daemon. Other variables (`HOME`, `CARGO_HOME`, `VIRTUAL_ENV`) are the daemon's own: start it from the environment the agent runs in.
- **`--watch`** reparses a file the daemon has parsed as soon as it changes, from its previous tree (tree-sitter's incremental parse), so the next invocation finds it ready. Without `--watch`, the next invocation that reads the file parses it. Correctness never depends on the watcher: every invocation reads and hashes every file it uses.
- **Lifetime.** The daemon serves until `cupel serve --stop`, its `--timeout`, Ctrl-C, or its socket file being removed. Invocations run concurrently, as separate processes would; one whose client is killed is cancelled. Starting a second daemon where one answers exits 3 with `serve.running`. The socket is readable and writable only by the user who started the daemon. A socket path is limited to about 100 bytes, so a repository whose root path is longer than about 85 characters cannot have a daemon (`serve` exits 3 with `serve.failed`).
- **`--socket PATH`** listens elsewhere; the CLI looks only at `.cupel/serve.sock`, so a daemon elsewhere serves programs that speak its protocol themselves ([ADR-009](docs/adr/CUPEL_DECISION_009.md): one JSON line per message).

**Performance.** Measured on the Cupel repository itself (333 Rust files, 78k lines, 4133 symbols, release build, 8-core Apple silicon, other builds running: load average 30–60):

| `cupel check --depth quick` | Cold process | Warm daemon |
|---|---|---|
| Wall time, no coverage tool installed | 0.97–1.06 s | 0.28–0.31 s |
| Wall time, `cargo llvm-cov` installed and its cached report fresh | 1.07–1.20 s | 0.42–0.47 s |

What a warm run still does: walk the tree and hash every file, run the checks over the facts, ask git for the commit, and record the run. When `cargo llvm-cov` (or another coverage tool) is installed and configured, `static.crap` wants a coverage report even at `quick`: a fresh cached report is reused, but after any change to the tree the next run measures coverage again with the whole test suite, cold or warm alike.

### Global flags

| Flag | Meaning |
|---|---|
| `--format json\|jsonl\|text` | JSON when stdout is not a terminal, text when it is; `CUPEL_FORMAT` also sets it |
| `--dry-run` | Preview what would run or be written; change nothing |
| `--describe` | Describe the verb instead of running it |
| `--config PATH`, `--profile NAME` | Use this `cupel.toml`, or apply `[profile.NAME]` from it |
| `--cwd PATH` | The repository root; Cupel never relies on the shell's working directory |
| `--events PATH\|stderr` | Stream progress as JSON lines; stdout stays reserved for the envelope |
| `--timeout DURATION` | Stop after this long with partial results and exit 5 |
| `--seed N` | Recorded in the envelope; random by default |
| `--limit N`, `--offset N` | Page findings; the envelope always carries `truncated` and `total_findings` |
| `--install`, `--no-install` | Install missing tools without asking, or never; without either, Cupel asks when stdout is a terminal and installs nothing otherwise (`doctor --fix`, `check`, `survey`) |
| `--write` | Permit writes outside `.cupel/`; installing a tool is governed by `--install` instead |
| `--quiet`, `--verbose`, `--no-color` | Diagnostics on stderr only; stdout's shape never changes |

## Checks

Every check below exists and is supported in all five languages. All run at `--depth quick` except `tests.coverage`, `tests.flaky`, and `tests.mutation`, which run the project's test suite and so run from `--depth standard`, the default. The table is generated from `cupel describe` (see [Development](#development)). Tier is the strength of the evidence, from 0 (structural) upward. Gate metrics are the keys a gate rule can name as `<check>.<metric>`; ranked metrics are what `survey` ranks.

<!-- generated:check-table -->
| Check | Tier | Cost | Finding kinds | Gate metrics | Ranked by | Languages |
| --- | --- | --- | --- | --- | --- | --- |
| `static.clones` | 0 | quick | `clone.pair`, `clone.cluster` | `pairs`, `clusters`, `max_jaccard` | `cluster_size` | go, javascript, python, rust, typescript |
| `static.complexity` | 0 | quick | `complexity.high`, `nesting.deep`, `params.many`, `function.long` | `cyclomatic_max`, `cognitive_max`, `nesting_max`, `params_max`, `length_max` | `cyclomatic`, `cognitive` | go, javascript, python, rust, typescript |
| `static.crap` | 0 | quick | `crap.high`, `crap.watch` | `max`, `mean` | `crap` | go, javascript, python, rust, typescript |
| `static.dead-code` | 0 | quick | `dead.symbol`, `dead.export` | `dead_symbols`, `dead_exports` | — | go, javascript, python, rust, typescript |
| `static.deps` | 0 | quick | `dep.cycle`, `dep.layer-violation`, `dep.unstable-abstract` | `cycles`, `max_distance`, `violations` | `distance`, `coupling`, `cycle_size` | go, javascript, python, rust, typescript |
| `tests.coverage` | 2 | standard | `coverage.uncovered-symbol`, `coverage.uncovered-branch` | `line`, `branch`, `lines`, `branches`, `uncovered` | `uncovered_lines` |  |
| `tests.flaky` | 2 | standard | `test.flaky`, `test.order-dependent` | `flaky`, `order_dependent` | — | go, javascript, python, rust, typescript |
| `tests.mutation` | 3 | standard | `mutant.survived`, `mutant.timeout`, `mutant.uncovered`, `mutant.equivalent?` | `score`, `killed`, `survived`, `timeout`, `uncovered`, `unviable`, `equivalent`, `executed`, `reused`, `schemata`, `fallback` | `survivors` |  |
| `tests.quality` | 2 | quick | `test.weak-assertion`, `test.no-assertion`, `test.mock-heavy`, `test.logic`, `test.duplicate-setup` | `assertion_density`, `weak_ratio`, `no_assertion` | `mock_density` | go, javascript, python, rust, typescript |
| `tests.weakening` | 2 | quick | `assertion.removed`, `test.skipped`, `matcher.loosened`, `snapshot.rewritten`, `test.deleted` | `weakened` | — | go, javascript, python, rust, typescript |
<!-- end:check-table -->

What each check looks for:

- **`static.complexity`** measures every function's cyclomatic and cognitive complexity, nesting depth, parameter count, and length against configurable limits. A `match` or `switch` counts as one decision however many arms it has.
- **`static.crap`** scores each function by complexity weighted by missing test coverage, from the language's coverage report (see [Coverage](#coverage)); without one it passes with a null score and no findings rather than guess.
- **`static.clones`** finds functions whose normalized syntax trees are mostly the same, so renamed variables and reformatting do not hide a copy. Two members are a `clone.pair`; three or more are a `clone.cluster`.
- **`static.deps`** builds the module graph from resolved imports. It reports import cycles, modules that are both unstable and abstract, and imports that break the layering you configure.
- **`static.dead-code`** reports symbols and exports in modules that no import reaches and no entry point, test, or framework hook keeps alive. Reachability is decided per module, not per call, so an export of a module nothing imports is reported as `info` with low confidence: it may be public API.
- **`tests.quality`** reads every test: tests with no assertions or only weak ones (truthiness, presence, inequality), heavy mocking, loops and branches inside tests, and duplicated setup.
- **`tests.weakening`** compares the tests at the range's base with the tests now. It reports removed or loosened assertions, skipped or deleted tests, and rewritten snapshots. It needs `--changed` or `--base`.
- **`tests.flaky`** reruns the test suite several times in different orders and with different seeds, and reports tests whose outcome changes: `test.flaky` when two reruns in the same order disagree, `test.order-dependent` when the outcome follows the order. See [Flakiness](#flakiness).
- **`tests.mutation`** changes the code in small ways (`<` to `<=`, `+` to `-`, `&&` to `||`, `true` to `false`, `0` to `1`) and runs the tests that cover each change. A mutant every test passes against survived: the tests do not notice that line changing. The score is killed / (killed + survived). See [Mutation testing](#mutation-testing).

`cupel describe <check>` and `cupel describe finding <kind>` give the full contract, including each finding kind's evidence schema.

## Languages

Cupel understands **JavaScript, TypeScript, Rust, Go, and Python**. Each language is a tree-sitter grammar plus a declarative spec (queries and tables), and produces the same language-neutral facts: `symbols`, `decisions`, `norm-tree` (the structural hash that gives every function an identity surviving rename, move, and reformat), `mutation-sites`, `test-frames`, `test-conventions`, `module-graph` (imports resolved to files, with Node, `tsconfig` `paths`, Go module, Rust `mod`, and Python package resolution), `api-surface`, `roots`, and `project-info`. Checks consume facts and never see a language, so every check works in every language that produces what it needs.

The support matrix, from `cupel describe matrix`:

<!-- generated:support-matrix -->
| Check | go | javascript | python | rust | typescript |
| --- | --- | --- | --- | --- | --- |
| `static.clones` | supported | supported | supported | supported | supported |
| `static.complexity` | supported | supported | supported | supported | supported |
| `static.crap` | supported | supported | supported | supported | supported |
| `static.dead-code` | supported | supported | supported | supported | supported |
| `static.deps` | supported | supported | supported | supported | supported |
| `tests.coverage` | unconfigured | unconfigured | unconfigured | unconfigured | unconfigured |
| `tests.flaky` | supported | supported | supported | supported | supported |
| `tests.mutation` | unconfigured | unconfigured | unconfigured | unconfigured | unconfigured |
| `tests.quality` | supported | supported | supported | supported | supported |
| `tests.weakening` | supported | supported | supported | supported | supported |
<!-- end:support-matrix -->

```sh
cupel describe languages           # the five languages, one line each
cupel describe language go         # extensions, facts produced, checks that apply, toolchain row
cupel describe fact norm-tree      # a fact's schema, version, producers per language, consumers
```

JS and TS form one family: `static.clones` reports a TypeScript function copied from JavaScript. Languages in different families are never compared. Every language is held to the same conformance inventory, a shared list of constructs per fact that each language must exhibit in its fixture repository and detect, or mark not applicable with a reason.

## Test runners

Checks that depend on running the test suite read a `test-results` fact, which Cupel makes by running the project's own test runner and reading its machine-readable output:

| Language | Runners, in default order | Reads |
| --- | --- | --- |
| Rust | `cargo nextest` (when `.config/nextest.toml` exists), `cargo test` | JUnit through a nextest tool profile; libtest output |
| Go | `go test -json` | test2json events |
| Python | `pytest` (when the project mentions it), `unittest` | JUnit XML; `unittest -v` output |
| JavaScript, TypeScript | `vitest`, `jest` (whichever `package.json` depends on) | the Jest JSON report |

A runner is used when its program is on `PATH` (or, for `vitest` and `jest`, in the root's `node_modules/.bin`); otherwise checks that need test results are skipped as `unavailable` with the tool named, and `cupel doctor` says what to install (`cupel doctor --fix --install` installs it). `[lang.<id>] runner` chooses one explicitly, by its name with or without the `tool.` prefix:

```toml
[lang.python]
runner = "unittest"
```

Before any check that needs test results runs, Cupel runs the suite once in the real tree, the **baseline**. If any test fails, or the runner cannot finish (a build error), the run is refused: `status: "error"`, exit code 4, no check scores, and one `baseline.test-failed` finding per failing test (with its file, runner, and the tail of its output) or `baseline.run-incomplete` per run that did not finish. Fix the suite, then rerun. A mutation or flakiness score over a red suite would mean nothing, so no mutant and no rerun runs before the baseline is green. Every test of the baseline is also indexed in `.cupel/cupel.db`. `tests.flaky` and `tests.mutation` need test results, so `cupel check` at the default depth (`standard`) runs the baseline; at `--depth quick` no test runs. A language whose baseline ran no test at all skips them as `unavailable`.

### Flakiness

`tests.flaky` reruns the tests the baseline ran, three times by default, with the same runner, and compares the outcomes. The reruns differ on purpose: the first two run the tests in their declared order one at a time, differing only in seed; the third runs them reversed (pytest and unittest, which take the tests in order) or in the runner's seeded shuffle (`go test -shuffle`, libtest's `--shuffle-seed`, `jest --randomize`, `vitest --sequence.shuffle`); further reruns shuffle again, alternating between the runner's default parallelism and one test at a time. Every rerun's seed is derived from the run's `--seed`, so `cupel check --only tests.flaky --seed N`, which each finding gives as its `repro`, repeats the same reruns.

- `test.flaky`: the test passed and failed in two reruns that ran the tests in the same order. Something besides the code and the order decides it: time, randomness, concurrency, or the environment.
- `test.order-dependent`: the outcome is the same in every rerun of one order and differs between orders. The evidence lists `preceded_by`, the tests that ran before it in every rerun it failed and after it in some rerun it passed, when the runner reports its order.

```toml
[lang.rust]
flake_runs = 5      # reruns per check run; 0 turns them off
```

The reruns repeat the whole suite (impacted-test selection is not built yet), so they cost about three baseline runs. `cargo nextest` runs each test in its own process, where order cannot matter; with it as the runner, the flake engine does not rerun and `tests.flaky` ends in `error` for that language.

## Coverage

`tests.coverage` and `static.crap` read a `coverage-map` fact: line hits, and branch hits where the tool measures them. Cupel runs the coverage command the repository configures — `cupel init --write` writes one per language — and reads the report it leaves, in any of LCOV, Cobertura XML, Istanbul JSON, Go coverprofile, or JaCoCo XML:

```toml
[lang.rust]
coverage = "cargo llvm-cov --lcov --output-path {out}"    # {out}: a report path Cupel chooses

[lang.typescript]
coverage = "npx vitest run --coverage --coverage.reporter=lcov"   # no {out}: coverage/lcov.info
```

| Language | Coverage producers, in default order | Branches |
|---|---|---|
| Rust | `cargo llvm-cov` | no (stable llvm-cov reports none) |
| Go | `go test -coverprofile` | no (coverprofile has none) |
| Python | `pytest-cov` (with pytest), `coverage.py` (with unittest) | with `--cov-branch` |
| JavaScript, TypeScript | `vitest --coverage`, `jest --coverage`, `c8` | yes |

Without a `coverage` command a language's `tests.coverage` is skipped as `unconfigured` and `static.crap` reports no CRAP; with one but without the tool, it is `unavailable` and `cupel doctor` names the tool. A report is reused instead of rerunning the suite while it is fresh: Cupel records the hash of every file under the project root when it makes one (and trusts files not modified since), and a report the tool left at its default location is used when it is newer than every source. A command that exits non-zero gives no coverage and a `coverage.run-failed` finding, since a red suite has no meaningful coverage.

`tests.coverage` reports `coverage.uncovered-symbol` for each function no test ran (a warning, which fails the check) and `coverage.uncovered-branch` for a function that ran with a branch no test took. Where the tool measured no branches (Go, Rust), there are no branch findings and the result says `notes: ["branches: unavailable (go)"]`. Its gate keys are `tests.coverage.line` and `tests.coverage.branch` (covered shares), `lines`, `branches`, and `uncovered`. `static.crap` computes `CC² × (1 − coverage)³ + CC` from the same map.

## Mutation testing

`tests.mutation` scores a `mutation-outcomes` fact, which the mutation engine makes from the `mutation-sites` every language produces, the coverage report, and the baseline. The engine runs each mutant with the same runner as the baseline; the check only counts and reports, so another producer of the fact (a wrapper around a mutation tool) would feed the same check. Coverage gates which mutants run, so a language without a `coverage` command skips `tests.mutation` as `unconfigured`, as it does `tests.coverage`.

- `mutant.survived` (fails the check): every test that ran against the mutant passed. The finding sits at the mutated site and names the change (`from`, `to`, `operator`), the tests that missed it (`covering_tests`), and the nearest test file. Write a test that fails on the mutant.
- `mutant.uncovered` (a note): no test runs the site, so its mutant was not run.
- `mutant.timeout` (a note): the tests ran past their timeout against the mutant, which usually means it loops.
- `mutant.equivalent?` (a note, confidence 0.3): the mutant survived but may behave exactly as the original — today, swapping `+`/`-` before a literal `0` or `*`/`/` before a literal `1`. It is left out of the score; suppress it once you agree.

The score `tests.mutation.score` is killed / (killed + survived), 1 when nothing was scored. The other gate keys count mutants by outcome (`killed`, `survived`, `timeout`, `uncovered`, `unviable` for one that did not build, `equivalent`) and say how many ran this time (`executed`) or were reused (`reused`), and how many of those ran behind a schemata build (`schemata`) or fell back to file copies (`fallback`).

How much is mutated follows the depth: at `standard` with `--changed` or `--base`, the functions the change touched; at `deep` and `exhaustive`, every function of the files in scope (the touched files with `--changed`, the tree without). Test code is never mutated. Only the `default` operators run unless the language asks for more:

```toml
[lang.python]
mutation = { operators = "extended", jobs = 2, timeout_factor = 10 }

[lang.rust]
mutation = { strategy = "file-copy" }   # "schemata" (the default) or "file-copy"
```

Mutation testing is expensive, so the engine stacks every saving it can:

1. **Coverage gating.** A site no test runs, by the coverage report, is reported `uncovered` and never run.
2. **Reuse.** An outcome is stored in `.cupel/cupel.db` under a key of the function's source text, the site, the replacement, the tests that cover it, and the runner's version; a later run reuses it while that key holds. A second run with nothing changed runs no mutant; changing one test file reruns only the mutants that test covers.
3. **Per-test selection.** Where the language has a per-test coverage map from the baseline's runner (Go; Python with pytest-cov; Rust when cargo-nextest is the runner and cargo-llvm-cov is installed), a mutant runs only the tests that execute its line. Measuring a map from nothing happens at `--depth deep` and beyond; at `standard` the engine uses a cached map (see [`cupel tests --impacted`](#cupel-tests---impacted)) when refreshing it is cheap: one suite run for Rust and Python, at most 32 tests for Go. `cupel check --dry-run` says which, in the check's `needs`: `per-test-map(cached)`, `per-test-map(refresh 3 stale)`, or `per-test-map(deep)` when the depth leaves it out. Without a map a mutant runs the whole suite.
4. **Schemata (Rust).** Instead of one build per mutant, each Rust file is rewritten once with every mutant behind a run-time switch — the smallest whole expression around each site becomes `if cupel_mutant_id() == N { mutated } else { original }`, and a small reader of the `CUPEL_MUTANT` environment variable is appended to the file — so the file builds once and each mutant runs with `CUPEL_MUTANT=N`. Every line stays where it was. Before any mutant runs, each worker builds the rewritten file and runs its tests with no mutant selected; they must pass. Sites where an `if` cannot stand (patterns, `const` and `static` items, `const fn`, array lengths, macro arguments, attributes) and constants the compiler checks run as ordinary file copies. If the rewritten file does not compile, the sites on the lines the compiler names run as file copies and the file is rebuilt; if it still fails, the whole file runs as file copies. Nothing is lost either way: outcomes are the same under both, which the tests check on the Rust fixtures. The check reports how many mutants ran each way (`schemata`, `fallback`) and a note for each reason a mutant fell back. Other languages build per mutant (Go's builds take a fraction of a second; see [ADR-005](docs/adr/CUPEL_DECISION_005.md)).
5. **Overlays.** Mutants run in copies of the tree outside the repository, one per worker, kept between runs so builds stay incremental: files are cloned (macOS) or hard-linked, what `.gitignore` ignores is shared through symlinks except the runner's build directory (`target/` for cargo), and the mutated file is a private copy. Mutants of one file run concurrently; files run one after another.
6. **Timeouts.** Before its first mutant of a file, each worker times the unmutated file's tests (for Rust, its schemata build's), rebuild included; a mutant gets ten times that (`timeout_factor`), at least two seconds.
7. **Order.** Last run's survivors run first, then mutants of functions never run before, then the rest, so a time limit cuts the least informative work.

A Rust file's mutants cost one build per worker plus one test run each; outside Rust, each mutant costs an incremental build of what it changes plus the tests. The run's `--seed` reaches every mutant run.

## Configuration

`cupel.toml` is found by walking up from `--cwd`. Precedence is flags, then `CUPEL_*` environment variables (`CUPEL_DEPTH`, `CUPEL_FORMAT`, `CUPEL_PROFILE`), then `--profile`, then `cupel.toml`, then defaults. `CUPEL_NO_DAEMON=1` makes an invocation run in its own process even when a daemon serves the repository (see `cupel serve`). The envelope echoes the effective configuration and the layers that contributed under `run.config`. Unknown keys are usage errors (exit 2) that list the valid keys, and `cupel describe schema config` is the full schema.

```toml
default_branch = "main"            # what --changed diffs against
depth = "standard"

[roots]                            # monorepos; omit for a single root
engine = { path = "engine", lang = "rust" }
web = { path = "web", lang = "typescript" }

[scope]
exclude = ["generated/**", "*.pb.go"]   # gitignore-style; a pattern without "/" matches at any depth

[lang.rust]                        # tools for one language, chosen by init
test = "cargo test --workspace"
coverage = "cargo llvm-cov --lcov --output-path {out}"
runner = "cargo-nextest"           # the test runner Cupel uses; see Test runners

[checks."static.complexity"]       # check settings, for every language
cyclomatic_max = 10

[checks."static.complexity".lang.go]   # the same settings, overridden for one language
cyclomatic_max = 15

[checks."static.deps".layers]      # lower layers may not import higher ones
order = ["domain", "app", "infra"]
map = { domain = "src/domain/**", app = "src/app/**", infra = "src/infra/**" }

[gate.changed]                     # evaluated for --changed and --base scopes
"tests.weakening" = "none"
"static.complexity" = "none"
"static.complexity.cyclomatic_max" = "<= 10"
"static.deps.cycles" = "== 0"

[gate.tree]                        # evaluated for whole-tree and --paths scopes
"static.clones.pairs" = "<= 40"

[profile.loop]                     # cupel check --profile loop
depth = "quick"
```

**Check settings** live under `[checks."<id>"]`, and `[checks."<id>".lang.<lang>]` overrides any of them for one language. `static.complexity` takes `cyclomatic_max` (10), `cognitive_max` (15), `nesting_max` (4), `params_max` (5), and `length_max` (60). `static.clones` takes `threshold` (0.82), `min_lines` (4), and `min_nodes` (20). `static.crap` takes `high` (30) and `watch` (5). `static.deps` takes `distance_max` (0.7) and `layers`. `tests.quality` takes `mock_density_max` (50), `mocks_min` (2), `logic_max` (0), and `setup_statements_min` (3).

**Gate rules** map a key to a condition. The key is a check ID or `<check-id>.<metric>`, where the metric is one of that check's gate metrics in the table above. The condition is `none` (the check reported no unsuppressed `warn` or `fail` finding) or a comparison `>= > <= < ==` against a number; a number ending in `%` is divided by 100. A rule passes only when its check ran to completion and reported the metric; a rule on a check that did not run is `skipped`, and any rule that does not pass fails the gate. `[gate.changed]` applies to `--changed` and `--base` scopes and `[gate.tree]` to everything else. A `[gate.tree]` rule on `tests.weakening`, which needs a range, is rejected when the config loads.

Cupel writes only under `.cupel/` (run records, the SQLite fact store, `cache/` with coverage reports and per-test maps, scratch space) unless `--write` is given. An install you allow (`--install`, or yes to the question) runs your package manager, which for the JavaScript launchers, uv, and poetry adds a dev dependency to `package.json` or `pyproject.toml` and its lockfile. `.cupel/.gitignore` excludes everything there except `suppressions.toml`.

## Agents

An agent can learn Cupel from Cupel. `cupel describe --all` is one JSON document with every verb, flag, exit code, and schema. `cupel describe <check>` and `cupel describe finding <kind>` explain any check or finding the agent meets. Every verb accepts `--describe`, and `--dry-run` shows exactly what a command would do before it does anything.

The loop is the one in [The agent loop](#the-agent-loop): run the gate, take `next_actions[0]`, fix it, confirm with the finding's `repro`, and repeat until the gate exits 0. An agent should parse only stdout, branch on `exit_code` and `status`, and treat exit 2 as a mistake in its own invocation, whose envelope names the fix.

## Development

All automation goes through `make`.

| Target | Does |
|---|---|
| `make check` | fmt, clippy with `-D warnings`, every test, rustdoc with `-D warnings`, and a check that the build left the tree clean; the pre-commit bar |
| `make test` | `cargo test --workspace`; `UPDATE_GOLDENS=1 make test` rewrites golden envelopes |
| `make lint`, `make fmt` | `cargo fmt --check` and clippy; `cargo fmt` |
| `make build`, `make doc` | build the workspace; build its rustdoc |
| `make matrix` | print the check × language support matrix |
| `make readme` | regenerate this README's exit-code table, check table, check summaries, and support matrix from `cupel describe` |
| `make parity` | every golden-harness run again through `cupel serve` on a copy of its directory, which must print the same after redaction ([ADR-085](docs/adr/CUPEL_DECISION_085.md)) |
| `make dogfood` | Cupel grades itself: `cupel check --changed --gate` against `main`; `make dogfood DOGFOOD_SCOPE=tree` grades the whole tree with `[gate.tree]`. Both skip `tests.flaky` and `tests.mutation`, which would rerun Cupel's whole suite many times |
| `make install` | `cargo install --path crates/cupel-cli` |

The generated README sections sit between `<!-- generated:NAME -->` and `<!-- end:NAME -->` markers. A test in `crates/cupel-cli/tests/readme.rs` renders them from the binary's `describe` output and fails when the committed text differs, so `make check` catches a stale table. Run `make readme` and review the diff.

CI runs `make check` and `make parity` on Linux and macOS and `make dogfood` on every push: the tree gate on `main` and the changed gate on branches. Cupel's own `cupel.toml` is the example to copy.

The workspace is a set of crates under `crates/`. `cupel-core` holds the output contract, the fact vocabulary, scope, the scheduler, and gates. `cupel-lang` holds the `Language` trait, the query-driven producers, and one `langs/<id>/` directory per language. `cupel-adapt` is the toolchain table of runners and coverage and bench tools per language. `cupel-mutate` is the mutation engine. `cupel-checks` holds the checks, which depend on `cupel-core` alone. `cupel-store` is the SQLite fact store. `cupel-serve` is the daemon's socket protocol, server, client, and file watcher, generic over what it runs. `cupel-cli` is the binary, and the daemon runs requests through the same code a cold invocation does. `cupel-fixtures` holds fixture repositories, conformance inventories, and golden envelopes. Every behavior is built test-first; the implementation plan has the definition of done for each step.

Checks and tool producers register themselves from their own modules ([ADR-074](docs/adr/CUPEL_DECISION_074.md)), so additions made in parallel never edit the same lines. Adding a check is one module in `crates/cupel-checks/src/` that submits itself with `inventory::submit!`, its `mod` line in that crate's `lib.rs`, its fixture code, and its goldens under `crates/cupel-fixtures/golden/<check-id>/` (the matrix golden per language, `describe.json`, and `survey-<lang>.json`); a check can keep its fixture code in its own `crates/cupel-fixtures/repos/<check-id>/<lang>/` so no other check's goldens change. Adding a tool producer is one module in `crates/cupel-adapt/src/` and its `mod` line. Tests derive what must be described, golden, and supported from the registries, so nothing else lists checks; `UPDATE_GOLDENS=1 make test` writes a new check's goldens and `make readme` adds its rows here.

## License

BSD 3-Clause. See [LICENSE](LICENSE).
