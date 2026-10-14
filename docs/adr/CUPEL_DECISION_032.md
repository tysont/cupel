# CUPEL_DECISION_032: Go syntactic facts: visibility, test idioms, and mutation spellings

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P1.2–P1.6

## Context

Go's producers for `Symbols`, `Decisions`, `NormTree`, `MutationSites`, `TestFrames`, and `TestConventions` are queries and tables only, but several Go constructs have no counterpart in the Rust reference, and each needed a decision every later Go check depends on:

1. Visibility. Go has two levels: capitalized names are exported, all others are visible throughout their package. `internal/` directories restrict who may import a package without changing any name.
2. Assertions. The standard `testing` package has no assertion functions: a test fails by calling `t.Error*`, `t.Fatal*`, or `t.Fail*`, almost always inside an `if` whose condition is the real assertion. testify and go-snaps add named assertions.
3. Subtests. `t.Run(name, func(t *testing.T) { … })` runs a nested test, usually with a computed name.
4. Return-value mutation. Go has no literal that is the zero value of every type.
5. Ternaries and catches. Go has neither.

## Options

**Visibility.** (a) Unexported = `private`, as the plan's step text suggested. (b) Unexported = `internal`, matching `conformance/symbols.toml`, whose `internal` construct is "visible throughout its package … but not exported (`pub(crate)`, lowercase Go)" and whose `private` is "visible only within its module, file, or class". For `internal/` directories: (c) a path-aware rule, which `[visibility]` cannot express without a new table key; (d) keep their exported names `public`.

**Assertions.** (a) Count every failure call as a weak truthiness assertion. (b) Make the guarding `if` the assertion node: a failure call outside an `if` would then need a second node kind. (c) Make the failure call the assertion node and pick its key from the condition guarding it, with idiom patterns placed before the plain failure-call pattern so the earliest pattern wins.

**Subtests.** (a) Each `t.Run` is a case: the enclosing test then has no assertions of its own and every table-driven test looks assertion-free. (b) Subtests belong to the enclosing test function.

**Return values.** `nil` (errors, pointers, slices, maps, channels, functions, interfaces) versus `0` or `""`: one template applies to every site.

## Decision

- Unexported names are `internal`; Go maps `private` as not applicable. Exported names in `internal/` packages stay `public`: they are visible outside their package, and the import restriction is a module-boundary rule a name rule cannot see. A symbol's namespace is its package clause's name, then the method receiver's type.
- Each failure call on `t`, `b`, or `tb` is one assertion, classified by its guard: `if err == nil` (a left operand named `err…`) is `throws`/weak; `if err != nil` is `not-null`/weak (the call succeeded, as Rust's `is_ok` idiom); `if x == nil` is `not-null`/weak; `if got != want` is `equality`/strong; `if got == unwanted` is `equality`/weak; any other failure call is `truthiness`/weak. testify functions are keyed by name (`Equal` strong, `NotEqual` weak, `NoError` and `NotNil` not-null weak, `Error` throws weak, `ErrorIs` throws strong, …); go-snaps `MatchSnapshot` is a snapshot; testify mock `AssertExpectations` and friends are `custom`/strong.
- A file importing testify has framework `testify`, otherwise `go-test`. Cases are `func TestX(t *testing.T)` with Go's naming rule (`Test` then a non-lowercase character). Subtests belong to their test function. Mocks are gomock/mockery constructors (`NewMockX(…)`) and `&MockX{}` literals; setup is `TestMain` and `t.Cleanup`; skips are `t.Skip*` with the first string argument as reason.
- Test files are `**/*_test.go`; everything under `testdata/` also counts as test material, since the go tool never builds it. A source file's test file is `{dir}/{stem}_test.go`; Go has no inline tests.
- `return-value` replaces each returned expression that could be nilable (names, calls, selectors, index and slice expressions, unary and composite expressions, type assertions, function literals, parentheses) by `nil`; literals, arithmetic, and comparisons are not sites. `++` and `--` are `arith-add-sub` sites. Negation applies to `if`, condition-only `for`, and three-clause `for` conditions. Statement deletion applies only to statements directly in a block.
- `ternary` and `catch` are not applicable: errors are values tested by `if`, which is a branch, and `recover` is an ordinary call.
- Method receivers need no `NormTree` override: receiver names normalize to `$id` and the pointer or value shape stays in the tree.

## Consequences

- `tests.quality` sees table-driven tests as one case with the subtests' assertions; per-subtest outcomes arrive with `TestResults` (P3.1), whose names include `TestX/sub`.
- A guarded failure call whose condition is not one of the recognized shapes is weak truthiness, so `tests.weakening` may under-rate custom comparisons such as `if !reflect.DeepEqual(got, want)`; adding such an idiom is a query pattern and a table row.
- A mutant returning `nil` for a non-nilable type does not compile; the mutation engine (P3.4) reports it as non-viable.
- Code under `internal/` is reported as public API and as export roots, which is conservative for `static.dead-code`.
