# Cupel

Cupel grades the code a coding agent produced: it measures structure and test strength across a repository or a change, and returns a ranked list of what to fix next. One binary, one JSON envelope from every command, and a gate that tells an agent when it is allowed to stop.

A *cupel* is the porous cup used in fire assay to separate precious metal from base.

```sh
cupel init --write && cupel check --changed
```

That detects your project, writes `cupel.toml`, and grades everything that differs from your default branch.

## What Cupel does

Cupel runs seven static checks over JavaScript, TypeScript, Rust, Go, Python, Java, and Clojure. They measure complexity, CRAP, clones, dependency structure, dead code, test quality, and tests weakened on a branch. Every check works in every language, because checks read language-neutral facts and never see source syntax.

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

### From a release archive

Each [GitHub Release](https://github.com/tysont/cupel/releases) carries one archive per platform, `cupel-<version>-<target>.tar.gz`, for `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` (static binaries), `aarch64-apple-darwin`, and `x86_64-apple-darwin`, and a `SHA256SUMS` file. Download your platform's archive and `SHA256SUMS`, verify the checksum, and put `cupel` on your PATH:

```sh
sha256sum --ignore-missing -c SHA256SUMS     # on macOS: shasum -a 256 --ignore-missing -c SHA256SUMS
tar -xzf cupel-0.1.0-x86_64-unknown-linux-musl.tar.gz
install cupel-0.1.0-x86_64-unknown-linux-musl/cupel ~/.local/bin/
```

On Windows, run the Linux archive under WSL.

### From source

Build from source with a Rust toolchain. The pinned version is in `rust-toolchain.toml`.

```sh
git clone https://github.com/tysont/cupel
cd cupel
make install            # cargo install --path crates/cupel-cli
```

The binary has no runtime dependencies except `git`, which Cupel calls for `--changed` and `--base` scopes.

Every other route installs a release binary from GitHub Releases, checked against the release's `SHA256SUMS`; `<target>` is one of the targets above. The installers are in `dist/` and are tested in CI against a fake release. The table is the binary's own: `cupel describe` lists the same routes as `install_routes`, and `doctor`'s `doctor.version-unsatisfied` finding names the ones available now.

<!-- generated:install-routes -->
| Route | Command | Platforms | Available |
| --- | --- | --- | --- |
| Install script | `curl -fsSL https://raw.githubusercontent.com/tysont/cupel/main/dist/install.sh \| sh` | Linux, macOS | now |
| cargo-binstall | `cargo binstall --git https://github.com/tysont/cupel cupel-cli` | Linux, macOS | now |
| Release archive | `gh release download --repo tysont/cupel --pattern SHA256SUMS --pattern 'cupel-*-<target>.tar.gz'` | Linux, macOS | now |
| From source | `cargo install --locked --git https://github.com/tysont/cupel cupel-cli` | Linux, macOS | now |
| Homebrew | `brew install tysont/cupel/cupel` | Linux, macOS | once the release workflow pushes the formula to the `tysont/homebrew-cupel` tap |
| npm | `npx cupel` | Linux, macOS | once the `cupel` package is published to npm |
| pipx | `pipx install cupel` | Linux, macOS | once Cupel's package is on PyPI; the `cupel` there is another project's |
| crates.io | `cargo install --locked cupel-cli` | Linux, macOS | once the crates are published to crates.io |
| cupel.dev | `curl -fsSL https://cupel.dev/install \| sh` | Linux, macOS | once cupel.dev serves the install script |
<!-- end:install-routes -->

- **Install script** (POSIX `sh`): installs Linux x86_64/aarch64 and macOS arm64/x86_64 binaries into `~/.local/bin`, or `CUPEL_INSTALL_DIR`, and prints the line to add to your `PATH` when that directory is not on it. `CUPEL_VERSION=0.1.0` installs that release instead of the latest. It needs `curl` or `wget`, `tar`, and `sha256sum` or `shasum`.
- **npm and pipx**: the package's `cupel` command downloads the binary of the package's own version on first run into `CUPEL_CACHE_DIR`, else `~/.cache/cupel` (`%LOCALAPPDATA%\cupel` on Windows), then runs it with your arguments, stdio, and exit code. When the binary cannot be downloaded or its checksum differs, it prints why and exits 3. npm needs Node 18 or newer; pipx, Python 3.9 or newer.
- **Homebrew**: the formula, rendered per release by `dist/homebrew/render.sh` from `SHA256SUMS`, covers macOS and Linux.

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

### `cupel init`

`init` detects project roots and languages and chooses test, coverage, and bench commands for each. It understands Cargo crates and workspaces, Go modules and workspaces, npm, pnpm, and yarn packages and workspaces (TypeScript when `tsconfig.json` is present), and Python projects (`pyproject.toml`, `setup.cfg`, `requirements.txt`).

```sh
cupel init                   # preview: the envelope carries unified diffs of what would be written
cupel init --write           # write cupel.toml and .cupel/.gitignore
cupel init --write --force   # replace an existing cupel.toml
```

### `cupel doctor`

`doctor` verifies the binary against `required_version` (see [Configuration](#configuration); the other verbs refuse to run outside it), the validity of `cupel.toml`, and write access to `.cupel/`. For every tool a configured check needs through its facts (a test runner, for a check that needs test results; a coverage tool, for a language with a `coverage` command), it checks the tool is on `PATH` and reports its version under `run.toolchain`; a `[lang.<id>] runner` that names no runner is a finding listing the valid ones. Every problem is a finding with a `configure` or `install_tool` action. Exit 3 means something required is missing.

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

Each schema's `$id` is the URL it is published at, under [`https://tysont.github.io/cupel/schema/v1/`](https://tysont.github.io/cupel/): `run.json`, `finding.json`, `events.json`, `config.json`, `plugin.json`, and `fact/<kind>.json` for each fact kind. The `pages` workflow publishes them from `main` with `scripts/schemas.sh` ([ADR-127](docs/adr/CUPEL_DECISION_127.md)).

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

Under `--changed` or `--base` the symbols in scope are the functions and methods the range's changed lines touch (a file git does not track yet counts whole): a clone group is reported when one of its members was touched, every other per-symbol finding when its symbol was, so the changed gate rules on what the change wrote, and the tree gate grades the rest ([ADR-142](docs/adr/CUPEL_DECISION_142.md)). Facts are still produced for whole files, so a touched function's clone elsewhere in its file is found. A `--changed` scope with no changes passes with `scope.symbols: 0`. `tests.weakening` compares tests between two revisions, so it runs only with `--changed` or `--base` and is reported as skipped on a tree.

**Selection** decides which checks run.

| Flag | Selects |
|---|---|
| `--depth quick\|standard\|deep\|exhaustive` | How much evidence to gather; default `standard`, which adds the checks that run the test suite (`gen.property` and `conc.race` among them) to the `quick` ones; `deep` adds `gen.differential`, `gen.metamorphic`, `gen.fuzz`, `perf.bench`, and `conc.linearizability`; `exhaustive` adds `conc.dst`, `spec.trace`, `conc.model-check`, and `spec.prove` |
| `--only ID`, `--skip ID` | Checks by ID (`static.clones`) or group (`static`, `tests`); repeatable |
| `--tier N+` | Only checks at or above evidence tier N, e.g. `--tier 2+` |
| `--budget DURATION` | A wall-clock budget (`250ms`, `20s`, `10m`): run the most valuable selected checks that fit and skip the rest; `CUPEL_BUDGET` or `budget` in `cupel.toml` also set it |
| `--jobs N` | How many threads and mutation workers run at once; default the cores (and, for mutation, the language's `mutation.jobs`) |
| `--oracle NAME` | The oracle checks that compare against one use; default the first verified (see [`cupel oracle`](#cupel-oracle)) |
| `--cases N` | The case budget of generative checks: how many inputs `gen.differential` and `gen.metamorphic` generate per entry point or relation (default 100), and how many generated cases each property test runs and how many inputs each fuzz target runs on (see [Property tests](#property-tests) and [Fuzzing](#fuzzing); default 1 000 at `standard`, ten times more at each deeper depth) |
| `--scaffold` | Also write a property-test stub for each `property.missing` finding; without `--write` the envelope's `plan.would_write` shows each file's diff and nothing is written |

**Budget.** `--budget` chooses among the checks the depth and the selection flags already chose; it never adds deeper ones. Each check costs its own run plus the facts it needs that nothing has made yet: the baseline test run, a coverage report, the mutation engine. A fact two checks share is paid once. A cost is the median of the last ten recorded runs of the same kind of work on this repository (the same check, language, and order of magnitude of targets; the same producer and language, run or read from cache), once three are recorded in `.cupel/cupel.db`; before that it comes from the cost class (a `quick` producer 100 ms, `standard` 30 s, `deep` 5 min). A check's value is its evidence tier plus one, doubled when it has never produced a result here, halved when its last result was made at this commit on a clean tree, and doubled again inside a `--changed` or `--base` scope. Checks are taken greedily by value per millisecond while they fit; ties are broken by `--seed`. The budget counts from the start of the invocation, as `--timeout` does: a check is not started when its estimate exceeds what is left, and work still running at the deadline is stopped (its processes are killed), so a run overshoots its budget only by the moment that takes; a mutation run stopped that way reports `partial` with no score, and the mutants it finished are not kept. Anything the budget left out is `skipped` with `reason: budget` and `est_ms`, its estimate, the run is `partial` (exit 5), and `next_actions` offers the command that runs it alone. On a small Python project with a test suite and no recorded runs, one second leaves out everything that needs the suite:

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
  "cupel": { "version": "0.1.0", "schema": "https://tysont.github.io/cupel/schema/v1/run.json" },
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

`--format sarif` writes the envelope as a [SARIF 2.1.0](https://docs.oasis-open.org/sarif/sarif/v2.1.0/) log for code-scanning tools and editors, from every verb. Each finding is a result: its kind is the `ruleId`, its severity the `level` (`fail` is `error`, `warn` is `warning`, `info` is `note`), its location a region relative to the `SRCROOT` base (the repository root), and its ID `partialFingerprints["cupel/findingId"]`; a suppressed finding carries an `accepted` suppression, and `fixes` is always empty. The rules are the finding kinds of the checks that ran, described as `cupel describe finding` describes them. The commit is in `versionControlProvenance` and the exit code in the invocation. What SARIF has no place for is kept in `properties`: a result's holds the finding's check, tier, confidence, evidence, action, and repro; the invocation's holds the envelope's `run`; the run's holds the rest of the envelope (checks, summary and gate, `next_actions`, and each verb's own payload).

### `cupel survey`

`survey` is the first command to run on an unfamiliar repository. It takes the same scope and selection flags as `check`, never gates, and adds `rankings`: the ten worst targets for every per-symbol, per-module, and per-group metric (`worst` says which end ranks first: `lowest` for the protection metrics `tests.mutation.mutation_score`, `tests.mutation.tests`, and `tests.coverage.line_coverage`, `highest` for every other), and `targets`, protect-first.

`targets` are the symbols a finding asks to protect (`write_test`, `strengthen_assertion`) or to simplify (`reduce_complexity`, `refactor_extract`), each with its `recommend` (`protect`, `simplify`, or `both`), its `protection` (`mutation_score`, `coverage`, and `tests`; null where nothing measured them), its value and rank in every dimension, and the findings that recommend it. Refactoring code no test protects is unsafe, so targets to protect come first, the weakest protection (the mutation score, else coverage) first among them; equal protection falls back to the target's best rank in any other dimension. `recommended` counts them all; `targets` lists the first ten. `survey`'s `next_actions` follow the same order: an action to protect each target, then one to simplify each, then the rest. Protection is measured from `--depth standard`, where `tests.mutation` and `tests.coverage` run; at `--depth quick` it is null and only the findings say what to do. Suppressed findings recommend nothing.

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

### `cupel explain`

`explain` reads the fact store and never re-runs anything. Given a finding ID it returns the stored finding with a source excerpt, its full evidence, and its remediation hint in full: for `clone.cluster` the largest subtree every member shares (its place in each member and the names and literals that differ, the extracted function's parameters), for `complexity.high` the most deeply nested block (its place, the constructs on the path to it, and the complexity it carries), and for `dep.cycle` the cheapest import to cut (the names it carries, and whether removing it breaks every cycle among the modules). The checks compute these hints when they report the finding, so `check` and `survey` carry them in `action.hints` too. Given a check ID it returns that check's result, findings, and metrics from the last run that included it.

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

A finding's ID is derived from its function's structure, so renaming or moving a function keeps it, but editing the function changes it. Under `--changed` or `--base`, the functions the range removed are matched to the ones it added by how much of their structure they share: a rename with an edit, a move, a function split in two, or two merged into one. A finding on the old function with a finding of the same check and kind on any of its successors is neither new nor resolved. Each check's `delta` lists these matches as `sets`: the functions on each side, how similar they are, the findings carried across, and each per-function metric over each side (the sum of a count such as `cyclomatic`, the size-weighted mean of a score such as `crap`). `tests.weakening` matches a renamed or split test the same way instead of reporting it deleted ([ADR-131](docs/adr/CUPEL_DECISION_131.md)).

### `cupel oracle`

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

summary   1 finding · fail 1 · warn 0 · info 0 · new 0 · resolved 0 · suppressed 0

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

A check that compares against an oracle names it in its result's `oracle` member. It uses `--oracle NAME` on `check` or `survey`, else the first verified oracle, else the first defined one; with no oracle defined, the check is skipped as `unconfigured`. When the run is graded, a result against an oracle that is not verified at that moment is capped at tier 3 and the run carries a `warn` finding, `oracle.unverified` (`cupel describe finding oracle.unverified`), whose action is `verify_oracle`. Only `cupel oracle verify` lifts the cap; suppressing the finding does not. `gen.differential` is the built-in check that compares against an oracle (see [Differential testing](#differential-testing)); it compares against `ref` and `command` oracles and skips itself, as `unconfigured`, when the run's oracle is a spec. `spec.trace` compares against `spec` oracles (see [Trace validation](#trace-validation)): name one with `--oracle` when the repository defines oracles of both kinds.

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

- **When the CLI uses it.** Every invocation whose repository root (`--cwd`, else the working directory) has a `.cupel/serve.sock`. It runs in its own process instead when `CUPEL_NO_DAEMON=1` is set, for `serve` itself, when an argument is not UTF-8, when the run could ask a question on the terminal (stdout is a terminal and neither `--install` nor `--no-install` is given), when `--jobs` is given (a process sizes its thread pool once), and when nothing answers on the socket (a daemon that was killed leaves its socket file behind; it is ignored).
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

### MCP

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

### Global flags

Every verb takes these. A flag's variable sets it too, below the flag and above `cupel.toml` (see [Configuration](#configuration)); `--no-path-plugins` is described under [Plugins](#plugins).

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

## Checks

Every check below exists. Each is supported in all seven languages except `gen.property`, which Go, Java, and Clojure lack (Cupel runs no property framework for them); `gen.metamorphic`, which has a Python harness only; `conc.race`, which has race detectors for Go and Rust only; `gen.fuzz`, which has no TypeScript, Java, or Clojure fuzzer; `perf.bench`, which has no Java or Clojure bench tool; `conc.dst`, which has a simulator for Rust only (madsim); `conc.model-check`, which has a model checker for Rust only; `spec.contracts`, which reads Rust's `contracts` crate and Python's icontract only; and `gen.differential`, `static.analyze`, and `perf.footprint`, which have no Java or Clojure harness, analyzer, or lockfile reader yet; the [support matrix](#languages) says `unsupported`. All run at `--depth quick` except `gen.property`, `tests.coverage`, `tests.flaky`, `tests.mutation`, `conc.race`, and `spec.contracts`, which run the project's test suite, `meta.traceability`, and `static.analyze`, which runs its type checkers and linters, so they run from `--depth standard`, the default; and `gen.differential`, `gen.metamorphic`, `gen.fuzz`, and `perf.bench`, which run from `--depth deep` (or by name with `--only`), the first two once a language lists the functions it compares or declares relations (see [Differential testing](#differential-testing) and [Metamorphic relations](#metamorphic-relations)); `conc.linearizability` also runs from `--depth deep`, once a language names the command that records its histories; and `conc.dst`, `spec.trace`, `conc.model-check`, and `spec.prove` run from `--depth exhaustive` (or by name with `--only`), `spec.trace` once a spec oracle is defined (see [Trace validation](#trace-validation)). The table is generated from `cupel describe` (see [Development](#development)). Tier is the strength of the evidence, from 0 (structural) upward. Gate metrics are the keys a gate rule can name as `<check>.<metric>`; ranked metrics are what `survey` ranks.

<!-- generated:check-table -->
| Check | Tier | Cost | Finding kinds | Gate metrics | Ranked by | Languages |
| --- | --- | --- | --- | --- | --- | --- |
| `conc.dst` | 4 | exhaustive | `dst.failure` | `harnesses`, `failures` | — | rust |
| `conc.linearizability` | 5 | deep | `consistency.violation` | `histories`, `operations`, `violations`, `unknown`, `invalid` | — |  |
| `conc.model-check` | 5 | exhaustive | `model.counterexample` | `harnesses`, `verified`, `failed`, `undetermined` | — | rust |
| `conc.race` | 4 | standard | `race.detected`, `ub.detected` | `races`, `undefined_behavior` | — | go, rust |
| `gen.differential` | 4 | deep | `diff.divergence` | `divergences`, `cases` | — |  |
| `gen.fuzz` | 4 | deep | `fuzz.crash`, `fuzz.timeout`, `fuzz.target-missing` | `targets`, `crashes`, `timeouts`, `missing` | — | go, javascript, python, rust |
| `gen.metamorphic` | 4 | deep | `metamorphic.violated` | `violated`, `relations` | — |  |
| `gen.property` | 4 | standard | `property.falsified`, `property.missing` | `properties`, `falsified`, `missing` | — | javascript, python, rust, typescript |
| `meta.oracle-independence` | 2 | quick | `evidence.self-authored-only` | `independent`, `self_authored_only` | — | clojure, go, java, javascript, python, rust, typescript |
| `meta.traceability` | 2 | standard | `spec.untested-requirement`, `spec.unlisted-requirement` | `untested` | — | clojure, go, java, javascript, python, rust, typescript |
| `perf.bench` | 0 | deep | `perf.regression`, `perf.improvement` | `benchmarks`, `compared`, `regression`, `regressed`, `improved` | — | go, python, rust |
| `perf.footprint` | 0 | quick | `footprint.grew`, `dep.vuln`, `dep.license` | `dependencies`, `vulnerable`, `copyleft`, `artifact_bytes` | — | go, javascript, python, rust, typescript |
| `spec.contracts` | 5 | standard | `contract.violated` | `violations` | — | python, rust |
| `spec.prove` | 6 | exhaustive | `proof.failed`, `proof.obligation-open` | `verified`, `failed`, `open` | — | clojure, go, java, javascript, python, rust, typescript |
| `spec.trace` | 5 | exhaustive | `trace.diverged` | `traces`, `diverged`, `unvalidated` | — | clojure, go, java, javascript, python, rust, typescript |
| `static.analyze` | 1 | standard | `analyze.<tool>.<rule>` | `errors`, `diagnostics` | — | go, javascript, python, rust, typescript |
| `static.api-surface` | 0 | quick | `api.breaking`, `api.widened` | `breaking`, `widened` | — | clojure, go, java, javascript, python, rust, typescript |
| `static.clones` | 0 | quick | `clone.pair`, `clone.cluster` | `pairs`, `clusters`, `max_jaccard` | `cluster_size` | clojure, go, java, javascript, python, rust, typescript |
| `static.complexity` | 0 | quick | `complexity.high`, `nesting.deep`, `params.many`, `function.long` | `cyclomatic_max`, `cognitive_max`, `nesting_max`, `params_max`, `length_max` | `cyclomatic`, `cognitive` | clojure, go, java, javascript, python, rust, typescript |
| `static.crap` | 0 | quick | `crap.high`, `crap.watch` | `max`, `mean` | `crap` | clojure, go, java, javascript, python, rust, typescript |
| `static.dead-code` | 0 | quick | `dead.symbol`, `dead.export` | `dead_symbols`, `dead_exports` | — | clojure, go, java, javascript, python, rust, typescript |
| `static.deps` | 0 | quick | `dep.cycle`, `dep.layer-violation`, `dep.unstable-abstract` | `cycles`, `max_distance`, `violations` | `distance`, `coupling`, `cycle_size` | clojure, go, java, javascript, python, rust, typescript |
| `tests.assertion-density` | 2 | quick | `density.sparse-test`, `density.sparse-file` | `assertions_per_test`, `assertions_per_public_function`, `tested_public_functions`, `sparse_tests`, `sparse_files` | — | clojure, go, java, javascript, python, rust, typescript |
| `tests.coverage` | 2 | standard | `coverage.uncovered-symbol`, `coverage.uncovered-branch` | `line`, `branch`, `lines`, `branches`, `uncovered` | `uncovered_lines`, `line_coverage` |  |
| `tests.flaky` | 2 | standard | `test.flaky`, `test.order-dependent` | `flaky`, `order_dependent` | — | clojure, go, java, javascript, python, rust, typescript |
| `tests.mutation` | 3 | standard | `mutant.survived`, `mutant.timeout`, `mutant.uncovered`, `mutant.equivalent?` | `score`, `killed`, `survived`, `timeout`, `uncovered`, `unviable`, `equivalent`, `executed`, `reused`, `schemata`, `fallback` | `survivors`, `mutation_score`, `tests` |  |
| `tests.quality` | 2 | quick | `test.weak-assertion`, `test.no-assertion`, `test.mock-heavy`, `test.logic`, `test.duplicate-setup` | `assertion_density`, `weak_ratio`, `no_assertion` | `mock_density` | clojure, go, java, javascript, python, rust, typescript |
| `tests.weakening` | 2 | quick | `assertion.removed`, `test.skipped`, `matcher.loosened`, `snapshot.rewritten`, `test.deleted` | `weakened` | — | clojure, go, java, javascript, python, rust, typescript |
<!-- end:check-table -->

What each check looks for:

- **`static.complexity`** measures every function's cyclomatic and cognitive complexity, nesting depth, parameter count, and length against configurable limits. A `match` or `switch` counts as one decision however many arms it has.
- **`static.crap`** scores each function by complexity weighted by missing test coverage, from the language's coverage report (see [Coverage](#coverage)); without one it passes with a null score and no findings rather than guess.
- **`static.clones`** finds functions whose normalized syntax trees are mostly the same, so renamed variables and reformatting do not hide a copy. Two members are a `clone.pair`; three or more are a `clone.cluster`. Each says its `clone_type`: 1 for the same tokens (only layout, comments, or the function's name differ), 2 for the same tree with identifiers or literals renamed (listed in `renamed`, one text per member), 3 for statements inserted, removed, or changed (listed in `edits`, with where each is).
- **`static.deps`** builds the module graph from resolved imports. It reports import cycles, modules that are both unstable and abstract, and imports that break the layering you configure.
- **`static.dead-code`** reports symbols and exports in modules that no import reaches and no entry point, test, or framework hook keeps alive. Reachability is decided per module, not per call, so an export of a module nothing imports is reported as `info` with low confidence: it may be public API.
- **`tests.quality`** reads every test: tests with no assertions or only weak ones (truthiness, presence, inequality), heavy mocking, loops and branches inside tests, and duplicated setup.
- **`static.api-surface`** compares the public API at the range's base with the API now, path by path. A public item that is gone (deleted, renamed, or made private) or lost a signature is `api.breaking` (`warn`); one that is new or gained a signature (an overload) is `api.widened` (`info`). It needs `--changed` or `--base`, and the gate rule `"static.api-surface" = "no-breaking"` holds a branch to no unsuppressed break.
- **`static.analyze`** runs each language's type checker at its strictest and its linters, and reports every result in a file in scope as `analyze.<tool>.<rule>` (`analyze.mypy.arg-type`, `analyze.clippy.needless_return`): errors `fail`, warnings `warn`, notes `info`. See [Analyzers](#analyzers).
- **`perf.footprint`** reads the dependencies the lockfiles list, without network access, and reports those with a known vulnerability (`dep.vuln`) or a copyleft license (`dep.license`); under a range it reports only dependencies the range added or changed, and `footprint.grew` when there are more than at the base. See [Dependencies and footprint](#dependencies-and-footprint).
- **`tests.assertion-density`** measures assertions per test and per tested public function (one whose file has tests). It reports long tests that assert little for their length (`density.sparse-test`) and test files whose assertions per test fall far below the repository's (`density.sparse-file`); a test that asserts nothing is `tests.quality`'s.
- **`tests.weakening`** compares the tests at the range's base with the tests now. It reports removed or loosened assertions, skipped or deleted tests, and rewritten snapshots. It needs `--changed` or `--base`.
- **`tests.flaky`** reruns the test suite several times in different orders and with different seeds, and reports tests whose outcome changes: `test.flaky` when two reruns in the same order disagree, `test.order-dependent` when the outcome follows the order. See [Flakiness](#flakiness).
- **`gen.differential`** calls the functions you name on generated inputs, in the working tree and against an oracle (the same code at an earlier commit, or a command that computes the reference answer), and reports the smallest input on which the two answer differently, with both answers. See [Differential testing](#differential-testing).
- **`gen.property`** runs the project's existing property tests (proptest and quickcheck, Hypothesis, fast-check) with the run's seed and many more generated cases than the suite usually gives them. A property that fails is `property.falsified`, with the framework's shrunk counterexample; a public function that takes arguments, in a file whose tests hold no property test, is `property.missing` (`info`). See [Property tests](#property-tests).
- **`tests.mutation`** changes the code in small ways (`<` to `<=`, `+` to `-`, `&&` to `||`, `true` to `false`, `0` to `1`) and runs the tests that cover each change. A mutant every test passes against survived: the tests do not notice that line changing. The score is killed / (killed + survived). See [Mutation testing](#mutation-testing).

- **`gen.metamorphic`** evaluates the metamorphic relations you declare (sorting a list and sorting its reverse give the same result; `add(a, b) == add(b, a)`) on generated inputs, and reports each relation that does not hold, with the smallest failing input it could find and both outputs. See [Metamorphic relations](#metamorphic-relations).
- **`gen.fuzz`** runs every fuzz target the project has (cargo-fuzz targets, Go `FuzzXxx` functions, Jazzer.js and atheris modules) with the run's seed and an execution budget, and reports each input that crashes a target (`fuzz.crash`) or runs past the per-input limit (`fuzz.timeout`), with the saved input and the command that reruns it; a project root with no fuzz target is `fuzz.target-missing` (`info`). See [Fuzzing](#fuzzing).
- **`conc.race`** runs the tests under a race detector (Go's `-race`; Miri or ThreadSanitizer for Rust) and reports each data race (`race.detected`) and each operation with undefined behavior (`ub.detected`), with the stacks of the accesses inside the repository. See [Race detectors](#race-detectors).
- **`conc.linearizability`** runs the command you name to record histories of concurrent operations (a test or harness writing JSON lines), checks each against the model it declares (a register, a key-value store) with Porcupine, and reports each history, or key, that no order consistent with real time explains (`consistency.violation`), with the operations that cannot come next. See [Linearizability](#linearizability).
- **`conc.dst`** runs the project's deterministic simulation harnesses (madsim's `#[madsim::test]`) over a range of seeds from the run's seed, and reports each harness's first failing seed (`dst.failure`) with what it printed, replayed alone, and the command that replays it. See [Deterministic simulation](#deterministic-simulation).
- **`perf.bench`** runs the project's benchmarks (Go's, criterion, pytest-benchmark, or commands timed by hyperfine) at the range's base commit and in the working tree, and compares each benchmark's samples with a Mann-Whitney U test: a significant slowdown of at least 5% is `perf.regression` (`warn`), a speedup `perf.improvement` (`info`). Without a range there is nothing to compare, and it says so. See [Benchmarks](#benchmarks).
- **`spec.trace`** runs the tests with `CUPEL_TRACE_DIR` set, so the implementation logs each step it takes (its state, and the spec action it took) as JSON lines, and validates every recorded trace against the run's TLA+ spec oracle with TLC. A trace with a step the spec does not allow is `trace.diverged`, at the step, with the state logged there and the last state the spec allows. See [Trace validation](#trace-validation).
- **`conc.model-check`** model-checks the project's proof harnesses (Kani's `#[kani::proof]` functions for Rust) over every input within their bounds, and reports each property an input breaks (an assertion, an overflow, an out-of-bounds access) as `model.counterexample`, with the values that break it and a test that replays them. See [Model checking](#model-checking).
- **`spec.prove`** verifies the project's Dafny proofs, and reports each proof obligation the verifier could not prove (`proof.failed`) and each the proofs assume instead of proving or leave undecided in the verifier's budget (`proof.obligation-open`, `warn`). See [Proofs](#proofs).
- **`spec.contracts`** runs the tests again with every contract the project's contract library checks turned on, the debug-only and slow ones too, and its property tests at the case budget, and reports each precondition, postcondition, and invariant a test violated (`contract.violated`), at the contract. See [Contracts](#contracts).
- **`meta.traceability`** maps each requirement ID you list to the tests the range added whose names or doc comments carry it (`test_req_12_parses`, `parses week dates #REQ-12`, `/// Parses week dates (REQ-12).`), and reports each requirement no such test names (`spec.untested-requirement`); a `REQ-<n>` marker on an added test that names no listed requirement is listed as one (`spec.unlisted-requirement`, `info`). It needs `--changed` or `--base`. See [Requirements and test provenance](#requirements-and-test-provenance).
- **`meta.oracle-independence`** labels each changed function the tests exercise by whether a test that existed before the change runs it, and reports one only the change's own tests run (`evidence.self-authored-only`, `warn`): the only evidence it works was written with it. It needs `--changed` or `--base` and a per-test coverage map. See [Requirements and test provenance](#requirements-and-test-provenance).

`cupel describe <check>` and `cupel describe finding <kind>` give the full contract, including each finding kind's evidence schema.

## Languages

Cupel understands **JavaScript, TypeScript, Rust, Go, Python, Java, and Clojure**. Each language is a tree-sitter grammar plus a declarative spec (queries and tables), and produces the same language-neutral facts: `symbols`, `decisions`, `norm-tree` (the structural hash that gives every function an identity surviving rename, move, and reformat), `mutation-sites`, `test-frames`, `test-conventions`, `module-graph` (imports resolved to files, with Node, `tsconfig` `paths`, Go module, Rust `mod`, Python package, Java source-root, and Clojure classpath resolution), `api-surface`, `roots`, `references` (every place that may name a function, resolved within its file or through its imports, and where names are computed at run time), and `project-info`. Checks consume facts and never see a language, so every check works in every language that produces what it needs.

The support matrix, from `cupel describe matrix`:

<!-- generated:support-matrix -->
| Check | clojure | go | java | javascript | python | rust | typescript |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `conc.dst` | unsupported | unsupported | unsupported | unsupported | unsupported | supported | unsupported |
| `conc.linearizability` | unconfigured | unconfigured | unconfigured | unconfigured | unconfigured | unconfigured | unconfigured |
| `conc.model-check` | unsupported | unsupported | unsupported | unsupported | unsupported | supported | unsupported |
| `conc.race` | unsupported | supported | unsupported | unsupported | unsupported | supported | unsupported |
| `gen.differential` | unsupported | unconfigured | unsupported | unconfigured | unconfigured | unconfigured | unconfigured |
| `gen.fuzz` | unsupported | supported | unsupported | supported | supported | supported | unsupported |
| `gen.metamorphic` | unsupported | unsupported | unsupported | unsupported | unconfigured | unsupported | unsupported |
| `gen.property` | unsupported | unsupported | unsupported | supported | supported | supported | supported |
| `meta.oracle-independence` | supported | supported | supported | supported | supported | supported | supported |
| `meta.traceability` | supported | supported | supported | supported | supported | supported | supported |
| `perf.bench` | unsupported | supported | unsupported | unconfigured | supported | supported | unconfigured |
| `perf.footprint` | unsupported | supported | unsupported | supported | supported | supported | supported |
| `spec.contracts` | unsupported | unsupported | unsupported | unsupported | supported | supported | unsupported |
| `spec.prove` | supported | supported | supported | supported | supported | supported | supported |
| `spec.trace` | supported | supported | supported | supported | supported | supported | supported |
| `static.analyze` | unsupported | supported | unsupported | supported | supported | supported | supported |
| `static.api-surface` | supported | supported | supported | supported | supported | supported | supported |
| `static.clones` | supported | supported | supported | supported | supported | supported | supported |
| `static.complexity` | supported | supported | supported | supported | supported | supported | supported |
| `static.crap` | supported | supported | supported | supported | supported | supported | supported |
| `static.dead-code` | supported | supported | supported | supported | supported | supported | supported |
| `static.deps` | supported | supported | supported | supported | supported | supported | supported |
| `tests.assertion-density` | supported | supported | supported | supported | supported | supported | supported |
| `tests.coverage` | unconfigured | unconfigured | unconfigured | unconfigured | unconfigured | unconfigured | unconfigured |
| `tests.flaky` | supported | supported | supported | supported | supported | supported | supported |
| `tests.mutation` | unconfigured | unconfigured | unconfigured | unconfigured | unconfigured | unconfigured | unconfigured |
| `tests.quality` | supported | supported | supported | supported | supported | supported | supported |
| `tests.weakening` | supported | supported | supported | supported | supported | supported | supported |
<!-- end:support-matrix -->

```sh
cupel describe languages           # the seven languages, one line each
cupel describe language go         # extensions, facts produced, checks that apply, toolchain row
cupel describe fact norm-tree      # a fact's schema, version, producers per language, consumers
```

JS and TS form one family: `static.clones` reports a TypeScript function copied from JavaScript. Languages in different families are never compared.

Java is read through its package clauses: a method's name is `package.Class.method`, `public` and `protected` members are public, package-private ones `internal`, and a Maven or Gradle build (`pom.xml`, `build.gradle(.kts)`, `settings.gradle(.kts)`) is a project whose modules are members. Tests are JUnit 5, JUnit 4, and jqwik methods. Clojure is read through its `ns` forms: a var is `namespace/name`, `defn-` and `^:private` make it private, `cond` is its else-if, and a `deps.edn` is a project. Tests are `clojure.test` `deftest`s and test.check `defspec`s. Every language is held to the same conformance inventory, a shared list of constructs per fact that each language must exhibit in its fixture repository and detect, or mark not applicable with a reason.

## Test runners

Checks that depend on running the test suite read a `test-results` fact, which Cupel makes by running the project's own test runner and reading its machine-readable output:

| Language | Runners, in default order | Reads |
| --- | --- | --- |
| Rust | `cargo nextest` (when `.config/nextest.toml` exists), `cargo test` | JUnit through a nextest tool profile; libtest output |
| Go | `go test -json` | test2json events |
| Python | `pytest` (when the project mentions it), `unittest` | JUnit XML; `unittest -v` output |
| JavaScript, TypeScript | `vitest`, `jest` (whichever `package.json` depends on) | the Jest JSON report |
| Java | Maven (`mvn test`, when a `pom.xml` exists), Gradle (`gradle test --rerun`) | Surefire's and Gradle's JUnit XML, one file per test class |
| Clojure | `clojure.test` through the Clojure CLI (`clojure -M:test` and a runner script Cupel writes under `.cupel/`) | `clojure.test.junit` XML |

A runner is used when its program is on `PATH` (or, for `vitest` and `jest`, in the root's `node_modules/.bin`); otherwise checks that need test results are skipped as `unavailable` with the tool named, and `cupel doctor` says what to install (`cupel doctor --fix --install` installs it). `[lang.<id>] runner` chooses one explicitly, by its name with or without the `tool.` prefix:

```toml
[lang.python]
runner = "unittest"
```

Before any check that needs test results runs, Cupel runs the suite once in the real tree, the **baseline**. If any test fails, or the runner cannot finish (a build error), the run is refused: `status: "error"`, exit code 4, no check scores, and one `baseline.test-failed` finding per failing test (with its file, runner, and the tail of its output) or `baseline.run-incomplete` per run that did not finish. Fix the suite, then rerun. A mutation or flakiness score over a red suite would mean nothing, so no mutant and no rerun runs before the baseline is green. Every test of the baseline is also indexed in `.cupel/cupel.db`. `tests.flaky` and `tests.mutation` need test results, so `cupel check` at the default depth (`standard`) runs the baseline; at `--depth quick` no test runs. A language whose baseline ran no test at all skips them as `unavailable`.

### One suite run

The baseline, the coverage report, and the per-test map each need the suite to run. Where the runner and the coverage tool are one toolchain and the `coverage` command is the one `cupel init` writes, a run that needs two of them runs the suite once, under coverage, and takes all three from it:

| Language | One run | Makes |
| --- | --- | --- |
| Rust (runner `cargo-nextest`) | `cargo llvm-cov nextest`, each test process's profile kept apart | baseline, coverage (`cargo llvm-cov`'s own report of the run), per-test map |
| Python (runner `pytest`) | `pytest --cov --cov-context=test` | baseline, coverage, per-test map |
| Go | `go test -json -coverprofile` | baseline, coverage (the per-test map stays one run per test) |

What the run made is cached in `.cupel/cache/suite/` with the hash of every file under the project root. While nothing there changes, the next run reuses it and runs no test, the baseline included: a cached red baseline still refuses the run with exit 4. Any change runs the suite again, so the mutation and flakiness engines always start from a real baseline of the tree they test. A customized coverage command, another runner (`cargo test`, `unittest`), or a pin on one keeps separate runs. `cupel check --dry-run` says which in each check's `needs`:

```console
$ cupel check --only tests.mutation --dry-run    # pytest, with the default coverage command
  "needs": ["symbols", "mutation-outcomes", "test-conventions", "test-results+coverage-map+per-test-map(one run)"]

$ cupel check --only tests.mutation --dry-run    # coverage = "pytest --cov --cov-branch --cov-report=lcov:{out}"
  "needs": ["symbols", "mutation-outcomes", "test-conventions", "test-results(run)", "coverage-map(run)", "per-test-map(deep)"]
```

With the run cached the first says `test-results+coverage-map+per-test-map(cached)`.

### Flakiness

`tests.flaky` reruns the tests the baseline ran, three times by default, with the same runner, and compares the outcomes. The reruns differ on purpose: the first two run the tests in their declared order one at a time, differing only in seed; the third runs them reversed (pytest and unittest, which take the tests in order) or in the runner's seeded shuffle (`go test -shuffle`, libtest's `--shuffle-seed`, `jest --randomize`, `vitest --sequence.shuffle`, Surefire's random class order, which otherwise runs classes alphabetically; Cupel's Clojure runner takes the reversed order); further reruns shuffle again, alternating between the runner's default parallelism and one test at a time. Every rerun's seed is derived from the run's `--seed`, so `cupel check --only tests.flaky --seed N`, which each finding gives as its `repro`, repeats the same reruns.

- `test.flaky`: the test passed and failed in two reruns that ran the tests in the same order. Something besides the code and the order decides it: time, randomness, concurrency, or the environment.
- `test.order-dependent`: the outcome is the same in every rerun of one order and differs between orders. The evidence lists `preceded_by`, the tests that ran before it in every rerun it failed and after it in some rerun it passed, when the runner reports its order.

```toml
[lang.rust]
flake_runs = 5      # reruns per check run; 0 turns them off
```

The reruns repeat the whole suite (impacted-test selection is not built yet), so they cost about three baseline runs. `cargo nextest` runs each test in its own process, where order cannot matter; with it as the runner, the flake engine does not rerun and `tests.flaky` ends in `error` for that language.

## Property tests

`gen.property` finds the property tests among the tests in scope (Hypothesis's `@given`, each test in a `proptest!` block, `proptest!` inside a test, `#[proptest]`, `#[quickcheck]` and `quickcheck(…)`, and fast-check's `fc.assert` and `fc.check`), runs them once more after the baseline with the same runner, and reads back what each framework reports. Every framework keeps its own shrinking; the counterexample in a finding is the framework's minimized input, as it printed it.

Each framework gets the run's `--seed` and the case budget `--cases` (default 1 000 at `--depth standard`, 10 000 at `deep`, 100 000 at `exhaustive`) its own way, and writes nothing outside `.cupel/` while it runs:

| Framework | Runners | How the budget reaches it |
|---|---|---|
| proptest | `cargo test`, `cargo nextest` | `PROPTEST_CASES`, `PROPTEST_RNG_SEED`, and `PROPTEST_DISABLE_FAILURE_PERSISTENCE` (no `proptest-regressions/` files) |
| quickcheck | `cargo test`, `cargo nextest` | `QUICKCHECK_TESTS`; quickcheck takes no seed, so its findings say `seed: null` |
| Hypothesis | `pytest` | A plugin Cupel writes under `.cupel/` loads a settings profile with `max_examples` the budget, unpinned and without an example database, after the project's conftests; the seed is `--hypothesis-seed`, and Hypothesis's files stay under `.cupel/` |
| fast-check | `vitest`, `jest` | A setup file under `.cupel/` calls `fc.configureGlobal` with `numRuns` and `seed`; a configuration file beside it, passed as `--config`, loads the project's own vitest or jest configuration and appends that setup file to its setup files |

A property whose own code fixes its count or seed (`ProptestConfig::with_cases`, `@settings(max_examples=…)`, `fc.assert(p, { numRuns })`) keeps them. A project whose everyday suite runs few cases with a pinned seed is the usual case, and the one `gen.property` exists for: on the fixtures, the suite's ten pinned cases pass and Cupel's thousand find the bug:

```
$ cupel check --only gen.property --seed 3 --format text
cupel check: pass (exit 0)

checks
  gen.property  fail     tier 4 · standard · score 1 falsified (n 1) · metrics falsified 1, missing 1, properties 1 · 2 findings · 1027 ms

fail (1)
  gen.property
    `test_digits_counts_decimal_places` is falsified by `n=100000`
      fnd_2341b0 · property.falsified · tier 4 · confidence 0.95
      at tests/test_digits.py:7:1-8:36
      evidence: cases 1000 · counterexample "n=100000" · framework "hypothesis" · runner "tool.pytest" · seed 3 · test "tests/test_digits.py::test_digits_counts…" (full: cupel explain fnd_2341b0)
      repro: cupel check --only gen.property --paths tests/test_digits.py --seed 3 --cases 1000

info (1)
  gen.property
    `units.percent.clamp_percent` has no property test
      fnd_7307dd · property.missing · tier 4 · confidence 0.5
      at src/units/percent.py:1:1-3:35
        nearest_test_file: tests/test_percent.py
```

With no gate the run passes (exit 0) while the check fails; a `[gate]` rule such as `"gen.property.falsified" = "== 0"` makes it exit 1.

`property.falsified` gives the framework, the runner, the counterexample, the seed, and the case budget as evidence, and a `repro` that runs the same property with the same seed and budget. A property that failed without a counterexample (it errored before any case ran) is reported the same way with `counterexample: null` and lower confidence.

`--scaffold` turns each `property.missing` into a skipped stub that names the function, generates one input per parameter (typed by the function's signature where the language's table maps the type), and calls it, for you to state the property and remove the skip marker: a Hypothesis test appended to the source's nearest test file in Python, a fast-check test in JavaScript and TypeScript (imported the way the file imports), and an ignored proptest test in an inline `#[cfg(test)]` module of the function's own file in Rust, where a parameter type the table does not map (a struct, a generic) gets no stub. Without `--write` nothing is written and `plan.would_write` holds each file's diff; with it the stubs are written, a file's header lines once, and a function already stubbed is left alone.

```sh
cupel check --only gen.property --scaffold          # preview the stubs
cupel check --only gen.property --scaffold --write  # write them
```

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
| Java | JaCoCo through Maven (its goals by coordinate around `mvn test`), JaCoCo through Gradle (`gradle test jacocoTestReport`, with the `jacoco` plugin applied and XML reports on) | yes |
| Clojure | Cloverage (`-Sdeps` adds it for the run, `--lcov`) | no |

Without a `coverage` command a language's `tests.coverage` is skipped as `unconfigured` and `static.crap` reports no CRAP; with one but without the tool, it is `unavailable` and `cupel doctor` names the tool. A report is reused instead of rerunning the suite while it is fresh: Cupel records the hash of every file under the project root when it makes one (and trusts files not modified since), and a report the tool left at its default location is used when it is newer than every source. A command that exits non-zero gives no coverage and a `coverage.run-failed` finding, since a red suite has no meaningful coverage.

`tests.coverage` reports `coverage.uncovered-symbol` for each function no test ran (a warning, which fails the check) and `coverage.uncovered-branch` for a function that ran with a branch no test took. Where the tool measured no branches (Go, Rust), there are no branch findings and the result says `notes: ["branches: unavailable (go)"]`. Its gate keys are `tests.coverage.line` and `tests.coverage.branch` (covered shares), `lines`, `branches`, and `uncovered`. `static.crap` computes `CC² × (1 − coverage)³ + CC` from the same map.

## Mutation testing

`tests.mutation` scores a `mutation-outcomes` fact, which the mutation engine makes from the `mutation-sites` every language produces, the coverage report, and the baseline. The engine runs each mutant with the same runner as the baseline; the check only counts and reports, so another producer of the fact feeds the same check. For Java that producer can be PIT, through the [PIT plugin](#the-pit-plugin). Coverage gates which mutants run, so a language without a `coverage` command skips `tests.mutation` as `unconfigured`, as it does `tests.coverage`.

- `mutant.survived` (fails the check): every test that ran against the mutant passed. The finding sits at the mutated site and names the change (`from`, `to`, `operator`), the tests that missed it (`covering_tests`), and the nearest test file. Write a test that fails on the mutant.
- `mutant.uncovered` (a note): no test runs the site, so its mutant was not run.
- `mutant.timeout` (a note): the tests ran past their timeout against the mutant, which usually means it loops.
- `mutant.equivalent?` (a note, confidence 0.3): the mutant survived but may behave exactly as the original — today, swapping `+`/`-` before a literal `0` or `*`/`/` before a literal `1`. It is left out of the score; suppress it once you agree.

The score `tests.mutation.score` is killed / (killed + survived), 1 when nothing was scored. The other gate keys count mutants by outcome (`killed`, `survived`, `timeout`, `uncovered`, `unviable` for one that did not build, `equivalent`) and say how many ran this time (`executed`) or were reused (`reused`), and how many of those ran behind a schemata build (`schemata`) or fell back to file copies (`fallback`).

How much is mutated follows the depth: at `standard` with `--changed` or `--base`, the functions the change touched; at `deep` and `exhaustive`, every function of the files in scope (the touched files with `--changed`, the tree without). Under a range, `tests.mutation` scores only the mutants in the functions the change touched (ADR-142). Test code is never mutated. Only the `default` operators run unless the language asks for more:

```toml
[lang.python]
mutation = { operators = "extended", jobs = 2, timeout_factor = 10 }

[lang.rust]
mutation = { strategy = "file-copy" }   # "schemata" (the default) or "file-copy"
```

Mutation testing is expensive, so the engine stacks every saving it can. Before the first mutant, the baseline, the coverage report, and the per-test map come from [one run of the suite](#one-suite-run) where the toolchain allows, or from its cache when nothing changed. Then:

1. **Coverage gating.** A site no test runs, by the coverage report, is reported `uncovered` and never run.
2. **Reuse.** An outcome is stored in `.cupel/cupel.db` under a key of the function's source text, the site, the replacement, the tests that cover it, and the runner's version; a later run reuses it while that key holds. A second run with nothing changed runs no mutant; changing one test file reruns only the mutants that test covers.
3. **Per-test selection.** Where the language has a per-test coverage map from the baseline's runner (Go; Python with pytest-cov; Rust when cargo-nextest is the runner and cargo-llvm-cov is installed), a mutant runs only the tests that execute its line. The [one suite run](#one-suite-run) makes the map for Rust and Python at any depth. Otherwise, measuring a map from nothing happens at `--depth deep` and beyond; at `standard` the engine uses a cached map (see [`cupel tests --impacted`](#cupel-tests---impacted)) when refreshing it is cheap: one suite run for Rust and Python, at most 32 tests for Go. `cupel check --dry-run` says which, in the check's `needs`: `per-test-map(cached)`, `per-test-map(refresh 3 stale)`, or `per-test-map(deep)` when the depth leaves it out. Without a map a mutant runs the whole suite.
4. **Schemata (Rust).** Instead of one build per mutant, each Rust file is rewritten once with every mutant behind a run-time switch — the smallest whole expression around each site becomes `if cupel_mutant_id() == N { mutated } else { original }`, and a small reader of the `CUPEL_MUTANT` environment variable is appended to the file — so the file builds once and each mutant runs with `CUPEL_MUTANT=N`. Every line stays where it was. Before any mutant runs, each worker builds the rewritten file and runs its tests with no mutant selected; they must pass. Sites where an `if` cannot stand (patterns, `const` and `static` items, `const fn`, array lengths, macro arguments, attributes) and constants the compiler checks run as ordinary file copies. If the rewritten file does not compile, the sites on the lines the compiler names run as file copies and the file is rebuilt; if it still fails, the whole file runs as file copies. Nothing is lost either way: outcomes are the same under both, which the tests check on the Rust fixtures. The check reports how many mutants ran each way (`schemata`, `fallback`) and a note for each reason a mutant fell back. Other languages build per mutant (Go's builds take a fraction of a second; see [ADR-005](docs/adr/CUPEL_DECISION_005.md)).
5. **Overlays.** Mutants run in copies of the tree outside the repository, one per worker, kept between runs so builds stay incremental: files are cloned (macOS) or hard-linked, what `.gitignore` ignores is shared through symlinks except the runner's build directory (`target/` for cargo), and the mutated file is a private copy. Mutants of one file run concurrently; files run one after another.
6. **Timeouts.** Before its first mutant of a file, each worker times the unmutated file's tests (for Rust, its schemata build's), rebuild included; a mutant gets ten times that (`timeout_factor`), at least two seconds.
7. **Order.** Last run's survivors run first, then mutants of functions never run before, then the rest, so a time limit cuts the least informative work.

A Rust file's mutants cost one build per worker plus one test run each; outside Rust, each mutant costs an incremental build of what it changes plus the tests. The run's `--seed` reaches every mutant run.

On the Cupel repository itself (1377 tests, release build, 8-core Apple silicon, a quiet machine), a one-function change checked at `standard` with a warm cache takes about 10.5 minutes: one instrumented run of the suite (5.3 min) and the mutants (5.2 min, two mutants with sixteen covering tests each); an unchanged rerun takes 2 seconds.

## Differential testing

`gen.differential` compares functions with an [oracle](#cupel-oracle) on generated inputs. You name the functions, per language, as `<file>:<function>`; the file is relative to the repository root:

```toml
[oracles.release]
kind = "ref"
ref = "v1"

[lang.python]
differential_entries = ["shipping.py:shipping"]
```

For each entry in scope, the differential engine reads the function's parameter types from its signature, generates inputs from them (boundary values first: 0, 1, -1, the type's extremes, empty strings and lists; then random values that grow in size), and calls the function on every input in the working tree and through the oracle. A `ref` oracle runs the same function at the commit its ref names, exported into a temporary directory; a `command` oracle runs once per input with `{input}` replaced by the argument (a string's text, any other value's JSON) or, for several parameters, the JSON array of them, and its standard output is its answer; a command oracle's known inputs (`--input`) are compared first. The first input on which the answers differ is shrunk step by step (integers toward 0, strings and lists shorter) while the answers still differ, so the finding shows the smallest such input. Nothing is written in the working tree.

```console
$ cupel oracle verify release
$ cupel check --only gen.differential --seed 7 --format text
cupel check: pass (exit 0)

summary   1 finding · fail 1 · warn 0 · info 0 · new 0 · resolved 0 · suppressed 0

checks
  gen.differential  fail     tier 4 · deep · score 1 divergent (n 1) · metrics cases 100, divergences 1 · 1 finding · 5993 ms · oracle release

fail (1)
  gen.differential
    `shipping(20001)` returned `2500`, but oracle `release` returned `2600`
      fnd_fb42fe · diff.divergence · tier 4 · confidence 1
      at shipping.py:1:1-3:52 in shipping.shipping (python, sh_d5d28ffa7e1a1f68)
      …
      action fix_bug (priority 2): Decide which answer is right for `shipping(20001)`: fix the implementation, or, if the change is intended, the oracle.
      repro: cupel check --only gen.differential --symbols sh_d5d28ffa7e1a1f68 --oracle release --seed 7 --cases 100
```

The change capped the cost at 2500; the boundary input `9223372036854775807` diverged first and shrank in 53 steps to `20001`. The finding's evidence holds the `entry`, the `oracle` (`name`, `kind`, and a ref's `commit`), the minimized `input`, the input as `found`, its `case` index, the `shrinks`, and both answers, `implementation` and `reference`, each one of `{"returned": …}`, `{"raised": …}`, or `{"crashed": …}`. The repro regenerates the same inputs: the seed decides them, and `--cases` sets how many are generated per entry (100 by default). The gate keys are `gen.differential.divergences` (entries that diverged) and `cases` (inputs compared); when no entry could be compared, the check passes with no score and no metrics, and its notes say why.

| Language | How it is called | Parameter types it generates |
|---|---|---|
| Python | `python3 -B` imports the module and calls the function; answers are JSON (or `repr`), exceptions `raised` | `int`, `float`, `bool`, `str`, `list[T]`, `Optional[T]` / `T \| None`; unannotated parameters get mixed values |
| JavaScript, TypeScript | `node` imports the file (TypeScript by Node's type stripping, Node 22.18 or later) and calls the named export | `number`, `string`, `boolean`, `T[]`, `T \| null`; JavaScript's untyped parameters get mixed values |
| Go | an internal test file laid over the package with `go test -overlay`, built once | integer types, `float32`, `float64`, `bool`, `string`, `[]T` |
| Rust | a separate crate that depends on the entry's crate by path, built once into `.cupel/cache/differential/` | integer types, `f32`, `f64`, `bool`, `String`, `&str`, `Vec<T>`, `&[T]`, `Option<T>` |

An entry is called with whatever side effects it has, so name functions that are safe to call with any argument. It must be a module-level, public function (in Rust, one reachable from the crate root): an entry that is missing, private, a method, or has a parameter of another type is not compared, and the result's notes say why. Without `differential_entries` a language is skipped as `unconfigured`; without its program (`python3`, `node`, `go`, `cargo`) it is `unavailable`. A command answers in text, so against a command oracle a returned string matches the same text unquoted, and any error matches any error; against a `ref` oracle answers match when they are equal as text or as JSON.
## Metamorphic relations

A metamorphic relation says how a function's output changes when its input does, without saying what the output is: sorting a list and sorting its reverse give the same list. `gen.metamorphic` checks relations you declare in `cupel.toml`, per language, under `[lang.<id>.relations.<name>]`. Python is the language with a harness today.

```toml
[lang.python.relations.sort-reverse]
function = "sorting:sort_list"          # module:name, imported from the repository root
input = "list[int]"                     # bool, int, str, list[T], tuple[T, ...]
transform = "lambda xs: list(reversed(xs))"   # the follow-up input, from the input

[lang.python.relations.add-commutes]
function = "arith:add"                  # a tuple input is spread into the arguments: add(a, b)
input = "tuple[int, int]"
transform = "lambda pair: (pair[1], pair[0])"
relation = "lambda a, b: a == b"        # how the two outputs relate; default ==
cases = 50                              # inputs to generate; default 100
```

`transform` and `relation` are Python expressions evaluated in the function's module. For every input the harness calls the function on it, the transform on it, and the function on the transform's result, each on its own copy; the relation is violated when it returns false or when any of those calls raises. A relation whose function cannot be imported, or whose expressions do not evaluate to callables, makes the check `error` with the reason in its `notes`; a key that is not one of these five, a missing one, or an input type outside the list is a usage error (exit 2) when `cupel.toml` loads.

Inputs are generated from `--seed`, the relation's name, and the case number alone, starting small (lists of at most one element, integers within ±16) and growing to lists and strings of up to eight elements and integers within ±1024. `--cases N` sets how many inputs every relation gets, over its `cases`. The first input that breaks a relation is minimized: integers move toward zero, lists and strings shrink and their elements get simpler, as long as the relation still breaks. The finding's evidence has the minimized input, the transformed input, both outputs (each `{"value": …}`, `{"raised": "Type: message"}`, or `{"repr": …}` for a value JSON cannot hold), the generated input it started from, and the seed; its `repro` reruns the same inputs. A relation is one finding however many inputs break it, located at the function's definition. In the fixture, `sort_list` makes a single bubble pass:

```text
$ cupel check --only gen.metamorphic --seed 3 --format text
checks
  gen.metamorphic  fail     tier 4 · deep · score 1 violated (n 2) · metrics relations 2, violated 1 · 1 finding · 1042 ms

fail (1)
  gen.metamorphic
    `sorting:sort_list` breaks the relation `sort-reverse`
      fnd_30a135 · metamorphic.violated · tier 4 · confidence 1
      at sorting.py:1-7
      evidence: case 11 · followup_output {1} · function "sorting:sort_list" · input [3] · input_type "list[int]" · original_input [3] · relation "sort-reverse" · seed 3 · shrinks 10 · source_output {1} · transformed {1} (full: cupel explain fnd_30a135)
      action fix_bug (priority 1): Fix `sorting:sort_list` so that `sort-reverse` holds for the input [0,1,1]; the evidence has the follow-up input and both outputs.
      repro: cupel check --only gen.metamorphic --seed 3
```

The minimized input `[0, 1, 1]` sorts to `[0, 1, 1]`, its reverse `[1, 1, 0]` to `[1, 0, 1]`. A failing check does not fail the run by itself; a gate rule such as `"gen.metamorphic.violated" = "== 0"` does. Without any relation declared for a language, the check is skipped there as `unconfigured` (`lang.python.relations`); the support matrix above, which knows no repository's configuration, shows Python that way. The other four languages have no harness yet, so the check is `unsupported` for them. Every relation is evaluated whatever the scope, and only those whose function is defined in a file in scope are reported.

## Analyzers

`static.analyze` runs every analyzer of a language that is installed, in each project root, and keeps the results in files in scope. Type checkers run at their strictest; linters use the project's own configuration.

| Language | Analyzers | Run as |
|---|---|---|
| Rust | clippy | `cargo clippy --all-targets --message-format=json`, with the project's `[lints]` |
| Python | mypy, ruff | `mypy --strict --output json` and `ruff check --output-format json` on the files in scope |
| JavaScript | ESLint | `eslint --format json`, only where the root has a flat config (`eslint.config.js` and its siblings) |
| TypeScript | tsc, ESLint | `tsc --noEmit --strict` over the root's `tsconfig.json`, else over the files in scope; ESLint as for JavaScript |
| Go | go vet, staticcheck | `go vet -json` and `staticcheck -f json` over the module's packages |

The check runs when at least one of a language's analyzers is installed; each one that is missing, unconfigured (ESLint without a config), or failed (its output could not be read) is a note in the result saying what to install or fix, and the others still run. The finding's evidence holds the `tool`, its `rule`, the `level`, and the `message`; its ID does not change when the line moves. The gate keys are `static.analyze.errors` and `diagnostics`. Results depend on each tool's version, so pin them where a gate reads them.

## Dependencies and footprint

`perf.footprint` reads, in each project root, the lockfile or manifest the ecosystem keeps, and never the network:

| Language | Dependencies | Licenses |
|---|---|---|
| Rust | `Cargo.lock`, except the workspace's own packages | `cargo metadata --offline --locked`, from the local registry cache |
| Python | `uv.lock`, else `poetry.lock`, else `requirements*.txt` | the package metadata in `.venv/` or `venv/` |
| JavaScript, TypeScript | `package-lock.json`, else `package.json` (without versions) | the lockfile |
| Go | `go.mod`'s `require` lines | not read |

A dependency is direct when the project declares it itself. Two measurements are opt-in, per language:

```toml
[lang.rust]
advisories = "advisories"            # a directory of OSV advisories (*.json), such as an osv.dev export
artifacts = ["target/release/app"]   # files or globs whose sizes the artifact_bytes metric adds up
```

A dependency whose name and version an advisory's `affected` entry lists, or falls in its ranges, is `dep.vuln` (`fail`, with the advisory and the fixed version); a license expression with no alternative free of GPL, AGPL, LGPL, SSPL, EUPL, or OSL is `dep.license` (`warn`). Under `--changed` or `--base` the same files are read at the base commit, and only dependencies whose name and version are new are reported; `footprint.grew` (`warn`) lists what was added and removed when the count grew. The gate keys are `perf.footprint.dependencies`, `vulnerable`, `copyleft`, and `artifact_bytes` (a size budget, such as `"perf.footprint.artifact_bytes" = "<= 4000000"`); what was not measured (licenses, advisories, artifacts) is a note, never a silent zero.

## Race detectors

`conc.race` runs a language's tests in each project root under its race detector, and reports what the detector saw:

| Language | Detector | Run as |
|---|---|---|
| Go | Go's race detector | `go test -race -count=1 -json` over the module's packages |
| Rust | Miri (`tool.miri`, the default) | `cargo +nightly miri test --no-fail-fast`, one test at a time, with full backtraces |
| Rust | ThreadSanitizer (`tool.tsan`) | `cargo +nightly test -Zbuild-std --target <host> --tests` with `-Zsanitizer=thread` |

Miri interprets every test, so it finds undefined behavior (an out-of-bounds read, a dangling pointer, an invalid value) as well as data races, but it refuses what it cannot model (foreign functions, most file and process calls); ThreadSanitizer runs the tests natively and finds data races only. Both need a nightly toolchain (`rustup toolchain install nightly --component miri --component rust-src`); `[lang.rust] race = "tsan"` picks ThreadSanitizer. Builds go to `.cupel/cache/race/`, so nothing is written in the tree. Python, JavaScript, and TypeScript have no race detector, and the matrix says `unsupported`.

A report's two accesses are listed in the order of their place in the code, whichever happened first, and only frames in the repository are kept, so the same race reads the same on every run and machine. On the Go fixture, where `Publish` reads a variable a goroutine writes without synchronizing:

```text
$ cupel check --only conc.race --format text
fail (1)
  conc.race
    data race: `write` in `example.com/tally.Publish.func1` and `read` in `example.com/tally.Publish`
      fnd_c9f4ec · race.detected · tier 4 · confidence 1
      at tally.go:10
      evidence: message "data race" · stacks [2] · test "TestPublish" · tool "tool.go-race" (full: cupel explain fnd_c9f4ec)
      action fix_bug (priority 1): Synchronize the accesses (a lock, an atomic, a channel) or stop sharing the memory, then rerun the race detector.
      repro: cupel check --only conc.race --paths tally.go
```

The evidence holds the `tool`, the `test` that was running, the detector's `message` (addresses and allocation IDs masked), and the `stacks`, each an `access` (`read`, `write`, `non-atomic write`; none for undefined behavior) and its `frames`. Under `--changed` a report is kept when one of its frames is in a changed file. A project root whose run could not test what it was given (a build failure, `-race` without cgo, an operation Miri does not support) is a note, beside whatever was found before it. The gate keys are `conc.race.races` and `undefined_behavior`.

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

## Fuzzing

`gen.fuzz` fuzzes every target in each project root that holds a file in scope, each with the same execution budget: `--cases`, else 1 000 inputs at `--depth standard` (with `--only`), 10 000 at `deep`, and 100 000 at `exhaustive`, each input limited to 10 s and each target to 300 s.

| Language | Fuzzer | Targets | Ready when |
|---|---|---|---|
| Rust | cargo-fuzz (libFuzzer) | the binaries of a `fuzz/Cargo.toml` with `cargo-fuzz = true`, built with `cargo +nightly fuzz build` and run directly | `cargo-fuzz` and a nightly toolchain are installed |
| Go | Go's native fuzzer | `FuzzXxx` functions, run with `go test -fuzz=^<Name>$ -fuzztime=<cases>x` | `go` is installed |
| JavaScript | Jazzer.js | `*.fuzz.js` modules exporting `fuzz` | `jazzer` is in `node_modules/.bin` or on `PATH` |
| Python | atheris | `fuzz_*.py` scripts that call `atheris.Fuzz(` | `python3` imports `atheris` |

libFuzzer-based fuzzers get the run's `--seed`, so `--seed` and `--cases` repeat a run on one toolchain; Go's fuzzer takes no seed. Nothing is written in the tree: each target's corpus and saved inputs live in `.cupel/cache/fuzz/` and are emptied before each run (a corpus the project commits is read as well), and Go, which saves a failing input into the package's `testdata/`, fuzzes a copy of the module. atheris builds only against a clang that ships libFuzzer, so on macOS with Apple's clang the Python pair is `unavailable`; TypeScript has no producer (Jazzer.js loads only `.js` modules) and is `unsupported`. On the Rust fixture, whose `first` target indexes its input:

```text
$ cupel check --only gen.fuzz --format text
fail (1)
  gen.fuzz
    fuzz target `first` crashes on the empty input: index out of bounds: the len is 0 but the index is 0
      fnd_d59cd3 · fuzz.crash · tier 4 · confidence 1
      at src/lib.rs:5:5-5
      action fix_bug (priority 1): Fix `first`'s failure on the saved input (`cargo +nightly fuzz run first .cupel/cache/fuzz/rust/root/first/inputs/crash-da39a3ee5e6b4b0d3255bfef95601890afd80709` reruns it), then keep the input in the target's corpus as a regression case.
      repro: cupel check --only gen.fuzz --paths fuzz/fuzz_targets/first.rs --seed 3 --cases 1000
```

The evidence holds the `target`, the `tool`, the `message`, the `input` (as text, or `hex:` and its bytes), the `reproducer` file and the `reproduce` command (for Go, the commands that make the input a regression case under `testdata/fuzz/`), the `seed`, and the `cases`. An input that runs past the per-input limit is `fuzz.timeout` (`warn`, confidence 0.7: a slow path under load can cross it). A target that does not build is a note. The gate keys are `gen.fuzz.crashes`, `timeouts`, `targets`, and `missing`.

## Linearizability

`conc.linearizability` checks histories of concurrent operations that your own tests or harness record: Cupel never runs the system itself. Name the command in `[lang.<id>] history_command`; Cupel runs it from the repository root with `CUPEL_HISTORY_DIR` set to an empty directory under `.cupel/cache/linearizability/` and `CUPEL_SEED` set to the run's seed, and checks every `*.jsonl` file the command writes there. Until the command is set the check is `unconfigured` for the language.

A history is JSON lines in the order things happened. The first line declares the model, `register` (one value: `read`, `write`, and `cas` with the value `[expected, new]`) or `kv` (the same per `key`, checked key by key); each other line is an event of a process, an `invoke` and then its completion, `ok` (it took effect), `fail` (it did not), or `info` (unknown: a timeout, a crash). An invocation the history never completes is unknown too, and may have taken effect anywhere after it began.

```text
{"model": "register"}
{"process": 0, "type": "invoke", "f": "write", "value": 1}
{"process": 0, "type": "ok", "f": "write", "value": 1}
{"process": 0, "type": "invoke", "f": "write", "value": 2}
{"process": 0, "type": "ok", "f": "write", "value": 2}
{"process": 1, "type": "invoke", "f": "read", "value": null}
{"process": 1, "type": "ok", "f": "read", "value": 1}
```

Porcupine (v1.3.1, run with `go run` through a small runner Cupel writes into `.cupel/cache/linearizability/porcupine/`) decides whether some order of the operations, consistent with real time, is a legal run of the model. The read above began after the write of 2 completed, so no order explains it:

```text
$ cupel check --only conc.linearizability --format text
fail (1)
  conc.linearizability
    history `histories/stale-read.jsonl` (register) is non-linearizable: `read 1` by process 1 (line 6) cannot follow state `2`
      fnd_a1c5c8 · consistency.violation · tier 5 · confidence 1
      evidence: anomaly "non-linearizable" · command "cp -R histories "$CUPEL_HISTORY_DIR"/" · history "histories/stale-read.jsonl" · linearized [2] · model "register" · seed 3 · states [1] · stuck [1] · tool "tool.porcupine" (full: cupel explain fnd_a1c5c8)
      action fix_bug (priority 1): Find why `read 1` by process 1 (line 6) cannot follow state `2` in `histories/stale-read.jsonl`: the operations a linearizable system completes take effect in an order real time allows. `CUPEL_SEED=3 cp -R histories "$CUPEL_HISTORY_DIR"/` records the history again.
      repro: cupel check --only conc.linearizability --seed 3
```

The evidence holds the `linearized` operations (the last three of the longest prefix the checker could order), the `states` the model is in after them, and the `stuck` operations that could come next in real time but that the model does not allow; each operation names its `process`, `f`, `key`, `value`, `outcome`, and the `line` that invoked it. A history that is malformed, or declares a model Porcupine does not check, is a note; so is one the checker ran out of time on (30 s per key, 300 s at `exhaustive`). The command's failure is a note, and the histories it wrote are still checked. Porcupine is downloaded from the Go module proxy on the first run; Go must be on `PATH`. The gate keys are `conc.linearizability.violations`, `histories`, `operations`, `unknown`, and `invalid`.

## Deterministic simulation

`conc.dst` runs deterministic simulation harnesses over a range of seeds. A simulator replaces a program's clock, network, scheduler, and randomness with one driven by a seed, so a seed replays the same interleaving, delays, and faults every time. Cupel wraps [madsim](https://github.com/madsim-rs/madsim): in each Rust project root holding a file in scope, every package that depends on `madsim` has its library and test targets run as `cargo test` under `RUSTFLAGS=--cfg madsim`, building into `.cupel/cache/dst/madsim/target`; its `#[madsim::test]` functions are the harnesses. The seeds are the run's seed and the ones after it: `--cases`, else 1 000 at `--depth standard` (with `--only`), 10 000 at `deep`, and 100 000 at `exhaustive`, where the check runs by default. madsim stops a harness at its first failing seed and names it; Cupel then replays that seed alone, which gives the trace of that seed only and shows whether the failure repeats. Other languages have no simulator Cupel wraps, and the matrix says `unsupported`.

On the fixture, where two deposits read a balance, wait on an audit, and write it back, so overlapping audits lose one:

```text
$ cupel check --only conc.dst --seed 3 --format text
fail (2)
  conc.dst
    simulation `a_withdrawal_waits_for_its_deposit` fails at seed 5: overdrawn: 10 from 0
      fnd_d7a5ab · dst.failure · tier 4 · confidence 1
      at src/lib.rs:29:9-29
      ...
    simulation `concurrent_deposits_are_all_kept` fails at seed 4: assertion `left == right` failed: a deposit was lost
      fnd_350eac · dst.failure · tier 4 · confidence 1
      at tests/sim.rs:23:5-23
      evidence: first_seed 3 · harness "concurrent_deposits_are_all_kept" · message "assertion `left == right` failed: a depo…" · reproduce "RUSTFLAGS='--cfg madsim' MADSIM_TEST_SEE…" · reproduced true · seed 4 · seeds 1000 · tool "tool.madsim" · trace [10] (full: cupel explain fnd_350eac)
      action fix_bug (priority 1): Fix the failure `concurrent_deposits_are_all_kept` shows at seed 4 (`RUSTFLAGS='--cfg madsim' MADSIM_TEST_SEED=4 MADSIM_TEST_NUM=1 cargo test -p ledger --test sim concurrent_deposits_are_all_kept -- --exact` replays it from `.`), then keep the seed as a regression case of the harness.
      repro: cupel check --only conc.dst --paths tests/sim.rs --seed 4 --cases 1
```

The repro runs that one seed again (`--seed 4 --cases 1`). The evidence holds the `harness`, the failing `seed`, the range (`first_seed`, `seeds`), the panic's `message`, the `trace` (what the replayed seed printed, at most 50 lines, with thread numbers and paths outside the repository removed), whether it `reproduced` alone (a failure that does not repeat is a harness that is not deterministic, and is reported with confidence 0.5), and the `reproduce` command. A root whose build fails is a note, and so is a root with no harness. The gate keys are `conc.dst.failures` and `harnesses`.
## Trace validation

`spec.trace` checks that what the implementation did is a behavior of its specification. The specification is a `spec` oracle in TLA+ (`cupel oracle add counter --kind spec --spec spec/Counter.tla --lang tla+ --write`), and the evidence is a trace the implementation logs while its tests run. The convention is a file format, so it needs no build step or library in any of the seven languages: when the variable `CUPEL_TRACE_DIR` is set, a test writes one `.jsonl` file per behavior into that directory, one JSON object per step, and writes nothing when it is unset.

```json
{"state": {"count": 0, "done": false}}
{"action": "Inc", "state": {"count": 1, "done": false}, "file": "counter.go", "line": 24}
```

`state` holds the values of the spec's variables after the step (a variable left out is left to the spec); `action` names the spec action the step took, `file` and `line` where it was logged, all three optional. Integers, strings, booleans, arrays (sequences), and objects (records) map to TLA+ values, and `{"$tla": "{1, 2}"}` is any TLA+ expression (a set, a model value). The producer reruns the suite once with `CUPEL_TRACE_DIR` pointing at a fresh `.cupel/cache/trace/` directory, then runs TLC (`tlc` on `PATH`, as tla-bin installs it) on a module that extends the spec and allows from each logged state only the spec's next-state relation (or the named action) to the next logged state. The spec's `INIT`, `NEXT`, and `CONSTANTS` come from the `.cfg` beside it, else `Init` and `Next`. On the Go fixture, whose `Bump` adds two but logs an `Inc`:

```text
$ cupel check --only spec.trace --format text
fail (1)
  spec.trace
    trace `bumps.jsonl` leaves spec/Counter.tla at step 3: `Inc` to {"count":3,"done":false} is not a step the spec allows from {"count":1,"done":false}
      fnd_c58eaf · trace.diverged · tier 5 · confidence 1
      at counter.go:29
      evidence: action "Inc" · oracle "counter" · previous {2} · spec "spec/Counter.tla" · state {2} · step 3 · steps 3 · tool "tool.tlc" · trace "bumps.jsonl" (full: cupel explain fnd_c58eaf)
      repro: cupel check --only spec.trace --oracle counter --paths counter.go
```

A trace whose first state is not an initial state of the spec diverges at step 1. A trace that is not JSON lines of steps, or that TLC rejects (a variable or action the spec does not have), is a note, and so is a recording run whose tests failed or that recorded nothing. The result is against an oracle, so it is tier 5 only while `cupel oracle verify` holds for the spec; otherwise it is capped at tier 3 with `oracle.unverified` (see [`cupel oracle`](#cupel-oracle)). Quint and P specs are oracles TLC does not read, and the result says so. The gate keys are `spec.trace.diverged`, `traces`, and `unvalidated`.

## Model checking

`conc.model-check` runs Kani (`cargo kani`, with concrete playback) in each Rust project root and checks every `#[kani::proof]` harness over all the inputs its `kani::any()` values can take, within the harness's bounds. Builds go to `.cupel/cache/model-check/`. Each property an input breaks is a finding at the place it breaks, with the values, in the order the harness drew them, and the unit test Kani wrote to replay them:

```text
$ cupel check --only conc.model-check --format text
fail (2)
  conc.model-check
    harness `proofs::average_never_overflows` refutes `attempt to add with overflow` in `average` with 128, 128
      fnd_f4c6a6 · model.counterexample · tier 5 · confidence 1
      at src/lib.rs:3
      evidence: description "attempt to add with overflow" · function "average" · harness "proofs::average_never_overflows" · replay "/// Test generated for harness `proofs::…" · tool "tool.kani" · values [2] (full: cupel explain fnd_f4c6a6)
      repro: cupel check --only conc.model-check --paths src/lib.rs
    harness `proofs::at_stays_in_bounds` refutes `index out of bounds: the length is less than or equal to the given index` in `at` with 4ul
      …
```

A harness Kani cannot decide is a note, and so is a root that does not build. Kani needs `cargo install --locked kani-verifier` and `cargo kani setup`, which downloads its bundle and the nightly it builds with. The other languages have no model checker, and the matrix says `unsupported`. The gate keys are `conc.model-check.failed`, `harnesses`, `verified`, and `undetermined`.

## Proofs

`spec.prove` runs `dafny verify` over the `.dfy` files in each project root of every language (Dafny compiles to several of them, and its proofs stand on their own), and reports each obligation the verifier did not discharge, at its place:

```text
$ cupel check --only spec.prove --format text
fail (1)
  spec.prove
    proof fails at proofs/tally.dfy:24: a postcondition could not be proved on this return path
      fnd_993483 · proof.failed · tier 6 · confidence 1
      at proofs/tally.dfy:24:1-24:2
      evidence: message "a postcondition could not be proved on t…" · related [1] · tool "tool.dafny" (full: cupel explain fnd_993483)
      repro: cupel check --only spec.prove --paths proofs/tally.dfy

warn (2)
  spec.prove
    proof obligation open at proofs/tally.dfy:31: assume statement has no {:axiom} annotation
      fnd_6f1c03 · proof.obligation-open · tier 6 · confidence 1
      …
    proof obligation open at proofs/tally.dfy:35: This ensures clause is part of a bodyless method. Add the {:axiom} attribute to it or the enclosing method to suppress this warning
      …
```

An obligation the verifier could not prove is `proof.failed` (`fail`), with the places it relates (the postcondition a return path breaks). One the proofs assume (an `assume` without `{:axiom}`, a lemma or method without a body) or that ran out of time or resources is `proof.obligation-open` (`warn`): nothing refutes it, and nothing proves it either. A file that does not parse or resolve is a note. Dafny's releases bundle Z3; put `dafny` on `PATH`. The gate keys are `spec.prove.failed`, `open`, and `verified`.
## Contracts

`spec.contracts` reruns the suite once more through the baseline's runner, after the baseline passes, with the contract library's switches on and every property test given the run's seed and case budget (as `gen.property` gives them), and reads each failed test's output for the library's report. A project root whose manifests do not declare the library is not run again, and a note says nothing was checked.

| Language | Library | Contracts | Turned on for the run |
|---|---|---|---|
| Rust | the `contracts` crate | `#[requires]`, `#[ensures]`, `#[invariant]`, and their `debug_` and `test_` forms | debug assertions in every profile a test builds with (`CARGO_PROFILE_DEV_DEBUG_ASSERTIONS`, `CARGO_PROFILE_TEST_DEBUG_ASSERTIONS`), which `debug_*` contracts need |
| Python | icontract | `@require`, `@ensure`, `@invariant` | `ICONTRACT_SLOW=1`, which enables contracts declared `enabled=icontract.SLOW` |

A violation is placed at the contract (the attribute or decorator), names the function or class it is declared on, and keeps the library's message: the description and condition, and for icontract the values it saw, with addresses masked. Under `--changed` a violation is kept when its contract or its test is in a changed file. On the Rust fixture, whose `debug_*` contracts its own `[profile.dev]` leaves unchecked:

```text
$ cupel check --only spec.contracts --format text
fail (3)
  spec.contracts
    precondition of `share` violated in `ledger/src/lib.rs::tests::shares_nothing_among_no_parts`: a share needs at least one part: parts > 0
      fnd_1d32c0 · contract.violated · tier 5 · confidence 1
      at src/lib.rs:4
      action fix_bug (priority 1): A caller passed what the precondition rejects: fix the call the test makes, or the precondition if it is wrong, then rerun.
      repro: cupel check --only spec.contracts --paths src/lib.rs
    postcondition of `half` violated in `ledger/src/lib.rs::tests::halves_one`: half of a positive amount is positive: ret > 0
      fnd_4aa38d · contract.violated · tier 5 · confidence 1
      at src/lib.rs:10
    invariant of `post` violated in `ledger/src/lib.rs::tests::posts_three_entries`: an account holds two entries: self.entries.len() <= 2
      fnd_bedc57 · contract.violated · tier 5 · confidence 1
      at src/lib.rs:25
```

The evidence holds the `tool`, the contract's `kind` (`precondition`, `postcondition`, `invariant`), the `function`, the `message`, and the `test`. A run that did not complete in a root is a note. The gate key is `spec.contracts.violations`. Go, JavaScript, TypeScript, Java, and Clojure have no producer: their tests' failures do not say a contract failed (an `assert`, a Guava precondition, and a Clojure `:pre` all read as plain assertions), so the pair is `unsupported`.

## Requirements and test provenance

`meta.traceability` checks a change against the task's requirements. List their IDs in `cupel.toml`, under a per-language override when a repository's languages trace different ones:

```toml
[checks."meta.traceability"]
requirements = ["REQ-12", "REQ-13"]
```

A test traces a requirement when the range added it (no test of that name in that file at the base commit) and its name or its doc comment carries the ID as whole words, in any case and with any separator: `test_req_12_parses`, `it("parses week dates #REQ-12")`, `/// Parses week dates (REQ-12).`, but not `test_req_120` or `testReq12`. The doc comment is the language's: Rust's `///` lines or `/** */` above the test's attributes, Go's `//` lines above the function, a Python docstring, a JSDoc or Javadoc `/** */` block above the test, and Clojure's `;` lines above `deftest`. A test's body is not read. A test that existed before the change passed without it, so it cannot show the change meets the requirement; the finding lists such tests with `added: false`.

A `REQ-<n>` marker (`req` followed by a number as its own word, as in `#REQ-7`, `req_7`, `TestParse_REQ_7`) on a test the range added is a requirement too, when no listed ID matches it: the test traces it, and the check lists it as `spec.unlisted-requirement` (`info`, so no gate fails on it), whose evidence says `source: marker` where a listed requirement's says `source: config`. So a repository that writes requirement IDs only into its tests gets them traced without configuration, and one that lists them hears of a marker the task does not name (often a mistyped ID). Markers on tests that existed before the change add nothing. With no requirement listed or marked the check passes with a note. The gate keys are `meta.traceability` (none untested) and `meta.traceability.untested`.

`meta.oracle-independence` asks who wrote the evidence. Under a range, with a per-test coverage map (made at `--depth standard` for Rust, Go, and Python; see [Coverage](#coverage)), it finds the tests that run each changed function. A function a test from before the change runs is `independent`; one only tests the range added run is `evidence.self-authored-only`: worth checking those tests' expectations against the task rather than the code. Without a map, it labels nothing and says why. A registered oracle's independence is its verification: a result against an unverified oracle is already capped at tier 3 with an `oracle.unverified` finding (see [`cupel oracle`](#cupel-oracle)). The gate keys are `meta.oracle-independence.independent` and `self_authored_only`.

## Plugins

A plugin is a check or a fact producer in its own executable, in any language, that speaks JSON over standard input and output. Cupel registers it beside the built-in checks and producers, so `describe`, `--only`, `--dry-run`, gates, `[checks."<id>"]` settings, and the support matrix treat it like one of them.

Cupel finds plugins in two places on every run: the `[plugins]` table of `cupel.toml`, and any executable named `cupel-check-<name>` or `cupel-produce-<name>` on `PATH` (the first directory wins, as in a shell). `--no-path-plugins` skips `PATH`.

```toml
[plugins]
untested = ["python3", "tools/untested.py"]   # a program and its arguments
per-test = "tools/cupel-produce-per-test"      # one executable; a path with "/" is relative to the root
```

Each plugin runs in the repository root as `<plugin> describe`, `<plugin> plan`, and `<plugin> run`, the last two with a request on standard input: the files in scope, the plugin's settings, the run's time left, a scratch directory under `.cupel/work/`, and a JSON file for each fact it declared. `cupel describe schema plugin` is the schema of every message.

- **A check plugin** describes itself with the manifest a built-in check has (`cupel describe <check>`), plus `"kind": "check"`: its ID, tier, cost, the facts it needs and wants, its finding kinds with their evidence schemas, and its gate metrics. `run` prints `finding` lines and one `result`.
- **A producer plugin** describes `"kind": "producer"`, an ID starting with `plugin.`, the fact kind and version it provides, its languages, and its needs. `run` prints one `fact` line. Producers are chosen the same way for plugins and built-ins (a pin in `[lang.<id>]`, `runner` for test results and `mutator` for mutation outcomes, then the first producer that is ready); plugins come after the built-in producers, so a plugin fills a gap or stands in for a missing tool rather than displacing a built-in producer that works.

Cupel holds plugins to the protocol. A plugin that cannot describe itself in 10 seconds with a manifest Cupel can register stops the run with exit 2, naming the plugin and the fix. A finding of an undeclared kind, evidence its schema rejects, an undeclared metric, a fact of another kind, version, or shape, or a non-zero exit without a `result` or `fact` makes the check `error`, with the reason in its `notes`, and nothing the plugin printed reaches the envelope. When `--timeout` or `--budget` runs out, the plugin gets SIGTERM, then SIGKILL two seconds later.

### The PIT plugin

[`plugins/pit/cupel_produce_pit.py`](plugins/pit/cupel_produce_pit.py) is a producer plugin that makes `mutation-outcomes` for Java with [PIT](https://pitest.org): it copies the project into its scratch directory, runs PIT there (Maven: the `pitest-maven` plugin with `pitest-junit5-plugin` declared in the `pom.xml`; Gradle: the `info.solidsoft.pitest` plugin with XML output and untimestamped reports), and places each PIT mutation on the Cupel mutation site of its line and operator, with the tests that killed it. `mutator` pins it over the built-in engine:

```toml
[plugins]
pit = ["python3", "plugins/pit/cupel_produce_pit.py"]

[lang.java]
mutator = "plugin.pit"
pit = "mvn -B -q test-compile org.pitest:pitest-maven:mutationCoverage -DoutputFormats=XML -DtimestampedReports=false -DfullMutationMatrix=true"   # the default for Maven
```

`pit_report` names where the command leaves `mutations.xml`, and `pit_report_ready = true` reads a report already in the tree instead of running PIT.

### Reference plugins

Reference plugins in Rust, Python, and TypeScript live in [`crates/cupel-fixtures/plugins/`](crates/cupel-fixtures/plugins/): a producer that reads a `per-test-map` for JavaScript and TypeScript from an LCOV file with one `TN:` section per test, and a check that reports the files in scope no test executes. With the Python pair in `[plugins]` of a TypeScript repository, `cupel describe fact per-test-map` lists it beside the built-in producers, `cupel tests --impacted` works for TypeScript, and the check runs:

```text
$ cupel check --only ref.untested-python --format text
cupel check: pass (exit 0)

summary   1 finding · fail 0 · warn 1 · info 0 · new 0 · resolved 0 · suppressed 0

checks
  ref.untested-python  fail     tier 2 · quick · score 0.6666666666666666 tested_files (n 3) · metrics untested 1 · 1 finding · 100 ms

warn (1)
  ref.untested-python
    No test executes src/report.ts
      fnd_2d9067 · ref.untested-file · tier 2 · confidence 1
      at src/report.ts:1
      evidence: file "src/report.ts" (full: cupel explain fnd_2d9067)
      action write_test (priority 1): Write a test that executes src/report.ts.
```

## Configuration

`cupel.toml` is found by walking up from `--cwd`. Precedence is flags, then `CUPEL_*` environment variables (`CUPEL_DEPTH`, `CUPEL_FORMAT`, `CUPEL_PROFILE`), then `--profile`, then `cupel.toml`, then defaults. `CUPEL_NO_DAEMON=1` makes an invocation run in its own process even when a daemon serves the repository (see `cupel serve`). The envelope echoes the effective configuration and the layers that contributed under `run.config`. Unknown keys are usage errors (exit 2) that list the valid keys, and `cupel describe schema config` is the full schema.

```toml
required_version = ">=0.1"         # a Cargo version requirement this binary must satisfy
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
differential_entries = ["src/money.rs:split"]   # what gen.differential compares
advisories = "advisories"          # OSV advisories perf.footprint matches dependencies against
race = "tsan"                      # the race detector conc.race uses; Miri by default
history_command = "cargo test --test history"   # what records conc.linearizability's histories

[lang.javascript]
bench_commands = ["node bench.js"] # what perf.bench times with hyperfine

[checks."static.complexity"]       # check settings, for every language
cyclomatic_max = 10

[checks."static.complexity".lang.go]   # the same settings, overridden for one language
cyclomatic_max = 15

[checks."static.deps".layers]      # lower layers may not import higher ones
order = ["domain", "app", "infra"]
map = { domain = "src/domain/**", app = "src/app/**", infra = "src/infra/**" }

[checks."meta.traceability"]       # the task's requirement IDs, traced by the names and docs of added tests
requirements = ["REQ-12", "REQ-13"]

[oracles.release]                  # what checks compare against; see cupel oracle
kind = "ref"
ref = "v1.2.0"

[gate.changed]                     # evaluated for --changed and --base scopes
"tests.weakening" = "none"
"static.api-surface" = "no-breaking"
"static.complexity" = "none"
"static.complexity.cyclomatic_max" = "<= 10"
"static.deps.cycles" = "== 0"

[gate.tree]                        # evaluated for whole-tree and --paths scopes
"static.clones.pairs" = "<= base"  # no more than the baseline run had

[profile.loop]                     # cupel check --profile loop
depth = "quick"
```

**`required_version`** is a Cargo version requirement (`>=0.4`, `^0.4`, `~0.4.1`, comma-joined ranges) on the `cupel` binary. When the binary does not satisfy it, `doctor` reports `doctor.version-unsatisfied` with the required and actual versions and an `install_tool` action naming the [install routes](#install) available now, and every verb that reads or grades the repository (all but `describe`, `doctor`, `init`, `suppress`, and `serve`) refuses with that finding and exit 3; a requirement that does not parse is `doctor.version-invalid`, exit 2 outside `doctor`. A suppressed version finding (`cupel suppress`) refuses nothing.

**Check settings** live under `[checks."<id>"]`, and `[checks."<id>".lang.<lang>]` overrides any of them for one language. Each check declares its settings in its manifest, and `cupel describe <check>` lists them with their defaults; a key a check does not declare is a configuration error that lists the ones it does. The checks that take settings:

<!-- generated:check-settings -->
| Check | Setting | Default | What it sets |
| --- | --- | --- | --- |
| `meta.traceability` | `requirements` | `[]` | The task's requirement IDs, such as `REQ-12`; a per-language override assigns them to the language whose tests trace them. |
| `perf.bench` | `threshold_percent` | `5` | The smallest change of a benchmark's median, in percent of the base's, that is reported. |
| `perf.bench` | `significance_percent` | `5` | The Mann-Whitney U test's level, in percent: a change with a lower p is significant. |
| `static.clones` | `threshold` | `0.82` | The least Jaccard similarity of two symbols' fingerprint sets that makes a clone, greater than 0 and at most 1. |
| `static.clones` | `min_lines` | `4` | The fewest lines a symbol must span to be compared. |
| `static.clones` | `min_nodes` | `20` | The fewest normalized-tree nodes a symbol must have to be compared. |
| `static.complexity` | `cyclomatic_max` | `10` | The highest cyclomatic complexity a symbol may have; above it is complexity.high. |
| `static.complexity` | `cognitive_max` | `15` | The highest cognitive complexity a symbol may have; above it is complexity.high. |
| `static.complexity` | `nesting_max` | `4` | The deepest control-flow nesting a symbol may have; deeper is nesting.deep. |
| `static.complexity` | `params_max` | `5` | The most parameters a symbol may declare, not counting a receiver; more is params.many. |
| `static.complexity` | `length_max` | `60` | The most lines a symbol may span; more is function.long. |
| `static.crap` | `high` | `30` | The CRAP score at or above which a symbol is crap.high. |
| `static.crap` | `watch` | `5` | The CRAP score at or above which a symbol below high is crap.watch. |
| `static.deps` | `distance_max` | `0.7` | The most A + I − 1 an abstract module may reach before dep.unstable-abstract, from 0 to 1. |
| `static.deps` | `layers` | — | Layers as order (names, lowest first) and map (a glob per layer); a module importing a higher layer is dep.layer-violation. No layering rule when unset. |
| `tests.assertion-density` | `lines_per_assertion_max` | `20` | The most lines per assertion a test may span before density.sparse-test. |
| `tests.assertion-density` | `file_density_percent_min` | `50` | The least assertions per test a file may make, in percent of the repository's, before density.sparse-file. |
| `tests.assertion-density` | `file_tests_min` | `3` | The fewest tests a file needs to be compared with the repository. |
| `tests.quality` | `mock_density_max` | `50` | The highest share, in percent, of doubles among a test's doubles and assertions before test.mock-heavy. |
| `tests.quality` | `mocks_min` | `2` | The fewest test doubles that can make a test test.mock-heavy. |
| `tests.quality` | `logic_max` | `0` | The most loops, alternatives, and unguarded branches a test may have before test.logic. |
| `tests.quality` | `setup_statements_min` | `3` | The fewest leading statements tests in one file must share to be test.duplicate-setup. |
<!-- end:check-settings -->

**Gate rules** map a key to a condition. The key is a check ID or `<check-id>.<metric>`, where the metric is one of that check's gate metrics in the check table under Checks. The condition is `none` (the check reported no unsuppressed `warn` or `fail` finding), `no-breaking` (no unsuppressed breaking finding, a kind ending in `.breaking`, such as `static.api-surface`'s `api.breaking`; any other check is rejected when the config loads), or a comparison `>= > <= < ==` against a number or against `base`; a number ending in `%` is divided by 100. `base` is the metric's value in the reference run, optionally with an offset in the metric's unit (`"<= base"`, `">= base - 0.02"`, `"< base + 3"`): the run `--against` names; otherwise, for `--changed` and `--base`, the base recomputed, by running the rule's checks over the base commit's versions of the changed files in a temporary copy; otherwise the `default` baseline (`cupel baseline set --write`). A rule passes only when its check ran to completion and reported the metric; a rule on a check that did not run is `skipped`, as is a rule against `base` with no reference value (no baseline is set, or every changed file is new), and any rule that does not pass fails the gate. Each rule in `summary.gate` carries `actual`, the `bound` it was compared with, and for a skipped rule the `reason`. `[gate.changed]` applies to `--changed` and `--base` scopes and `[gate.tree]` to everything else. A `[gate.tree]` rule on `tests.weakening`, `static.api-surface`, `meta.traceability`, or `meta.oracle-independence`, which need a range, is rejected when the config loads.

Cupel writes only under `.cupel/` (run records, the SQLite fact store, `cache/` with coverage reports and per-test maps, scratch space) unless `--write` is given. An install you allow (`--install`, or yes to the question) runs your package manager, which for the JavaScript launchers, uv, and poetry adds a dev dependency to `package.json` or `pyproject.toml` and its lockfile. `.cupel/.gitignore` excludes everything there except `suppressions.toml`.

## For agents

An agent can learn Cupel from Cupel. `cupel describe --all` is one JSON document with every verb, flag, exit code, and schema. `cupel describe <check>` and `cupel describe finding <kind>` explain any check or finding the agent meets. Every verb accepts `--describe`, and `--dry-run` shows exactly what a command would do before it does anything.

The loop is the one in [The agent loop](#the-agent-loop): run the gate, take `next_actions[0]`, fix it, confirm with the finding's `repro`, and repeat until the gate exits 0. An agent should parse only stdout, branch on `exit_code` and `status`, and treat exit 2 as a mistake in its own invocation, whose envelope names the fix.

`cupel describe --agent --format text` prints a guide of about sixty lines to paste into `CLAUDE.md` or `AGENTS.md`; `--format json` carries the same Markdown as the envelope's `description.markdown`. Its exit codes, flags, depths, checks, statuses, MCP tools, and verbs come from the binary's registries. This is its output:

<!-- generated:agent-guide -->
```markdown
# Cupel

Cupel grades what a coding agent produced. It measures structure and test strength over a change or a whole tree and returns one JSON envelope: findings with stable IDs, ranked `next_actions`, and a gate verdict. It never edits code; you fix, it grades.

## The loop

1. Run `cupel check --changed --format json`.
2. If `exit_code` is 0, stop: the gate passed, or there is no gate.
3. Otherwise take `next_actions[0]`, fix what it names, and rerun its finding's `repro` to confirm.
4. Go back to 1. Do not stop until it exits 0.

- `cupel explain <finding-id>` shows a finding's source, covering tests, and a test skeleton.
- `cupel tests --impacted --changed` lists the tests the change affects and the command that runs them.
- `cupel describe <check-id>` and `cupel describe finding <kind>` explain any check or finding kind.
- `--dry-run` shows what any command would run or write; `<verb> --describe` lists its flags.

## Exit codes

| Code | Name | Meaning |
| --- | --- | --- |
| 0 | pass | Ran fully; gate passed or no gate. |
| 1 | fail | Ran fully; gate failed. |
| 2 | usage | Bad flags or config; the envelope names the fix. |
| 3 | environment | Missing tool, oracle, or toolchain; doctor findings attached. |
| 4 | unevaluable | Baseline test suite fails or build broken; scoring refused. |
| 5 | partial | Timeout or budget hit; the envelope has valid partial results. |
| 6 | internal | Cupel bug; the envelope includes a bug-report payload. |

Exit 2 is a mistake in your command line or `cupel.toml`; its finding names the fix. On exit 3 run `cupel doctor` and follow its `install_tool` and `configure` actions. On exit 4 make the test suite pass first.

## The gate

`[gate.changed]` in `cupel.toml` applies to `--changed` and `--base` scopes, `[gate.tree]` to the rest. A rule maps a check, or `<check-id>.<metric>`, to `none`, `no-breaking`, or a comparison such as `"static.complexity.cyclomatic_max" = "<= 10"` or `"tests.weakening" = "none"`; `"static.clones.pairs" = "<= base"` compares with the baseline, or with a range's base (`--against baseline|REF|run:ID` names another reference). `--gate` is the default when the table exists and `--no-gate` reports without a verdict. A rule on a check that did not run is skipped, and every rule that does not pass fails the gate (exit 1); `summary.gate` has each rule's result. Do not loosen the gate or suppress findings to pass it unless asked.

## Depth

- `--depth quick|standard|deep|exhaustive`, default `standard`. How much evidence to gather.
- `--budget DURATION`. A wall-clock budget: run the most valuable checks that fit and skip the rest.
- Scope flags (`--changed`, `--base` with `--head`, `--paths`, `--symbols`) intersect; under `--changed` or `--base` the checks rule on the functions the change touched. `--only` and `--skip` select checks by ID or group, `--tier N+` by tier; `--jobs N` caps the threads and test processes.
- `--oracle NAME` picks the reference implementation `gen.differential` compares with and `--cases N` the inputs generative checks try; `check --scaffold --write` writes a property stub for each `property.missing` finding.

| Depth | Adds |
| --- | --- |
| `quick` | `meta.oracle-independence`, `perf.footprint`, `static.api-surface`, `static.clones`, `static.complexity`, `static.crap`, `static.dead-code`, `static.deps`, `tests.assertion-density`, `tests.quality`, `tests.weakening` |
| `standard` | `conc.race`, `gen.property`, `meta.traceability`, `spec.contracts`, `static.analyze`, `tests.coverage`, `tests.flaky`, `tests.mutation` |
| `deep` | `conc.linearizability`, `gen.differential`, `gen.fuzz`, `gen.metamorphic`, `perf.bench` |
| `exhaustive` | `conc.dst`, `conc.model-check`, `spec.prove`, `spec.trace` |

## The envelope

Stdout is one JSON envelope; diagnostics go to stderr. Read:

- `status`: `pass`, `fail`, `partial`, `error`, `plan`; `exit_code` always equals the process's.
- `findings[]`: `id`, `check`, `kind`, `severity`, `location`, `title`, `action`, and `repro`, the command that reruns only that check on only that symbol.
- `next_actions[]`: what to fix, most important first; each has a `kind`, a `summary`, and its `finding`.
- `summary.gate`: the verdict and each rule's result; `truncated` says `--limit` cut the findings.

## MCP

A client that calls tools can run `cupel serve --mcp` (JSON-RPC on stdio) instead of the commands. Its tools are `check`, `survey`, `explain`, `tests_impacted`, `describe`; each takes its verb's flags as arguments and returns the envelope the command prints. Without `--mcp`, `cupel serve` keeps facts warm for every command run in the repository, and `--watch` reparses files as they change.

## More

The verbs are `init`, `doctor`, `describe`, `check`, `survey`, `explain`, `baseline`, `oracle`, `tests`, `suppress`, `serve`. `cupel describe --all` is every verb, flag, exit code, and schema as one JSON document.
```
<!-- end:agent-guide -->

## Development

All automation goes through `make`.

`make dev-setup` installs the pinned tool environment the tests, goldens, and gates need: every runner, coverage tool, analyzer, fuzzer, JVM build tool, and verifier CI installs, at CI's versions, under `~/.cache/cupel/dev-tools` (or `CUPEL_DEV_TOOLS`); a second run only verifies them. `make check` and every other target run with those tools first on `PATH`, and `eval "$(make -s dev-env)"` gives a shell that runs cargo directly the same environment ([ADR-139](docs/adr/CUPEL_DECISION_139.md)).

| Target | Does |
|---|---|
| `make dev-setup` | install or verify the pinned development tools and print each one's version |
| `make check` | fmt, clippy with `-D warnings`, every test, rustdoc with `-D warnings`, and a check that the build left the tree clean; the pre-commit bar |
| `make test` | `cargo test --workspace`; `UPDATE_GOLDENS=1 make test` rewrites golden envelopes |
| `make lint`, `make fmt` | `cargo fmt --check` and clippy; `cargo fmt` |
| `make build`, `make doc` | build the workspace; build its rustdoc |
| `make matrix` | print the check × language support matrix |
| `make readme` | regenerate this README's verb, global-flag, exit-code, check, check-settings, and support-matrix tables and its agent guide from `cupel describe`, and its MCP tool table from `cupel serve --mcp` |
| `make parity` | every golden-harness run again through `cupel serve` on a copy of its directory, which must print the same after redaction ([ADR-085](docs/adr/CUPEL_DECISION_085.md)), and each run of a verb an MCP tool serves again as that tool's call to `cupel serve --mcp` on another copy, which must return the same envelope and stderr ([ADR-121](docs/adr/CUPEL_DECISION_121.md)) |
| `make baseline` | after a green tree dogfood, sets the store's baseline to that run and exports it to `.cupel/baseline.json`, the committed reference the tree gate's `base` rules compare with ([ADR-140](docs/adr/CUPEL_DECISION_140.md)) |
| `make dogfood` | Cupel grades itself at `--depth deep`: `cupel check --changed --depth deep --gate` against `main`, which mutates every function of every changed file; `make dogfood DOGFOOD_SCOPE=tree` grades the whole tree with `[gate.tree]`, whose clone ratchets read `<= base` against the committed `.cupel/baseline.json` it imports first, and skips `tests.mutation`. Both skip `tests.flaky`, whose engine reruns the whole suite one test at a time and does not rerun cargo-nextest, and `conc.race`, which Miri cannot run on tests that spawn processes ([ADR-129](docs/adr/CUPEL_DECISION_129.md)). A local run is bounded by `--budget 30m` (`DOGFOOD_BUDGET`), lists what the budget left out, and exits 5 when it left anything out; CI runs both with `DOGFOOD_BUDGET=`, which runs every check. They need cargo-nextest and cargo-llvm-cov on `PATH`; one dogfood runs at a time on a machine |
| `make install` | `cargo install --path crates/cupel-cli` |
| `make dist` | build the stripped release binary and package it as `target/dist/cupel-<version>-<target>.tar.gz` (`.zip` on Windows) with `LICENSE` and this README; `TARGET=<triple>` builds another target, the Linux musl targets through `cargo-zigbuild` with `zig` on PATH ([ADR-124](docs/adr/CUPEL_DECISION_124.md)) |
| `make version V=X.Y.Z` | write the release version into every file that names it: the workspace `Cargo.toml`, the workspace crates in `Cargo.lock`, `dist/npm/package.json`, and `dist/pipx/cupel.py`; a test holds them equal |
| `make installers` | the tests of `dist/`'s installers against a fake release on a local HTTP server ([ADR-125](docs/adr/CUPEL_DECISION_125.md)); `make test` runs them too, skipping a test whose tool (ShellCheck, Ruby, Node, npm, Python) is missing |

The generated README sections sit between `<!-- generated:NAME -->` and `<!-- end:NAME -->` markers. A test in `crates/cupel-cli/tests/readme.rs` renders them from the binary's `describe` output and fails when the committed text differs, so `make check` catches a stale table. Run `make readme` and review the diff.

CI runs `make check` and `make parity` on Linux and macOS and `make dogfood` on every push: the tree gate on `main` and the changed gate on branches, both at `--depth deep`, on a Linux runner with cargo-nextest and cargo-llvm-cov installed. It runs `make installers` on Linux and macOS images with only Rust, Node, Python, and ShellCheck added, and Homebrew's `brew style` on the rendered formula on macOS. A `v*` tag runs `.github/workflows/release.yml`, which builds each platform's archive with `make dist`, writes `SHA256SUMS`, and publishes the GitHub Release; the tag must be `v` followed by the crate version, and a manual run of the workflow builds and uploads the archives without releasing. Cupel's own `cupel.toml` is the example to copy.

The workspace is a set of crates under `crates/`. `cupel-core` holds the output contract, the fact vocabulary, scope, the scheduler, and gates. `cupel-lang` holds the `Language` trait, the query-driven producers, and one `langs/<id>/` directory per language. `cupel-adapt` is the toolchain table of runners and coverage and bench tools per language. `cupel-mutate` is the mutation engine. `cupel-checks` holds the checks, which depend on `cupel-core` alone. `cupel-store` is the SQLite fact store. `cupel-serve` is the daemon's socket protocol, server, client, and file watcher, and the MCP server, generic over what they run. `cupel-cli` is the binary, and the daemon runs requests through the same code a cold invocation does. `cupel-fixtures` holds fixture repositories, conformance inventories, and golden envelopes. Every behavior is built test-first; the implementation plan has the definition of done for each step.

Checks and tool producers register themselves from their own modules ([ADR-074](docs/adr/CUPEL_DECISION_074.md)), so additions made in parallel never edit the same lines. Adding a check is one module in `crates/cupel-checks/src/` that submits itself with `inventory::submit!`, its `mod` line in that crate's `lib.rs`, its fixture code, and its goldens under `crates/cupel-fixtures/golden/<check-id>/` (the matrix golden per language, `describe.json`, and `survey-<lang>.json`); a check can keep its fixture code in its own `crates/cupel-fixtures/repos/<check-id>/<lang>/` so no other check's goldens change. Adding a tool producer is one module in `crates/cupel-adapt/src/` and its `mod` line. Adding a fact kind ([ADR-123](docs/adr/CUPEL_DECISION_123.md)) is its payload module in `crates/cupel-core/src/`, which submits the kind's catalog entry with `inventory::submit!`, its `mod` and `pub use` lines in that crate's `lib.rs`, its variant appended to `FactKind` in `crates/cupel-core/src/fact/kind.rs`, its conformance inventory in `crates/cupel-fixtures/conformance/` if it has one, and its schema snapshot, which `UPDATE_GOLDENS=1 INSTA_UPDATE=always make test` writes. Tests derive what must be described, golden, and supported from the registries, so nothing else lists checks; `UPDATE_GOLDENS=1 make test` writes a new check's goldens and `make readme` adds its rows here.

## License

BSD 3-Clause. See [LICENSE](LICENSE).
