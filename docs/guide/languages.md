<!-- ABOUTME: The seven languages, the generated support matrix, and the test runners Cupel drives. -->
<!-- ABOUTME: Part of Cupel's guide, linked from the README; `make readme` rewrites its generated sections and crates/cupel-cli/tests/readme.rs checks them. -->
# Languages and test runners

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

Java is read through its package clauses: a method's name is `package.Class.method`, `public` and `protected` members are public, package-private ones `internal`, and a Maven or Gradle build (`pom.xml`, `build.gradle(.kts)`, `settings.gradle(.kts)`) is a project whose modules are members; a `pom.xml` that does not name its coordinates (its own `artifactId`, and a `groupId` and `version` of its own or from a `<parent>`), such as the template a Clojure project's tools.build fills at release, is no Maven build, so it gets no Java root or runner. Tests are JUnit 5, JUnit 4, and jqwik methods. Clojure is read through its `ns` forms: a var is `namespace/name`, `defn-` and `^:private` make it private, `cond` is its else-if, and a `deps.edn` or Leiningen `project.clj` is a project. A Leiningen project (a `project.clj` and no `deps.edn`) gets `test = "lein test"` from `init`, and `coverage = "lein cloverage --lcov"` only where its `project.clj` declares the lein-cloverage plugin; Cupel runs neither, so its test checks skip and its static checks run ([ADR-234](../adr/CUPEL_DECISION_234.md)). Tests are `clojure.test` `deftest`s and test.check `defspec`s. Every language is held to the same conformance inventory, a shared list of constructs per fact that each language must exhibit in its fixture repository and detect, or mark not applicable with a reason.

## Test runners

Checks that depend on running the test suite read a `test-results` fact, which Cupel makes by running the project's own test runner and reading its machine-readable output:

| Language | Runners, in default order | Reads |
| --- | --- | --- |
| Rust | `cargo nextest` (when `.config/nextest.toml` exists), `cargo test` | JUnit through a nextest tool profile; libtest output |
| Go | `go test -json` | test2json events |
| Python | `pytest` (when the project mentions it), `unittest` | JUnit XML; `unittest -v` output |
| JavaScript, TypeScript | `vitest`, `jest` (whichever `package.json` depends on), `node --test` (Node's built-in runner, when neither is the root's runner) | the Jest JSON report; `node --test`'s JUnit XML (Node 24.11.1 or later, which names each test's file) |
| Java | Maven (`mvn test`, when a `pom.xml` exists), Gradle (`gradle test --rerun`) | Surefire's and Gradle's JUnit XML, one file per test class |
| Clojure | `clojure.test` through the Clojure CLI (`clojure -M:test` and a runner script Cupel writes under `.cupel/`; an alias Cupel adds clears the `:test` alias's `:main-opts`, such as cognitect-labs/test-runner's or Kaocha's, so the script runs with that alias's paths and dependencies, [ADR-210](../adr/CUPEL_DECISION_210.md); a project whose `deps.edn` declares `:deps/prep-lib` is prepared first, [ADR-230](../adr/CUPEL_DECISION_230.md)) | `clojure.test.junit` XML |

