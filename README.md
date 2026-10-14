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

`doctor` verifies the binary against `required_version`, the validity of `cupel.toml`, and write access to `.cupel/`. For every tool a configured check needs through its facts (a test runner, for a check that needs test results), it checks the tool is on `PATH` and reports its version under `run.toolchain`; a `[lang.<id>] runner` that names no runner is a finding listing the valid ones. Every problem is a finding with a `configure` or `install_tool` action. Exit 3 means something required is missing.

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

Every object has a fixed key set; an absent value is `null`, never a missing key. `status` is one of `pass`, `fail`, `partial`, `error`, or `plan`. `--format jsonl` writes the envelope header and then one finding per line. `cupel describe schema run` and `cupel describe schema finding` are the full schemas.

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
  python  per-test map from tool.pytest-cov-per-test
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

In JSON the same report is the envelope's `tests`: `per_test_map`, `impacted` (`id`, `file`, `reason`, `lang`), `commands` (`lang`, `runner`, `dir`, `command`, and `narrowed: false` when the runner cannot run only those tests and the command runs its whole suite), and `languages` (per language, whether a per-test map chose the tests or why not). Measuring a per-test map runs each test alone under coverage, so it costs a full test run or more; `cupel describe matrix` shows which languages can have one here.

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
| `--write` | Permit writes outside `.cupel/`; nothing else ever writes elsewhere |
| `--quiet`, `--verbose`, `--no-color` | Diagnostics on stderr only; stdout's shape never changes |

## Checks

