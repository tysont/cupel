<!-- ABOUTME: cupel.toml: every table, gate rules and their tolerances, and the generated table of check settings. -->
<!-- ABOUTME: Part of Cupel's guide, linked from the README; `make readme` rewrites its generated sections and crates/cupel-cli/tests/readme.rs checks them. -->
# Configuration

`cupel.toml` is found by walking up from `--cwd`. Precedence is flags, then `CUPEL_*` environment variables (`CUPEL_DEPTH`, `CUPEL_FORMAT`, `CUPEL_PROFILE`), then `--profile`, then `cupel.toml`, then defaults. `CUPEL_NO_DAEMON=1` makes an invocation run in its own process even when a daemon serves the repository (see `cupel serve`). The envelope echoes the effective configuration and the layers that contributed under `run.config`. Unknown keys are usage errors (exit 2) that list the valid keys, and `cupel describe schema config` is the full schema.

```toml
required_version = ">=0.1"         # a Cargo version requirement this binary must satisfy
default_branch = "main"            # what --changed diffs against
depth = "standard"

[roots]                            # monorepos; when set, exactly these are the project's roots and scope (ADR-167, ADR-177)
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

**`required_version`** is a Cargo version requirement (`>=0.4`, `^0.4`, `~0.4.1`, comma-joined ranges) on the `cupel` binary. When the binary does not satisfy it, `doctor` reports `doctor.version-unsatisfied` with the required and actual versions and an `install_tool` action naming the [install routes](install.md) available now, and every verb that reads or grades the repository (all but `describe`, `doctor`, `init`, `suppress`, and `serve`) refuses with that finding and exit 3; a requirement that does not parse is `doctor.version-invalid`, exit 2 outside `doctor`. A suppressed version finding (`cupel suppress`) refuses nothing.

**Check settings** live under `[checks."<id>"]`, and `[checks."<id>".lang.<lang>]` overrides any of them for one language. Each check declares its settings in its manifest, and `cupel describe <check>` lists them with their defaults; a key a check does not declare is a configuration error that lists the ones it does. The checks that take settings:

<!-- generated:check-settings -->
| Check | Setting | Default | What it sets |
| --- | --- | --- | --- |
| `meta.traceability` | `requirements` | `[]` | The task's requirement IDs, such as `REQ-12`; a per-language override assigns them to the language whose tests trace them. |
| `perf.bench` | `threshold_percent` | `5` | The smallest change of a benchmark's median, in percent of the base's, that is reported. |
| `perf.bench` | `significance_percent` | `5` | The Mann-Whitney U test's level, in percent: a change with a lower p is significant. |
| `static.clones` | `threshold` | `0.82` | The least Jaccard similarity of two symbols' fingerprint sets that makes a clone, greater than 0 and at most 1. |
| `static.clones` | `min_lines` | `4` | The fewest lines a symbol must span to be compared. |
| `static.clones` | `min_nodes` | `23` | The fewest normalized-tree nodes a symbol must have to be compared. |
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
| `tests.mutation` | `max_mutant_timeout` | `"120s"` | The longest a mutant's run may take, as a duration such as 90s or 5m; a run that reaches it is a timeout, whatever timeout_factor allows. |
| `tests.mutation` | `max_mutant_memory` | `"50%"` | The resident memory the mutant runs may hold together, as a size such as 3g or 512m or a share of physical memory such as 50%; each slot's run gets an equal part, and a run over it is killed and is out-of-memory. |
| `tests.quality` | `mock_density_max` | `50` | The highest share, in percent, of doubles among a test's doubles and assertions before test.mock-heavy. |
| `tests.quality` | `mocks_min` | `2` | The fewest test doubles that can make a test test.mock-heavy. |
| `tests.quality` | `logic_max` | `0` | The most loops, alternatives, and unguarded branches a test may have before test.logic. |
| `tests.quality` | `setup_statements_min` | `5` | The fewest leading statements tests in one file must share to be test.duplicate-setup. |
| `tests.quality` | `setup_tests_min` | `3` | The fewest tests in one file that must share their leading statements to be test.duplicate-setup. |
| `tests.quality` | `debug_assertions` | `true` | Whether assertions only a build with debug assertions evaluates (`debug_assert…` macros and methods) count as assertions; false leaves them out of every test. |
<!-- end:check-settings -->

**Gate rules** map a key to a condition. The key is a check ID or `<check-id>.<metric>`, where the metric is one of that check's gate metrics in the check table under Checks. The condition is `none` (the check reported no unsuppressed `warn` or `fail` finding), `no-breaking` (no unsuppressed breaking finding, a kind ending in `.breaking`, such as `static.api-surface`'s `api.breaking`; any other check is rejected when the config loads), or a comparison `>= > <= < ==` against a number or against `base`; a number ending in `%` is divided by 100. `base` is the metric's value in the reference run, optionally with an offset in the metric's unit (`"<= base"`, `">= base - 0.02"`, `"< base + 3"`). With no offset written, the metric's tolerance loosens the bound: base − δ under `>=` and `>`, base + δ under `<=` and `<`, none under `==` ([ADR-182](../adr/CUPEL_DECISION_182.md), [ADR-195](../adr/CUPEL_DECISION_195.md)). δ is 0.02 for `tests.mutation.score`, a sampled ratio; 2 for `static.complexity`'s `cyclomatic_max`, `cognitive_max`, and `nesting_max`, one function's largest value; and 0 for every other metric, so counts such as `static.deps.cycles` are exact; a written offset replaces it (`">= base - 0"` is exact). Those three `static.complexity` metrics also have a watch threshold, the check's limit setting of the same name (default 10, 15, and 4, with each language's override, the lowest among the scope's languages): a `<=` or `<` rule against `base` that writes no offset passes while the value is at most it, and fails when a change takes the value past it from a base at or below it, even within δ; a function already past the limit may still move by δ. So one branch added to a small function passes, and a function pushed past the limit fails ([ADR-199](../adr/CUPEL_DECISION_199.md), [ADR-205](../adr/CUPEL_DECISION_205.md)). A rule whose result the threshold decided carries `watch` (text: `within watch 10` or `past watch 10`). The reference run is the run `--against` names; otherwise, for `--changed` and `--base`, the base recomputed, by running the rule's checks over the base commit's versions of the changed files in a temporary copy, narrowed to the code the change replaced there ([ADR-175](../adr/CUPEL_DECISION_175.md)), as the current side is narrowed to the code it changed; otherwise the `default` baseline (`cupel baseline set --write`). A rule passes only when its check ran to completion and reported the metric; a rule on a check that did not run is `skipped`, as is a rule against `base` with no reference value (no baseline is set, or every changed file is new), and any rule that does not pass fails the gate, except one against `base` skipped for want of a reference: it emits a `baseline.missing` finding at `info` naming the rule, and the gate passes if every other rule does ([ADR-198](../adr/CUPEL_DECISION_198.md)). Each rule in `summary.gate` carries `actual`, the `bound` it was compared with (with the tolerance applied), `tolerance` when one was applied (text: `tolerance 0.02`), and for a skipped rule the `reason`. When the metric is a largest value over functions or modules (`static.complexity`'s `*_max`, `static.crap.max`, `static.deps.max_distance`), the rule also names what each value was measured on: `actual_target`, and for a rule against `base`, `bound_target` from the reference run, each with its display name and location (text: `actual 30 (fmt::render at src/fmt.rs:12)  bound 18 (fmt::render at src/fmt.rs:9)`). Counts and ratios name nothing, and nor does a threshold. `[gate.changed]` applies to `--changed` and `--base` scopes and `[gate.tree]` to everything else. A `[gate.tree]` rule on `tests.weakening`, `static.api-surface`, `meta.traceability`, or `meta.oracle-independence`, which need a range, is rejected when the config loads.

Cupel writes only under `.cupel/` (run records, the SQLite fact store, `cache/` with coverage reports and per-test maps, scratch space) unless `--write` is given. An install you allow (`--install`, or yes to the question) runs your package manager, which for the JavaScript launchers, uv, and poetry adds a dev dependency to `package.json` or `pyproject.toml` and its lockfile. `.cupel/.gitignore` excludes everything there except `suppressions.toml`.
