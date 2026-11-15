# Cupel

[![CI](https://github.com/tysont/cupel/actions/workflows/ci.yml/badge.svg)](https://github.com/tysont/cupel/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/tysont/cupel)](https://github.com/tysont/cupel/releases/latest)
[![License](https://img.shields.io/github/license/tysont/cupel)](LICENSE)

Cupel grades the code and tests a coding agent wrote. It runs [checks](#checks) over a change or a whole tree, from complexity, clones, and dead code through coverage, mutation testing, and tests weakened on the branch, in seven languages, and prints one JSON envelope: findings with stable IDs, `next_actions` ranked for an agent to work through, and a gate verdict in the exit code. An agent's own report that the work is done is not evidence; a deleted assertion, an untested function, or a copied block all pass a green test suite. Cupel measures those, so an agent can be told not to stop until `cupel check --changed --gate` exits 0. It never edits code: the agent fixes, Cupel grades. (A cupel is the porous cup used in fire assay to separate precious metal from base.)

## Quickstart

Install the latest release into `~/.local/bin` (Linux and macOS; [every route](docs/guide/install.md)):

```sh
curl -fsSL https://raw.githubusercontent.com/tysont/cupel/main/dist/install.sh | sh
```

<!-- captured by scripts/readme-capture.sh with cupel 0.2.9 built from batch-0.3.0 at a64a6303, on hashicorp/golang-lru at 2bb29091 (main) with one change on agent/add-all; … marks a trim -->
```console
$ cupel --version
cupel 0.2.9
```

In a repository, `cupel init` detects the languages and their test and coverage commands and shows the `cupel.toml` it would write, starter gate included; `cupel init --write` writes it (`cupel doctor` then checks the tools it names are installed):

```console
$ cupel init
cupel init: plan (exit 0)

plan      est 0 ms
would write
  cupel.toml
    --- /dev/null
    +++ b/cupel.toml
    @@ -0,0 +1,20 @@
    +# Cupel configuration, written by `cupel init`; `cupel describe schema config` documents every key.
    +default_branch = "main"
    +depth = "standard"
    +
    +[lang.go]
    +test = "go test -json ./..."
    +coverage = "go test -coverprofile={out} ./..."
    +bench = "go test -run=^$ -bench=. ./..."
    +
    +[gate.changed]
    …
next actions
  1. configure: Nothing was written; run `cupel init --write` to write these files.
     $ cupel init --write

$ cupel init --write
cupel init: pass (exit 0)
…
```

`cupel survey` grades the whole tree and ranks what to work on, protect-first: code with the weakest tests before code that is merely complex. It never gates, so it exits 0 whatever it finds.

```console
$ cupel survey
cupel survey: pass (exit 0)

summary   175 findings · fail 55 · warn 43 · info 77 · suppressed 0 · new and resolved need a baseline (`cupel baseline set`)

checks
  …
  static.clones             fail     tier 0 · quick · score 48 pairs (n 150) · … · 26 findings · 439 ms
  static.complexity         fail     tier 0 · quick · score 27 cyclomatic_max (n 168) · … · 24 findings · 26 ms
  …
  tests.coverage            fail     tier 2 · standard · score 0.9202279202279202 line_ratio (n 702) · … · 2 findings · 405 ms · …
  tests.flaky               pass     tier 2 · standard · score 0 unstable (n 48) · … · 0 findings · 4383 ms
  tests.mutation            fail     tier 3 · standard · score 0.5672268907563025 killed_ratio (n 238) · …
  …
  targets  68 recommended, protect first
     …
     2. protect  lru.Cache.RemoveOldest  mutation 0 · coverage 0 · tests 0  go · lru.go:204:1-218:2 · sh_998421c3989fc3ac
        static.complexity.cyclomatic 5 (#56) · static.complexity.cognitive 4 (#68) · static.crap.crap 30 (#1) · tests.coverage.uncovered_lines 13 (#1) · …
     3. both  arc.ARCCache.Add  mutation 0 · coverage - · tests 0  go · arc/arc.go:88:1-179:2 · sh_3f3d786cad558232
        static.complexity.cyclomatic 14 (#11) · static.complexity.cognitive 23 (#4) · … · tests.mutation.mutation_score 0 (#1) · …
…
next actions
  1. strengthen_assertion: Replace the truthiness, presence, or inequality checks in `TestOnEvictReentrant` with an assertion that pins the expected value. (finding fnd_e8115d)
  2. write_test: Write a test that calls `RemoveOldest`. (finding fnd_cde7cb)
  3. write_test: Write a test that runs line 110 of arc/arc.go. (finding fnd_552970)
  …
```

On a branch, `cupel check --changed --gate` grades only what the branch changed. Here an agent added a method no test calls and deleted one assertion from an existing test; the suite still passes, and the gate does not:

```console
$ cupel check --changed --gate
cupel check: fail (exit 1)

gate      fail (gate.changed)
  fail  tests.weakening none  actual 1
  fail  tests.mutation.score >= base - 0.02  actual 0  bound 0.98
  pass  static.complexity.cyclomatic_max <= base  actual 6 (lru.TestLRUContainsOrAdd at lru_test.go:181)  bound 9 (lru.TestLRUContainsOrAdd at lru_test.go:181)  tolerance 2
  pass  static.clones.pairs <= base  actual 0  bound 0  tolerance 0
  pass  static.api-surface no-breaking  actual 0

summary   8 findings · fail 2 · warn 2 · info 4 · suppressed 0 · new and resolved need a baseline (`cupel baseline set`)
…
fail (2)
  tests.weakening
    `TestLRUContainsOrAdd` makes 5 assertions, down from 6
      fnd_400a22 · assertion.removed · tier 2 · confidence 1
      at lru_test.go:181:1-205:2
      evidence: base 6 · head 5 · test "TestLRUContainsOrAdd" (full: cupel explain fnd_400a22)
      action restore_test (priority 1): Restore the assertions removed from `TestLRUContainsOrAdd` (see it at e28b6a599275113972f81b47dbfa6436780d0f5c); if one is wrong, replace it with a correct one rather than dropping it.
      repro: cupel check --only tests.weakening --base e28b6a599275113972f81b47dbfa6436780d0f5c
      suppress: cupel suppress fnd_400a22 --reason '…'
  static.analyze
    staticcheck SA4006: this value of evict is never used
      fnd_b8f153 · analyze.staticcheck.SA4006 · tier 1 · confidence 1
      …
warn (2)
  static.crap
    `lru.Cache.AddAll` has CRAP 30.0 (high at 30): cyclomatic 5 with 0% coverage
      …
  tests.coverage
    `lru.Cache.AddAll` never ran under test: none of its 11 instrumented lines
      …
info (4)
  …
  tests.mutation
    No test runs lru.go:147; its mutant `>=` → `>` was not run
      …
next actions
  1. restore_test: Restore the assertions removed from `TestLRUContainsOrAdd` (see it at e28b6a599275113972f81b47dbfa6436780d0f5c); if one is wrong, replace it with a correct one rather than dropping it. (finding fnd_400a22)
  2. fix_bug: Fix what staticcheck reports at lru_test.go:198 (SA4006), or configure staticcheck if the rule does not apply to this project. (finding fnd_b8f153)
  3. write_test: Write a test that calls `AddAll`. (finding fnd_eec201)
  …
```

## Using Cupel with an agent

**Instructions.** Add to `CLAUDE.md` or `AGENTS.md`:

```markdown
## Done means graded
Before you stop, run `cupel check --changed --gate --format json`. While it exits 1, fix
`next_actions[0]`, confirm with its finding's `repro`, and run it again. Never loosen the
gate or suppress a finding to pass. Run `cupel describe --agent` once for the loop, the
exit codes, and the envelope.
```

`cupel describe --agent` prints a guide of about sixty lines written for an agent ([all of it](docs/guide/agents.md)). It begins:

<!-- generated:agent-loop -->
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

…
```
<!-- end:agent-loop -->

**A Stop hook.** In Claude Code, a `Stop` hook that exits 2 keeps Claude working and hands it the hook's stderr ([hooks reference](https://code.claude.com/docs/en/hooks)). This `.claude/settings.json` blocks stopping exactly while the gate fails (exit 1); any other exit, such as a missing tool (3) or a red suite (4), lets Claude stop and report it:

```json
{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "cupel check --changed --gate --format text >&2; [ $? -ne 1 ] || exit 2"
          }
        ]
      }
    ]
  }
}
```

Run on the branch above, the hook exits 2 with the gate's report:

```console
$ sh -c 'cupel check --changed --gate --format text >&2; [ $? -ne 1 ] || exit 2'
cupel check: fail (exit 1)