Every check below exists, runs at `--depth quick`, and is supported in all five languages. The table is generated from `cupel describe` (see [Development](#development)). Tier is the strength of the evidence, from 0 (structural) upward. Gate metrics are the keys a gate rule can name as `<check>.<metric>`; ranked metrics are what `survey` ranks.

<!-- generated:check-table -->
| Check | Tier | Cost | Finding kinds | Gate metrics | Ranked by | Languages |
| --- | --- | --- | --- | --- | --- | --- |
| `static.clones` | 0 | quick | `clone.pair`, `clone.cluster` | `pairs`, `clusters`, `max_jaccard` | `cluster_size` | go, javascript, python, rust, typescript |
| `static.complexity` | 0 | quick | `complexity.high`, `nesting.deep`, `params.many`, `function.long` | `cyclomatic_max`, `cognitive_max`, `nesting_max`, `params_max`, `length_max` | `cyclomatic`, `cognitive` | go, javascript, python, rust, typescript |
| `static.crap` | 0 | quick | `crap.high`, `crap.watch` | `max`, `mean` | `crap` | go, javascript, python, rust, typescript |
| `static.dead-code` | 0 | quick | `dead.symbol`, `dead.export` | `dead_symbols`, `dead_exports` | — | go, javascript, python, rust, typescript |
| `static.deps` | 0 | quick | `dep.cycle`, `dep.layer-violation`, `dep.unstable-abstract` | `cycles`, `max_distance`, `violations` | `distance`, `coupling`, `cycle_size` | go, javascript, python, rust, typescript |
| `tests.quality` | 2 | quick | `test.weak-assertion`, `test.no-assertion`, `test.mock-heavy`, `test.logic`, `test.duplicate-setup` | `assertion_density`, `weak_ratio`, `no_assertion` | `mock_density` | go, javascript, python, rust, typescript |
| `tests.weakening` | 2 | quick | `assertion.removed`, `test.skipped`, `matcher.loosened`, `snapshot.rewritten`, `test.deleted` | `weakened` | — | go, javascript, python, rust, typescript |
<!-- end:check-table -->

What each check looks for:

- **`static.complexity`** measures every function's cyclomatic and cognitive complexity, nesting depth, parameter count, and length against configurable limits. A `match` or `switch` counts as one decision however many arms it has.
- **`static.crap`** scores each function by complexity weighted by missing test coverage. It needs line coverage, and Cupel does not produce coverage, so it passes with a null score and no findings rather than guess.
- **`static.clones`** finds functions whose normalized syntax trees are mostly the same, so renamed variables and reformatting do not hide a copy. Two members are a `clone.pair`; three or more are a `clone.cluster`.
- **`static.deps`** builds the module graph from resolved imports. It reports import cycles, modules that are both unstable and abstract, and imports that break the layering you configure.
- **`static.dead-code`** reports symbols and exports in modules that no import reaches and no entry point, test, or framework hook keeps alive. Reachability is decided per module, not per call, so an export of a module nothing imports is reported as `info` with low confidence: it may be public API.
- **`tests.quality`** reads every test: tests with no assertions or only weak ones (truthiness, presence, inequality), heavy mocking, loops and branches inside tests, and duplicated setup.
- **`tests.weakening`** compares the tests at the range's base with the tests now. It reports removed or loosened assertions, skipped or deleted tests, and rewritten snapshots. It needs `--changed` or `--base`.

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

A runner is used when its program is on `PATH` (or, for `vitest` and `jest`, in the root's `node_modules/.bin`); otherwise checks that need test results are skipped as `unavailable` with the tool named, and `cupel doctor` says what to install. `[lang.<id>] runner` chooses one explicitly, by its name with or without the `tool.` prefix:

```toml
[lang.python]
runner = "unittest"
```

Before any check that needs test results runs, Cupel runs the suite once in the real tree, the **baseline**. If any test fails, or the runner cannot finish (a build error), the run is refused: `status: "error"`, exit code 4, no check scores, and one `baseline.test-failed` finding per failing test (with its file, runner, and the tail of its output) or `baseline.run-incomplete` per run that did not finish. Fix the suite, then rerun. Every test of the baseline is also indexed in `.cupel/cupel.db`. The checks shipped today read only source facts, so `cupel check` runs no tests yet; the mutation and flakiness checks will.

## Configuration

`cupel.toml` is found by walking up from `--cwd`. Precedence is flags, then `CUPEL_*` environment variables (`CUPEL_DEPTH`, `CUPEL_FORMAT`, `CUPEL_PROFILE`), then `--profile`, then `cupel.toml`, then defaults. The envelope echoes the effective configuration and the layers that contributed under `run.config`. Unknown keys are usage errors (exit 2) that list the valid keys, and `cupel describe schema config` is the full schema.

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

Cupel writes only under `.cupel/` (run records, the SQLite fact store, scratch space) unless `--write` is given. `.cupel/.gitignore` excludes everything there except `suppressions.toml`.

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
| `make dogfood` | Cupel grades itself: `cupel check --changed --gate` against `main`; `make dogfood DOGFOOD_SCOPE=tree` grades the whole tree with `[gate.tree]` |
| `make install` | `cargo install --path crates/cupel-cli` |

The generated README sections sit between `<!-- generated:NAME -->` and `<!-- end:NAME -->` markers. A test in `crates/cupel-cli/tests/readme.rs` renders them from the binary's `describe` output and fails when the committed text differs, so `make check` catches a stale table. Run `make readme` and review the diff.

CI runs `make check` on Linux and macOS and `make dogfood` on every push: the tree gate on `main` and the changed gate on branches. Cupel's own `cupel.toml` is the example to copy.

The workspace is a set of crates under `crates/`. `cupel-core` holds the output contract, the fact vocabulary, scope, the scheduler, and gates. `cupel-lang` holds the `Language` trait, the query-driven producers, and one `langs/<id>/` directory per language. `cupel-adapt` is the toolchain table of runners and coverage and bench tools per language. `cupel-checks` holds the checks, which depend on `cupel-core` alone. `cupel-store` is the SQLite fact store. `cupel-cli` is the binary. `cupel-fixtures` holds fixture repositories, conformance inventories, and golden envelopes. Every behavior is built test-first; the implementation plan has the definition of done for each step.

Checks and tool producers register themselves from their own modules ([ADR-074](docs/adr/CUPEL_DECISION_074.md)), so additions made in parallel never edit the same lines. Adding a check is one module in `crates/cupel-checks/src/` that submits itself with `inventory::submit!`, its `mod` line in that crate's `lib.rs`, its fixture code, and its goldens under `crates/cupel-fixtures/golden/<check-id>/` (the matrix golden per language, `describe.json`, and `survey-<lang>.json`); a check can keep its fixture code in its own `crates/cupel-fixtures/repos/<check-id>/<lang>/` so no other check's goldens change. Adding a tool producer is one module in `crates/cupel-adapt/src/` and its `mod` line. Tests derive what must be described, golden, and supported from the registries, so nothing else lists checks; `UPDATE_GOLDENS=1 make test` writes a new check's goldens and `make readme` adds its rows here.

## License

BSD 3-Clause. See [LICENSE](LICENSE).