A runner is used when its program is on `PATH` (or, for `vitest` and `jest`, in the root's `node_modules/.bin`); otherwise checks that need test results are skipped as `unavailable` with the tool named, and `cupel doctor` says what to install (`cupel doctor --fix --install` installs it). `[lang.<id>] runner` chooses one explicitly, by its name with or without the `tool.` prefix:

```toml
[lang.python]
runner = "unittest"
```

A tools.deps project that needs its own build before its code loads declares it in `deps.edn` as `:deps/prep-lib {:alias :build :fn compile-java :ensure "target/classes"}` (clj-commons/clj-yaml compiles Java classes so). Before each Clojure run in such a root (the baseline, reruns, mutants' runs in their copies of the tree, and Cloverage), Cupel runs `clojure -T:build compile-java` there when the `:ensure` directory does not exist, and a build that fails fails the run with its output. `cupel doctor` reports a root whose `:ensure` directory is missing as `doctor.prep-needed` with the command: a note, or a warning without the Clojure CLI ([ADR-230](../adr/CUPEL_DECISION_230.md)).

Every pytest run Cupel makes (the baseline, the [one suite run](#one-suite-run), the per-test map, reruns, and mutants' runs) passes the arguments `[lang.python] test` gives pytest, so `test = "python -m pytest -m 'not network'"` leaves a project's network tests out of all of them. A run narrowed to tests Cupel names keeps the options with their values and leaves out configured paths, which pytest would run in full ([ADR-229](../adr/CUPEL_DECISION_229.md)).

Before any check that needs test results runs, Cupel runs the suite once in the real tree, the **baseline**. If any test fails, or the runner cannot finish (a build error), the run is refused: `status: "error"`, exit code 4, no check scores, and one `baseline.test-failed` finding per failing test (with its file, runner, and the tail of its output) or `baseline.run-incomplete` per run that did not finish. A Go package that did not build or set up, and a Go build that failed outside any package's tests (such as a toolchain without `covdata`), is one errored test named after the package, its output the build output `go test -json` printed for it. Fix the suite, then rerun. A mutation or flakiness score over a red suite would mean nothing, so no mutant and no rerun runs before the baseline is green. Every test of the baseline is also indexed in `.cupel/cupel.db`. `tests.flaky` and `tests.mutation` need test results, so `cupel check` at the default depth (`standard`) runs the baseline; at `--depth quick` no test runs. A language whose baseline ran no test at all skips them as `unavailable`.

### One suite run

The baseline, the coverage report, and the per-test map each need the suite to run. Where the runner and the coverage tool are one toolchain and the `coverage` command is the one `cupel init` writes, a run that needs two of them runs the suite once, under coverage, and takes all three from it:

| Language | One run | Makes |
| --- | --- | --- |
| Rust (runner `cargo-nextest`) | `cargo llvm-cov nextest`, each test process's profile kept apart | baseline, coverage (`cargo llvm-cov`'s own report of the run), per-test map |
| Python (runner `pytest`) | `pytest --cov --cov-context=test` | baseline, coverage, per-test map |
| Go | `go test -json -coverprofile` | baseline, coverage (the per-test map stays one run per test) |

A green run is cached in `.cupel/cache/suite/` with the hash of every file under the project root and the toolchain that measured it (`rustc -vV`'s release and host; the Python interpreter's and pytest's versions). While nothing there changes, the next run reuses it and runs no test, the baseline included. A red or incomplete run is never cached: it refuses the run with exit 4 and runs again next time, since what made it red may lie outside the tree (a full disk, a missing service). On a tree scope, at `--depth deep`, or under another toolchain, any change runs the suite again, so the mutation and flakiness engines always start from a real baseline of the tree they test.

On a `--changed` or `--base` scope below `deep`, a Rust or Python joint with a cached run reruns only the tests the change makes stale and keeps the rest from its cache ([ADR-149](../adr/CUPEL_DECISION_149.md)): a test is stale when a file it covers changed inside a function it executed, when its own test case's structure changed, when a file it owns changed, or when a build or test configuration file changed (then every test is). The kept tests' results and per-test lines join the rerun tests', and the coverage report of the rerun gets a hit on every line a kept test executed, so which lines ran is what a whole run would say. A new test file still runs the whole suite. The envelope's `run.suite` says what happened per language (`{"rust": {"mode": "incremental", "rerun": 3, "reused": 1275}}`; `full` or `cached` otherwise), and the text output prints it as `suite  rust incremental · 3 rerun · 1275 reused`. A customized coverage command, another runner (`cargo test`, `unittest`), or a pin on one keeps separate runs. `cupel check --dry-run` says which in each check's `needs`:

```console
$ cupel check --only tests.mutation --dry-run    # pytest, with the default coverage command
  "needs": ["symbols", "mutation-outcomes", "test-conventions", "test-results+coverage-map+per-test-map(one run)"]

$ cupel check --only tests.mutation --dry-run    # coverage = "pytest --cov --cov-branch --cov-report=lcov:{out}"
  "needs": ["symbols", "mutation-outcomes", "test-conventions", "test-results(run)", "coverage-map(run)", "per-test-map(deep)"]
```

With the run cached the first says `test-results+coverage-map+per-test-map(cached)`; on a range with a cached run it says `(incremental: 3 stale)`, the tests it would rerun.

A test that spawns the repository's own binary covers, through it, most of the code, so nearly any change makes it stale. An attribution names such tests by ID glob and the files they are attributed to instead ([ADR-151](../adr/CUPEL_DECISION_151.md)):

```toml
[lang.rust.attribution.binary]
tests = ["cupel-cli::*"]                          # globs over the test IDs the envelope shows
files = ["crates/cupel-cli", "crates/cupel-fixtures"]   # paths as `[scope] exclude` writes them
```

Below `--depth deep`, such a test reruns only when one of those files, its own files, or a build or test configuration file changed; a change elsewhere keeps its cached result and lines even when it ran the changed function, and `run.suite` counts it in `tagged` (`suite  rust incremental · 18 rerun · 2784 reused (10 by attribution)`). When only kept tests ran a change, they rerun after all, so the change is measured. Its coverage still counts everywhere coverage counts, and the mutation engine still selects it for the mutants it covers. `tests --impacted` lists it with `kept` saying which attribution kept it and leaves it out of the commands. At `--depth deep` no attribution applies.

### Flakiness

`tests.flaky` reruns the tests the baseline ran, three times by default, with the same runner, and compares the outcomes. The reruns differ on purpose: the first two run the tests in their declared order one at a time, differing only in seed; the third runs them reversed (pytest and unittest, which take the tests in order) or in the runner's seeded shuffle (`go test -shuffle`, libtest's `--shuffle-seed`, `jest --randomize`, `vitest --sequence.shuffle`, Surefire's random class order (`node --test` has no shuffle: its reruns differ only in `--test-concurrency`), which otherwise runs classes alphabetically; Cupel's Clojure runner takes the reversed order); further reruns shuffle again, alternating between the runner's default parallelism and one test at a time. Every rerun's seed is derived from the run's `--seed`, so `cupel check --only tests.flaky --seed N`, which each finding gives as its `repro`, repeats the same reruns.

- `test.flaky`: the test passed and failed in two reruns that ran the tests in the same order. Something besides the code and the order decides it: time, randomness, concurrency, or the environment.
- `test.order-dependent`: the outcome is the same in every rerun of one order and differs between orders. The evidence lists `preceded_by`, the tests that ran before it in every rerun it failed and after it in some rerun it passed, when the runner reports its order.

```toml
[lang.rust]
flake_runs = 5      # reruns per check run; 0 turns them off
```

Over the whole tree (or `--paths`) the reruns repeat the whole suite, so they cost about three baseline runs. Over a range (`--changed`, `--base`) they rerun only the tests the range impacts, chosen before the run as `cupel tests --impacted` chooses them: by the language's per-test map where the run's depth affords one, else the test files that import a changed file ([ADR-186](../adr/CUPEL_DECISION_186.md), [ADR-190](../adr/CUPEL_DECISION_190.md)). A range that impacts no test reruns nothing, and `tests.flaky` is skipped as `no-impacted-tests`. `cargo nextest` runs each test in its own process, where order cannot matter; with it as the runner, the flake engine does not rerun, and `tests.flaky` is skipped for that language as `unsupported`, naming the fact (`test-reruns`) and the tool (`cargo-nextest`), with the reason in its notes.