gate      fail (gate.changed)
  fail  tests.weakening none  actual 1
  fail  tests.mutation.score >= base - 0.02  actual 0  bound 0.98
…
[exit 2]
```

Claude Code ends the turn anyway after eight consecutive blocks, and a hook that runs past its timeout (600 s by default) is cancelled; a long run can be bounded with `--budget`, whose `partial` result (exit 5) also lets Claude stop.

**MCP.** `cupel serve --mcp` is a Model Context Protocol server on stdio for clients that call tools rather than run commands. Each tool takes its verb's flags as arguments and returns the same envelope ([details](docs/guide/usage.md#mcp)):

```json
{ "mcpServers": { "cupel": { "command": "cupel", "args": ["serve", "--mcp", "--cwd", "/path/to/repo"] } } }
```

<!-- generated:mcp-tools -->
| Tool | Runs | What it does |
| --- | --- | --- |
| `check` | `cupel check` | Run checks over a scope at a depth and evaluate the gate. Returns the envelope `cupel check --format json` prints; its exit_code and status say how the run ended. |
| `survey` | `cupel survey` | Rank a whole tree without a baseline or gate: the worst targets per metric, protect-first. Returns the envelope `cupel survey --format json` prints; its exit_code and status say how the run ended. |
| `explain` | `cupel explain` | Give full context and remediation for one finding or check result. Returns the envelope `cupel explain --format json` prints; its exit_code and status say how the run ended. |
| `tests_impacted` | `cupel tests --impacted` | List the tests a scope impacts, why, and the command that runs them. Returns the envelope `cupel tests --impacted --format json` prints; its exit_code and status say how the run ended. |
| `describe` | `cupel describe` | Describe the CLI, a verb, a check, a finding kind, a language, a fact, the support matrix, or a schema. Returns the envelope `cupel describe --format json` prints; its exit_code and status say how the run ended. |
<!-- end:mcp-tools -->

## The output contract

Every verb prints one JSON envelope on stdout (`--format text` renders it for people; `jsonl` and `sarif` also exist), and its `exit_code` always equals the process's. A finding has a stable `id`, a `kind`, a `severity`, a `tier`, a `location`, an `action` for the agent, and a `repro` that reruns only its check on only its scope. `next_actions` orders the actions, most important first. From the check above, the first finding and the first action:

```json
{
  "cupel": { "version": "0.2.9", "schema": "https://tysont.github.io/cupel/schema/v1/run.json" },
  "status": "fail",
  "exit_code": 1,
  "findings": [
    {
      "id": "fnd_400a22",
      "check": "tests.weakening",
      "kind": "assertion.removed",
      "severity": "fail",
      "confidence": 1.0,
      "tier": 2,
      "symbol": null,
      "location": { "file": "lru_test.go", "line": 181, "col": 1, "end_line": 205, "end_col": 2 },
      "title": "`TestLRUContainsOrAdd` makes 5 assertions, down from 6",
      "detail": null,
      "evidence": { "base": 6, "head": 5, "test": "TestLRUContainsOrAdd" },
      "action": {
        "kind": "restore_test",
        "priority": 1,
        "summary": "Restore the assertions removed from `TestLRUContainsOrAdd` (see it at e28b6a599275113972f81b47dbfa6436780d0f5c); if one is wrong, replace it with a correct one rather than dropping it.",
        "hints": {}
      },
      "repro": "cupel check --only tests.weakening --base e28b6a599275113972f81b47dbfa6436780d0f5c",
      "suppress": "cupel suppress fnd_400a22 --reason '…'",
      "new_since_baseline": false,
      "suppressed": false
    }
  ],
  "next_actions": [
    {
      "priority": 1,
      "finding": "fnd_400a22",
      "kind": "restore_test",
      "summary": "Restore the assertions removed from `TestLRUContainsOrAdd` (see it at e28b6a599275113972f81b47dbfa6436780d0f5c); if one is wrong, replace it with a correct one rather than dropping it.",
      "command": null
    },
    {
      "priority": 2,
      "finding": "fnd_b8f153",
      "kind": "fix_bug",
      "summary": "Fix what staticcheck reports at lru_test.go:198 (SA4006), or configure staticcheck if the rule does not apply to this project.",
      "command": null
    }
  ],
  "…": "…"
}
```

`cupel describe schema run` and `cupel describe schema finding` are the full JSON Schemas, published under [`tysont.github.io/cupel/schema/v1/`](https://tysont.github.io/cupel/schema/v1/run.json).

## Checks

`Tier` is the strength of a check's evidence (below); `Cost` is the cheapest depth that runs it. `Gate metrics` are what a gate rule can name as `<check>.<metric>`; `Ranked by` is what `survey` ranks. What each check looks for: [docs/guide/checks.md](docs/guide/checks.md); the full language matrix: [docs/guide/languages.md](docs/guide/languages.md).

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
| `gen.property` | 4 | standard | `property.falsified`, `property.missing`, `property.cases-pinned` | `properties`, `falsified`, `missing` | — | javascript, python, rust, typescript |
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
| `tests.coverage` | 2 | standard | `coverage.uncovered-symbol`, `coverage.uncovered-branch`, `coverage.unloaded-file`, `coverage.doctest-only` | `line`, `branch`, `lines`, `branches`, `uncovered` | `uncovered_lines`, `line_coverage` |  |
| `tests.flaky` | 2 | standard | `test.flaky`, `test.order-dependent` | `flaky`, `order_dependent` | — | clojure, go, java, javascript, python, rust, typescript |
| `tests.mutation` | 3 | standard | `mutant.survived`, `mutant.timeout`, `mutant.out-of-memory`, `mutant.uncovered`, `mutant.equivalent?`, `mutant.slow-test-excluded`, `mutant.flaky-test-excluded` | `score`, `killed`, `survived`, `timeout`, `out_of_memory`, `uncovered`, `unviable`, `not_run`, `equivalent`, `executed`, `reused`, `schemata`, `fallback`, `plan_ms`, `sync_ms`, `rewrite_ms`, `build_ms`, `run_ms`, `validate_ms` | `survivors`, `mutation_score`, `tests` |  |
| `tests.quality` | 2 | quick | `test.weak-assertion`, `test.no-assertion`, `test.mock-heavy`, `test.logic`, `test.duplicate-setup` | `assertion_density`, `weak_ratio`, `no_assertion` | `mock_density` | clojure, go, java, javascript, python, rust, typescript |
| `tests.weakening` | 2 | quick | `assertion.removed`, `test.skipped`, `matcher.loosened`, `snapshot.rewritten`, `test.deleted` | `weakened` | — | clojure, go, java, javascript, python, rust, typescript |
<!-- end:check-table -->

## Depths and exit codes

`--depth` (default `standard`) chooses how much evidence to gather. Each depth runs every check of its cost and of the cheaper depths: `quick` reads the source and runs no tests, `standard` adds the checks that run the test suite (coverage, mutation testing, reruns for flakiness) and the analyzers, and `deep` and `exhaustive` add generated inputs, benchmarks, simulation, model checking, and proofs where a project has them. `--budget 10m` runs the most valuable checks that fit and reports the rest as skipped ([budgets](docs/guide/usage.md#cupel-check)).

<!-- generated:depths -->
| Depth | Checks run | Adds |
| --- | --- | --- |
| `quick` | 11 | `meta.oracle-independence`, `perf.footprint`, `static.api-surface`, `static.clones`, `static.complexity`, `static.crap`, `static.dead-code`, `static.deps`, `tests.assertion-density`, `tests.quality`, `tests.weakening` |
| `standard` | 19 | `conc.race`, `gen.property`, `meta.traceability`, `spec.contracts`, `static.analyze`, `tests.coverage`, `tests.flaky`, `tests.mutation` |
| `deep` | 24 | `conc.linearizability`, `gen.differential`, `gen.fuzz`, `gen.metamorphic`, `perf.bench` |
| `exhaustive` | 28 | `conc.dst`, `conc.model-check`, `spec.prove`, `spec.trace` |
<!-- end:depths -->

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

## Gates and the ratchet

A gate is a table of rules in `cupel.toml`: `[gate.changed]` for `--changed` and `--base`, `[gate.tree]` for the whole tree. A rule maps a check, or `<check>.<metric>`, to `none` (no unsuppressed `warn` or `fail` finding), `no-breaking`, or a comparison against a number or against `base`. A rule against `base` is a ratchet: on a branch, `base` is the metric recomputed on the merge base's version of the changed code, so a change may not make it worse. `init` writes this starter gate:

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

`>= base - 0.02` and the complexity rule's watch threshold leave room for noise; a rule against `base` with nothing to compare (every changed file is new) is skipped with an `info` finding instead of failing. For the tree, `cupel baseline set --write` stores a passing run on the default branch as the reference. Every rule, tolerance, and the baseline's lifecycle: [docs/guide/configuration.md](docs/guide/configuration.md) and [docs/guide/usage.md](docs/guide/usage.md#cupel-baseline).

## Evidence tiers

Every check result and finding carries a tier, the kind of evidence behind it: 0 is structure read from the source (complexity, clones, dependencies), 1 a type checker or linter, 2 the tests' own structure, coverage, and reruns, 3 mutation testing (tests that notice the code changing), 4 generated inputs or an independent oracle (property tests, fuzzing, differential testing, race detectors), 5 conformance to a specification (contracts, traces, linearizability, model checking), and 6 a proof. A gate passing at tier 0 says less than one passing at tier 3; `--tier 3+` selects only the stronger checks, and a result compared with an oracle that is not verified is capped at tier 3.

## Credit

Cupel is inspired by tools [Robert C. Martin (Uncle Bob)](https://github.com/unclebob) published, and keeps their core metrics:

- [crapper](https://github.com/unclebob/crapper): "CRAP scores for Clojure, Java, Go, TypeScript, Rust, and Python", from the formula `CRAP = CC² × (1 − coverage)³ + CC`. Cupel's `static.crap` computes the same formula.
- [dryer](https://github.com/unclebob/dryer): "finds candidate duplicate code" by the Jaccard similarity of normalized structural fingerprints, reporting pairs at 0.82 or more. Cupel's `static.clones` normalizes the same way and defaults to the same threshold.
- [mutator](https://github.com/unclebob/mutator): "Mutation testing for Clojure, Java, Go, TypeScript, Rust, and Python": it runs only sites a test covers, scores killed / (killed + survived), and on a later run reruns only survivors and changed functions. Cupel's `tests.mutation` also skips uncovered sites and reuses stored outcomes, but scores killed / (killed + survived + uncovered), so code no test runs counts against the score.
- [scrap](https://github.com/unclebob/scrap): "a structural quality analyzer for Speclj specs", aimed "at test code in the same way CRAP is aimed at production code". Cupel's `tests.quality` and `tests.assertion-density` grade tests in every language it reads.

What Cupel adds, each shown above: the other checks in the [check table](#checks), among them `tests.weakening`, which caught the deleted assertion; JavaScript as a seventh language; a [gate](#gates-and-the-ratchet) whose rules ratchet against the merge base; and [one envelope](#the-output-contract) with ranked `next_actions`, from the command line, a [Stop hook](#using-cupel-with-an-agent), or MCP. Findings keep their IDs when a function is renamed or moved ([baselines](docs/guide/usage.md#cupel-baseline)).

## Status

This README describes only what exists. Cupel is pre-1.0; the [release badge](https://github.com/tysont/cupel/releases/latest) names the latest release. `cupel describe` gives everything it describes a stability: the exit codes are `stable`, and verbs, flags, checks, and schemas are `experimental` until 1.0 ([ADR-018](docs/adr/CUPEL_DECISION_018.md)). The schemas carry their major version in their URL (`schema/v1/`). From `cupel describe`:

<!-- generated:stability -->
| Described | Stability |
| --- | --- |
| The CLI | 1 `experimental` |
| Verbs | 11 `experimental` |
| Global flags | 19 `experimental` |
| Exit codes | 7 `stable` |
| Checks | 28 `experimental` |
| Schemas | 5 `experimental` |
| Install routes | 9 `experimental` |
<!-- end:stability -->

**Platforms.** Each release publishes one archive per target in [`dist/targets.toml`](dist/targets.toml), with a `SHA256SUMS`:

<!-- generated:targets -->
| System | Arch | Target | Archive |
| --- | --- | --- | --- |
| Linux | x86_64 | `x86_64-unknown-linux-musl` | `cupel-{version}-x86_64-unknown-linux-musl.tar.gz` |
| Linux | aarch64 | `aarch64-unknown-linux-musl` | `cupel-{version}-aarch64-unknown-linux-musl.tar.gz` |
| macOS | x86_64 | `x86_64-apple-darwin` | `cupel-{version}-x86_64-apple-darwin.tar.gz` |
| macOS | aarch64 | `aarch64-apple-darwin` | `cupel-{version}-aarch64-apple-darwin.tar.gz` |
| Windows | x86_64 | `x86_64-pc-windows-msvc` | `cupel-{version}-x86_64-pc-windows-msvc.zip` |
<!-- end:targets -->

CI runs `make check` (every test, with every runner, coverage tool, and analyzer installed) and `make parity` on Ubuntu 24.04 and macOS, and Cupel's own gate at `--depth deep` on Ubuntu. Windows is compiled in CI (`cargo check` of every target) and runs only the tests that need no Unix tools: the daemon's server tests and the library tests of `cupel-core`, `cupel-checks`, `cupel-lang`, and `cupel-mutate`; the end-to-end suites do not run there ([limits on Windows](docs/guide/install.md)). The install routes, from `cupel describe`:

<!-- generated:install-routes -->
| Route | Command | Platforms | Available |
| --- | --- | --- | --- |
| Install script | `curl -fsSL https://raw.githubusercontent.com/tysont/cupel/main/dist/install.sh \| sh` | Linux, macOS | now |
| cargo-binstall | `cargo binstall --git https://github.com/tysont/cupel cupel-cli` | Linux, macOS | now |
| Release archive | `gh release download --repo tysont/cupel --pattern SHA256SUMS --pattern 'cupel-*-<target>.tar.gz'` | Linux, macOS | now |
| From source | `cargo install --locked --git https://github.com/tysont/cupel cupel-cli` | Linux, macOS | now |
| Homebrew | `brew install tysont/cupel/cupel` | Linux, macOS | once the release workflow pushes the formula to the `tysont/homebrew-cupel` tap |
| npm | `npx cupel-cli` | Linux, macOS | once the `cupel-cli` package is published to npm |
| pipx | `pipx install cupel-cli` | Linux, macOS | once the `cupel-cli` package is published to PyPI |
| crates.io | `cargo install --locked cupel-cli` | Linux, macOS | once the library crates `cupel-cli` depends on are published to crates.io; `cupel-core` there is another project's |
| cupel.dev | `curl -fsSL https://cupel.dev/install \| sh` | Linux, macOS | once cupel.dev serves the install script |
<!-- end:install-routes -->

