# CUPEL_DECISION_231: Rust coverage runs doctests on a nightly toolchain, and a function only a doctest it did not run may cover is `coverage.doctest-only`

**Date:** 2026-11-14 · **Status:** accepted · **Phase:** v0.3.0, item 11 (Q-rs-4) · Details [079](CUPEL_DECISION_079.md) (coverage producers and `tests.coverage`); a field on two facts, not a new fact kind

## Context
Validating v0.2.9 on tokio-rs/bytes, the survey reported 159 `coverage.uncovered-symbol`, many of them `Buf::try_get_*`, which only their own doctests exercise (`src/buf/buf_impl.rs:1200`, `/// assert_eq!(Ok(0x08_u8), buf.try_get_u8());`). Cupel's coverage command, `cargo llvm-cov --lcov --output-path {out}`, runs no doctests: `cargo llvm-cov --doctests` is unstable and needs a nightly toolchain. NOTES Q-rs-4 recommended running doctests under coverage when a nightly toolchain is present, and otherwise reporting "a symbol whose own doc comment has a runnable example" at `info` as covered only by doctests and not measured.

The owner's ruling: "doctests run for coverage when a nightly toolchain is present, otherwise coverage.doctest-only at info."

Open: how Cupel tells a nightly toolchain is present without the network, what command it runs then, and which uncovered functions are `coverage.doctest-only`, which cannot be known for certain without running the doctests.

## Options
1. **Nightly present.** (a) `cargo +nightly --version`: exits 0 with the toolchain alone, while `cargo llvm-cov` on nightly also needs the toolchain's `llvm-tools` component, and without it cargo-llvm-cov offers to install it (network). (b) `rustup toolchain list` has the `nightly-<host>` toolchain (a dated nightly alone is not `+nightly`) and `rustup component list --toolchain nightly --installed` lists `llvm-tools`: both read local state.
2. **The command.** (a) Write a nightly command into `cupel.toml` at `init`: the file would differ between a machine with nightly and CI without it. (b) At run time, run the configured `cargo llvm-cov <options>` as `cargo +nightly llvm-cov --doctests <options>` where nightly is present; a configured command that passes `--doctests` itself runs as written and counts as running them; any other command (a subcommand such as `nextest`, a `+toolchain` of its own) runs as configured.
3. **Which functions are doctest-only.** (a) Any uncovered function of a crate with doctests: most of bytes' 159 would be hidden, including functions no example touches. (b) An uncovered function named in some doctest anywhere in the crate: a name in a module's or another function's example says little about which function the example runs, and needs a crate-wide index. (c) An uncovered function whose own doc comment holds a fenced block rustdoc runs (no info string, or only `rust`, `should_panic`, `test_harness`, `standalone_crate`, `edition…`, or an error code; not `ignore`, `ignore-…`, `no_run`, `compile_fail`, or another language) and whose code names the function as an identifier. That is the evidence NOTES cites: the example sits on the function and calls it.
4. **Where the knowledge lives.** `tests.coverage` sees no language. (a) A field on `SymbolDef`, `doc_example`, set by the language (Rust's from its `///` and `/** */` comments, through attributes and plain comments), and a field on `CoverageMap`, `doc_examples`, set by the producer that ran them. (b) A new fact kind for documentation examples: a cost across every language (§2.5) for one bit per symbol.

## Decision
1. **(b).** `cupel_adapt::coverage::nightly_coverage` asks `rustup`, once per process, for the `nightly` toolchain and its `llvm-tools` component.
2. **(b).** `tool.cargo-llvm-cov` runs `cargo +nightly llvm-cov --doctests <the configured options>` where it holds, and marks its map `doc_examples`; a command with `--doctests` is run as configured and marked too. The joint `tool.llvm-cov-nextest` and the per-test map run no doctests (nextest cannot), and their maps are not marked.
3. **(c).** A function no test ran, in a file the suite loaded, whose `doc_example` is set, under a map whose `doc_examples` is not, is `coverage.doctest-only` at `info`, with `coverage.uncovered-symbol`'s evidence (`lines`, `covered`, `own_root`); it fails nothing. Under a map that ran the examples, an uncovered function is `coverage.uncovered-symbol` as before, its example having run without reaching it. Its lines count in the line ratio and it counts in `uncovered` either way: the run measured them uncovered.
4. **(a).** `SymbolDef.doc_example` and `CoverageMap.doc_examples` are optional booleans, absent when false; `Symbols` goes to version 3 so cached parses are made again. `describe check tests.coverage` lists the kind.

## Consequences
- bytes' `try_get_*` functions are `coverage.doctest-only` notes on a machine without nightly, and measured as their doctests run on one with it.
- On nightly, coverage builds the crate with that toolchain: a crate that builds on stable only fails its coverage run (`coverage.run-failed`), and the first run is a build of its own.
- A cached report is served with the doctest marking the current machine would give it: a report made before nightly was installed reads as having run them until the tree changes.
- Indented code blocks, `#[doc = "…"]` attributes, and examples on a type or module are not read; their functions stay `coverage.uncovered-symbol`.
- Fixture: `crates/cupel-fixtures/repos/doctest-only/rust` (`add`, called only by its doctest; `double`, unit-tested; `halve`, called by nothing), tested by `crates/cupel-fixtures/tests/doctest_only.rs`; Rust's detection by `cupel-lang/tests/rust_symbols.rs`; the check's rule by `cupel-checks/tests/tests_coverage.rs`; the command by `cargo_llvm_cov`'s unit tests.
