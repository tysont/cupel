# CUPEL_DECISION_104: `tests.assertion-density`, the extension drill: the check's contract and every file a new check touched

**Date:** 2026-10-16 · **Status:** accepted · **Phase:** P6.5

## Context

P6.5 is a drill: add one genuinely new check over facts that already exist and record every file it touched. The plan's target is one module in `cupel-checks`, its `mod` line in `lib.rs`, and goldens (plan §2.8; design §8.3.1, "check over existing facts: one module in `cupel-checks` + goldens"; ADR-074 makes the rule exact: the module, its `mod` line, its fixtures, `golden/<check-id>/`). Anything else is an extension-model defect to fix before P7.

The check chosen is the plan's first candidate, `tests.assertion-density`. It needs a contract: what "density" is per test and per file, what "per tested public function" means when no fact records which function a test calls (ADR-058: no fact carries references), how it relates to `tests.quality` (which already reports `assertion_density`, assertions per test, as a metric), its tier, its settings, and how its findings are identified.

## Options

**Density per test as a count.** A test's assertions alone. `tests.quality` already reports a test with none (`test.no-assertion`), and one assertion is idiomatic for a short test (most tests in the fixtures and in Cupel make exactly one), so any useful minimum above zero reports most good tests. Rejected.

**Density per test against its length.** Assertions per line the test spans: a test that sets up thirty lines of state and pins one value is the case agents produce when they make a test pass rather than make it check. Needs only `TestCase.span` and `TestCase.assertions`. Chosen, as lines per assertion so the limit is an integer `Limits` can hold.

**Per file against a fixed limit vs. against the repository.** A fixed assertions-per-test minimum for a file is the per-test count again. Comparing each test file with the repository's assertions per test reports a file whose tests assert much less than the project's own norm, whatever the norm is; chosen. "The repository" is every test case in the `TestFrames` the run read: in a whole-tree run that is the repository, in a `--changed` run only the changed test files (facts are produced over the files in scope, ADR-041), which is recorded below as a limitation.

