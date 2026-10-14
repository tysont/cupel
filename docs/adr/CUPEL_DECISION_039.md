# CUPEL_DECISION_039: `Symbols` carries each symbol's parameter count, from `@symbol.parameter` captures

**Date:** 2026-10-13 · **Status:** accepted · **Phase:** P2.1

## Context

`static.complexity` reports `params.many` (design §5.1: "param count … per symbol"). A check sees only facts (design §8.3.1), and no fact in the catalog says how many parameters a function takes: `SymbolDef` has a name, namespace, kind, visibility, span, and body span. `NormTree` keeps the tree shape, but its labels are grammar node kinds (`parameters`, `formal_parameters`, `parameter_list`), so counting parameters there would make the check name a grammar, which the matrix rule forbids (plan §2.8).

Parameter lists differ by language in exactly the ways a check must not learn: Go groups names under one declaration (`a, b int`) and may omit names (`func(int)`); Rust and Python methods take `self` (and Python classmethods `cls`) inside the list; Go's receiver is a separate list; TypeScript wraps every parameter (`required_parameter`, `optional_parameter`); JavaScript and Python have rest, keyword-rest, defaulted, and destructured parameters.

This is not a new fact kind (no catalog row changes), but it changes the `Symbols` payload, which every language produces.

## Options

**Count the children of a captured parameter list, with table exclusions.** One `@symbol.parameters` capture per symbol pattern plus `[symbols]` keys for receiver kinds and names. It cannot express Go's grouped names without a further per-language rule, and every symbol pattern in every `symbols.scm` would need the extra capture.

**One capture per parameter (`@symbol.parameter`), attributed structurally.** Each language writes standalone patterns that capture one node per declared parameter, in its own grammar's terms (Go captures each `name` of a `parameter_declaration`, or the type when unnamed; Rust captures `parameter` and never `self_parameter`; Python excludes `self` and `cls` by predicate). A parameter belongs to the innermost symbol whose node contains it and counts only when it lies outside that symbol's body, so parameters of anonymous closures inside a body never count toward the enclosing function, and parameters of a named nested function count toward that function. Symbol patterns are untouched.

**Leave parameters to a later fact.** `params.many` would be `unsupported` everywhere, and the first slice would not deliver its fourth finding kind.

## Decision

- `SymbolDef` gains `parameters: u32`, the number of declared parameters, not counting an implicit or explicit receiver (`self`, `cls`, `this`, a Go receiver). `Symbols::VERSION` becomes 2.
- `symbols.scm` must capture `@symbol.parameter` (it joins the required captures of the contract); each capture is one parameter. Attribution is the structural rule above.
- Conformance: `conformance/symbols.toml` gains `parameters` (a function's count) and `receiver` (a method's receiver is not counted), mapped for all five languages.

## Consequences

- `static.complexity` reads `parameters` from `Symbols`; no check learns a grammar.
- Every `golden/facts/<lang>/symbols.json` gains the field; the `symbols` schema snapshot changes.
- A variadic parameter counts as one, a destructured parameter as one, Go's `a, b int` as two. A language added later must write the capture or fail the contract test.
