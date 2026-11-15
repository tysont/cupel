<!-- ABOUTME: Checks and fact producers added as plugins, the PIT plugin, and the reference plugins. -->
<!-- ABOUTME: Part of Cupel's guide, linked from the README. -->
# Plugins

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

## The PIT plugin

[`plugins/pit/cupel_produce_pit.py`](../../plugins/pit/cupel_produce_pit.py) is a producer plugin that makes `mutation-outcomes` for Java with [PIT](https://pitest.org): it copies the project into its scratch directory, runs PIT there (Maven: the `pitest-maven` plugin with `pitest-junit5-plugin` declared in the `pom.xml`; Gradle: the `info.solidsoft.pitest` plugin with XML output and untimestamped reports), and places each PIT mutation on the Cupel mutation site of its line and operator, with the tests that killed it. `mutator` pins it over the built-in engine:

```toml
[plugins]
pit = ["python3", "plugins/pit/cupel_produce_pit.py"]

[lang.java]
mutator = "plugin.pit"
pit = "mvn -B -q test-compile org.pitest:pitest-maven:mutationCoverage -DoutputFormats=XML -DtimestampedReports=false -DfullMutationMatrix=true"   # the default for Maven
```

`pit_report` names where the command leaves `mutations.xml`, and `pit_report_ready = true` reads a report already in the tree instead of running PIT.

## Reference plugins

Reference plugins in Rust, Python, and TypeScript live in [`crates/cupel-fixtures/plugins/`](../../crates/cupel-fixtures/plugins/): a producer that reads a `per-test-map` for JavaScript and TypeScript from an LCOV file with one `TN:` section per test, and a check that reports the files in scope no test executes. With the Python pair in `[plugins]` of a TypeScript repository, `cupel describe fact per-test-map` lists it beside the built-in producers, `cupel tests --impacted` works for TypeScript, and the check runs:

```text
$ cupel check --only ref.untested-python --format text
cupel check: pass (exit 0)

summary   1 finding · fail 0 · warn 1 · info 0 · suppressed 0 · new and resolved need a baseline (`cupel baseline set`)

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