## Documentation

- [Install](docs/guide/install.md) · [Usage](docs/guide/usage.md) (every verb, with examples) · [Configuration](docs/guide/configuration.md) · [Using Cupel with an agent](docs/guide/agents.md)
- [Checks](docs/guide/checks.md) · [Languages and test runners](docs/guide/languages.md) · [Coverage and mutation testing](docs/guide/coverage-and-mutation.md) · [Generative checks](docs/guide/generative.md) · [Analyzers, dependencies, and benchmarks](docs/guide/analysis.md) · [Concurrency](docs/guide/concurrency.md) · [Specifications and provenance](docs/guide/specifications.md) · [Plugins](docs/guide/plugins.md)
- [Technical design](docs/CUPEL_TECHNICAL_DESIGN.md) · [Implementation plan](docs/CUPEL_IMPLEMENTATION_PLAN.md) · [Decision records](docs/adr/)

## Contributing

Every change is test-first and goes through `make`: `make dev-setup` installs the pinned tools, and `make check` (format, clippy, every test, rustdoc, a clean tree) is the bar for a commit. The tables and guide excerpts in this README and in `docs/guide/` marked as generated come from `cupel describe`; `make readme` rewrites them, and `make check` fails while they are stale. The captured examples are rerun with `scripts/readme-capture.sh`. Every target and the workspace layout: [docs/guide/development.md](docs/guide/development.md).

## License

BSD 3-Clause. See [LICENSE](LICENSE).