**"Tested public function" from names, from conventions, or from references.** Matching test names to function names works for Rust, Python, and Go (`test_deposit`, `TestDeposit`) but not for JavaScript and TypeScript, whose tests are sentences (`"adds two numbers"`). A `References` fact (ADR-058's proposal) would say which function a test calls, but adding a fact kind is the one change the drill must not need. `TestConventions` already pairs each source file with its test files in all five languages and is what `gen.property` and `tests.mutation` use for the same question. Chosen: a public function is tested when its source file is paired with a test file the run read that holds at least one case, and the assertions it is credited with are those test files' assertions. So `TestConventions` is a third needed fact beside the plan's `TestFrames` and `Symbols`; all five languages produce it, so support is unchanged.

**Identity: the symbol's structural hash or the subject.** `tests.quality` identifies a test finding by the test symbol's `NormTree` hash, which needs a fourth fact and changes whenever the test body changes, that is, exactly when the test is fixed. `tests.weakening` and `tests.flaky` identify tests by subject (`<file>::<test name>`, ADR-016, ADR-057). Chosen: the subject, so `NormTree` is not needed; a file finding's subject is its path.

**Tier.** The step's brief suggested tier 0/1 "as the design's tiering implies". The design's tier table (§4) puts "coverage and test-suite hygiene" at tier 2 and §5.2 places every `tests.*` check at tier 2–3; `tests.quality`, which measures the same assertions, is tier 2. Tier 0/1 is "structural/static metric" and "static analysis" of the code under test. Chosen: tier 2, for consistency with the design's table and with `tests.quality`; a reviewer who wants 0 changes one line and the goldens.

## Decision

`tests.assertion-density`, version 1, tier 2, cost `quick`, `needs = [test-frames, symbols, test-conventions]`, no `wants`, stability `experimental`. Support is derived; all five languages produce the three facts, so it is `supported` everywhere.

**Settings** (`[checks."tests.assertion-density"]`, through `Limits`): `lines_per_assertion_max` (20), `file_density_percent_min` (50), `file_tests_min` (3). Unknown keys and non-integers are configuration errors listing the three names.

**Findings** (both `warn`, confidence 1, action `strengthen_assertion` priority 2, no `symbol`, repro `cupel check --only tests.assertion-density --paths <file>`):

- `density.sparse-test`: a test in scope that makes at least one assertion and spans more than `lines_per_assertion_max × assertions` lines (first to last line of its span). Subject `<file>::<test name>`, located at the test's span. Evidence `{test, assertions, lines, lines_per_assertion (rounded down), lines_per_assertion_max}`. A test that asserts nothing is never reported here: it is `tests.quality`'s `test.no-assertion`.
- `density.sparse-file`: a test file in `scope.files` with at least `file_tests_min` cases whose assertions per test are below `file_density_percent_min` percent of the repository's (every case in the `TestFrames` read), compared exactly as `100 × a_file × t_repo / (t_file × a_repo)` rounded down, so a file at exactly the limit is not reported. Subject the file's path, located from its first case's first line to its last case's last line. Evidence `{tests, assertions, repository_tests, repository_assertions, percent, file_density_percent_min, file_tests_min}`. Not reported when the repository makes no assertions.

**Metrics** (all `scope`, gate keys `tests.assertion-density.<metric>`): `assertions_per_test` (the score, over the tests in scope; equal by construction to `tests.quality.assertion_density`), `assertions_per_public_function` (assertions of the test files paired with each source file that has a public function in scope, over those functions), `tested_public_functions`, `sparse_tests`, `sparse_files`. A public function is a `Symbols` entry with `visibility: public`, kind `function` or `method`, in scope, in a file that is not a test file (`TestConventions::is_test`). Across languages `assertions_per_test` is weighted by tests (the score's `n`), `assertions_per_public_function` by `tested_public_functions`, and the counts add up. Status is `fail` when any finding was reported, as `tests.quality`'s is. No per-target (ranked) metric.

**Fixtures.** `repos/tests.assertion-density/<lang>/` in all five languages, the same shape: a ledger source file and a report source file with two public functions each; the ledger's test file has three tests of four assertions each; the report's has three tests of one assertion each, the last a 24–26-line test for its one assertion. Each golden therefore holds one `density.sparse-file` (40% of the repository's 2.5) and one `density.sparse-test`.

### Files touched

Outside `crates/cupel-checks/src/tests_assertion_density.rs`, its `mod` line in `crates/cupel-checks/src/lib.rs`, `crates/cupel-fixtures/repos/tests.assertion-density/`, and `crates/cupel-fixtures/golden/tests.assertion-density/` (the five matrix goldens, `describe.json`, and the five `survey-<lang>.json` the harness demands):

| File | Why it was necessary | Model defect? |
| --- | --- | --- |
| `crates/cupel-checks/tests/tests_assertion_density.rs` | The check's own tests (TDD). Every check has one; it is new, beside the module, and touches nothing shared. | No: part of the check, as the module is. |
| `crates/cupel-checks/src/metrics.rs` | `case_key`, the join from a `TestCase` to the `SymbolKey` that scope is decided on, moved here from `tests_quality.rs`, where it was private; the new check needed the same function and `static.clones` reported the copy. | Yes, fixed: the next check over `TestFrames` calls `metrics::case_key`. |
| `crates/cupel-checks/src/tests_quality.rs` | Calls `metrics::case_key` instead of its private copy (two lines). | Consequence of the fix above; behavior unchanged, goldens unchanged. |
| `crates/cupel-checks/tests/support/mod.rs` | `span(line, lines)`, the test-span helper, moved here from `tests_quality.rs`'s tests, for the same reason. | Yes, fixed: test-only, shared by the next check's tests. |
| `crates/cupel-checks/tests/tests_quality.rs` | Uses `support::span` instead of its copy. | Consequence of the fix above. |
| `README.md`, generated sections | The check-table row and the support-matrix row, written by `make readme`; the staleness test (ADR-073) fails until they are regenerated. | No: generated, and the ADR-074 note already names it as the one whole-registry artifact. |
| `README.md`, prose | A bullet under "What each check looks for" and the check's settings and defaults in the Configuration paragraph. | Yes, not fixed: see Consequences. |
| `docs/adr/CUPEL_DECISION_104.md`, `docs/CUPEL_IMPLEMENTATION_PLAN.md` | This record and the step's status and Notes. | No: process. |

Nothing in `cupel-core`, `cupel-cli`, `cupel-lang`, `cupel-adapt`, `cupel-mutate`, `cupel-plugin`, or `cupel-store`; no shared fixture repository; no other check's goldens; no whole-registry golden (`describe/*`, `cli/*`, `survey/<lang>.json`) changed. `describe <check>`, `describe matrix`, the matrix test, the declared-facts-only run, and the survey all derived the new check from the registry.

## Consequences

- A check over existing facts is, as ADR-074 intended, its module, its `mod` line, its fixtures, and its goldens, plus its own test file and the regenerated README tables. The two shared helpers moved by this step are where the next check over `TestFrames` finds them.
- **Defect recorded, not fixed: check settings are not self-described.** A check's settings live in its private `Limits` constant; the `Manifest` has no `settings` field, so `describe <check>` does not list them and the README's Configuration paragraph is hand-written per check. The fix is a `settings: Vec<SettingSpec { name, default, summary }>` on `Manifest`, filled from `Limits` (and from `static.clones`' and `static.deps`' non-integer settings), rendered by `describe` and by a generated README section. It changes the `Manifest` shape that plugins also declare (ADR-088), every check's `describe.json` golden, and the run and plugin schema snapshots, and it would conflict with the P6.2 branch adding three checks. It is a design change across the plugin protocol, so it is left for a decision at the P6 merge rather than made inside a drill; until then each new check adds one sentence to the README.
- The per-check README bullet is editorial prose beyond the manifest's one-line `summary`; it stays hand-written by choice.
- `--changed` compares a file with the changed test files only, not the whole repository: facts are produced over the scope's files. A whole-repository reference would need the scheduler to produce `TestFrames` over the tree for this check, or the store's history; neither exists, and the comparison is still between real test files.
- A test case with no `Symbols` entry at its key (one written inside a macro invocation, for example) is outside a symbol-resolved scope, as it is for `tests.quality`; on Cupel's own tree 20 of 1568 cases are (the cause of each was not traced). They still count toward the repository's density.
- One structural equality assertion (`assert_eq!(actual, expected_table)`) counts as one assertion however much it pins, so table-driven tests read as sparse; this shows in the dogfood (P6.5 Notes). A future `AssertionKind` weight, or counting expected elements, is a `TestFrames` question, not this check's.
- No gate rule is added to Cupel's `cupel.toml`; the dogfood findings are recorded in the plan.
