# CUPEL_DECISION_230: A tools.deps project's own `:deps/prep-lib` runs before Cupel's Clojure runs, and doctor reports it while it is not done

**Date:** 2026-11-14 · **Status:** accepted · **Phase:** v0.3.0, item 3 (Q-clj-1) · Details [109](CUPEL_DECISION_109.md) and [210](CUPEL_DECISION_210.md) (the Clojure runner and Cloverage)

## Context
Validating v0.2.9 on clj-commons/clj-yaml, the survey errored with `ClassNotFoundException clj_yaml.MarkedConstructor`: clj-yaml's Clojure code imports Java classes its own build compiles, and `git clean -x` had removed them. Its `deps.edn` says so: `:deps/prep-lib {:alias :build :fn compile-java :ensure "target/classes"}`, with `target/classes` on its `:paths`. Cupel never ran that build; the validation compiled the classes in a hook after each checkout (NOTES Q-clj-1: "recommend init (or the suite) runs the project's own prep … before tests when declared; otherwise doctor says so").

The owner's ruling: "run :deps/prep-lib as part of the project build; doctor reports when it is needed."

In tools.deps, `:deps/prep-lib` is how a library says it must be prepared before use: `:alias` names an alias of its `deps.edn`, `:fn` a function run in tool mode with that alias (`clojure -T<alias> <fn>`, resolved through the alias's `:ns-default`), and `:ensure` a directory, relative to the library, that exists once it is prepared. `clojure -X:deps prep` prepares every library in a project's dependency tree that declares it and whose `:ensure` directory is missing, running each in the library's own directory; the CLI refuses to build a classpath with an unprepared git or local dependency. Nothing prepares the project itself: the declaration is the same build, but `-X:deps prep` only walks the dependencies.

Open: when Cupel runs the prep, where, what failure does, what doctor says, and what of dependencies that need prep.

## Options
1. **When.** (a) Once at `init`: the files it makes are build output, which a clean checkout, a mutation slot (whose copy leaves out `target/`, ADR-080 §8), or `git clean` removes. (b) Before every Clojure run Cupel makes in a root, when the `:ensure` directory is missing, which is tools.deps' own test of whether a library is prepared: a prepared tree costs one directory check, and every copy of the tree prepares itself once.
2. **What counts as declared.** (a) The top-level `:deps/prep-lib` of the root's `deps.edn` with a keyword `:alias`, a symbol `:fn`, and a string `:ensure`; anything else (the key inside an alias, a missing part) is not a declaration Cupel can run. (b) Run `clojure -X:deps prep` as well, for the dependencies: it may fetch git dependencies, and the CLI already says what to run when one is unprepared.
3. **Failure.** (a) A prep that fails fails the run that needed it, with its command and the tail of its output, as a failed build does. (b) Run the tests anyway: they would fail on the missing classes and blame the tests.
4. **doctor.** (a) A finding per Clojure root whose declaration is not done (`:ensure` missing): `info`, as Cupel's next run prepares it, and `warn` when the Clojure CLI that runs it is not on `PATH` (doctor reports the CLI itself as a missing tool too). (b) A finding whenever a declaration exists: nothing to do once it is done.

## Decision
1. **(b).** Before each run and listing of `tool.clojure-test` in a root, and before Cloverage's (`tool.cloverage`) command there, Cupel reads the root's `deps.edn`; when it declares `:deps/prep-lib` and `<root>/<:ensure>` is not a directory, it runs `clojure -T<alias> <fn>` in the root and waits for it. Runner and coverage specs name what prepares a root (`prepare`); only the Clojure ones do.
2. **(a).** Only the project's own top-level declaration, read by a small EDN reader (strings, comments, commas, nesting; `#_` discards and reader conditionals are not interpreted). Dependencies that need prep are left to `clojure -X:deps prep`, which the CLI's own error names.
3. **(a).** A prep that exits other than 0 is an error of the run: "`clojure -T:build compile-java`, the build deps.edn's :deps/prep-lib declares, did not prepare target/classes: <output tail>".
4. **(a).** `doctor` reports `doctor.prep-needed` for each Clojure root whose declaration is not done, with evidence `root`, `alias`, `fn`, `ensure`, and `command`, `info` when `clojure` is on `PATH` and `warn` otherwise.

## Consequences
- clj-yaml's tests and Cloverage run on a clean checkout and in every mutation slot; each prepares once, then finds `target/classes`.
- A prep that needs the network (a tools.build dependency not yet in the local repository) needs it at that run, as the project's own build would.
- A stale `:ensure` directory (Java sources changed since) is not rebuilt: tools.deps' own test is the directory's existence, and Cupel follows it.
- Fixture: `crates/cupel-fixtures/repos/tools-build` now needs its prep (its namespace imports the Java class the build compiles, and its `:build` alias compiles with the JDK's own compiler, so no dependency is fetched); `cupel-adapt/tests/runners.rs`, `cupel-fixtures/tests/tools_build.rs`, and `cupel-cli/tests/doctor.rs` cover the runner, Cloverage, and doctor.
