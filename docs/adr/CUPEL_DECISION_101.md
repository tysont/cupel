# CUPEL_DECISION_101: `static.api-surface`: the public API diffed across a range, `api.breaking` and `api.widened`

**Date:** 2026-10-16 · **Status:** accepted; supersedes the `no-breaking` clause of [060](CUPEL_DECISION_060.md) §6 · **Phase:** P6.2

## Context

Design §5.1 lists `static.api-surface` as the "public API diff vs base: removed/changed signatures, visibility changes" with kinds `api.breaking` and `api.widened`, "build + wrap (`cargo-semver-checks`, `api-extractor` where present)". Plan P6.2 narrows it: `needs = [ApiSurface]` for base and head, a pure diff, no new facts. `ApiSurface` (P1.8) lists each language's public items with a path, a kind, a normalized signature (parameter names become `_`, layout disappears), a file, and a span; private items are not in it at all. ADR-056 supplies the base side of a range: the same producers over the base versions of the changed source files, and the range itself. Nothing says how an item on one side is matched to the other, what makes a change breaking when the check cannot read a signature's language, what a visibility change looks like in a fact that holds only public items, what severity and score the kinds carry, how a finding about an item that no longer exists is located and identified, or what the gate operator `no-breaking` (design §6.7), left `unsupported` by ADR-060 §6 until this step, evaluates.

## Options

**Matching.** By path and kind misses a function that became a constant under the same name (still a break for callers); by path alone keeps it. Several items can share a path (TypeScript overloads, a Python property's getter and setter, a Go method set across files), so a path's side is a multiset of `(kind, signature)` rather than one item. By file as well as path would turn a Rust module moved from `a.rs` to `a/mod.rs` into a removal and an addition, though its callers see nothing.

**What is breaking.** (a) Any change to a path's signatures: simple and language-neutral, but adding an overload (TypeScript) or a second accessor would be called breaking. (b) Base's multiset contained in head's: every item callers could use before is still there with the same signature, so only additions happened; anything else lost or changed an item someone may call. Optional parameters (`def f(x, y=0)` from `def f(x)`) are not breaking in Python or TypeScript but are in Rust and Go; telling them apart needs the signature's grammar, which only a language knows, and `ApiSurface` does not encode it. (b) is the language-neutral rule; finer compatibility is a semantic field a producer would add to the fact (plan §2.8), not a match on a language in the check.

**Visibility.** `ApiSurface` holds only public items, so narrowing an item's visibility is its removal from the surface and widening is its addition. A separate kind for each would need private items in the fact.

**Wrapping `cargo-semver-checks` and `api-extractor`.** Each sees more than a normalized signature (trait bounds, `#[non_exhaustive]`, declaration merging) but needs a built crate or a `.d.ts` rollup, runs for one ecosystem each, and would be a second producer of a new fact kind. The plan's step is the pure diff.

**`no-breaking`.** (a) A `breaking: bool` on `FindingKindSpec`: explicit, but the gate is evaluated over the envelope alone, which carries no manifests, and the field would change every manifest's schema and description. (b) A `breaking` metric compared with zero: but metrics ignore suppressions, so an accepted break could never be suppressed past the gate. (c) A naming rule for kinds, `<group>.breaking`, read from the findings themselves: suppressions already turn a finding `info`, which `none` ignores the same way, and the configuration loader, which has the manifests, can refuse the operator on a check that declares no such kind.

**Severity.** `fail` (as `tests.weakening`) makes every intended break a gate failure until suppressed; `warn` reports it without deciding the change is wrong, and the check's status still fails so a gate on `static.api-surface = "none"` holds the line where a project wants it.

## Decision

1. **`static.api-surface`**: tier 0, cost `quick`, needs `ApiSurface`, `base_needs = [ApiSurface]` (ADR-056), so it runs only under `--changed` or `--base` and is skipped `needs-range` otherwise; no settings (any key is a configuration error), stability `experimental`.
2. **Matching** is by path. Each path's side is the multiset of its items' `(kind, signature)`. The base side covers the changed files that existed at the base commit; the head side covers the changed files at head (the range's scope), so an item in an unchanged file is on neither side and an item moved to another changed file under the same path is unchanged.
3. **`api.breaking`** (severity `warn`, confidence 1, action `review`): a path on the base side whose multiset is not contained in head's. `change` is `removed` when the path is absent at head (deleted, renamed, moved to another path, or made private) and `changed` otherwise (a signature or kind lost). **`api.widened`** (severity `info`, confidence 1, action `review`): a path absent at base (added, renamed into, or made public), `change: "added"`, or a path whose base multiset is a strict subset of head's (an overload or accessor added), `change: "extended"`. Evidence for both: `{path, change, base: [{kind, signature}], head: [{kind, signature}]}`, each side in fact order.
4. **Identity and location.** A finding's ID is `FindingId::for_subject(check, kind, path)`, stable across line moves. It is located at the path's first head item, or for a removal at its first base item (its file and line at the base commit, as `tests.weakening` locates a deleted test). Repro: `cupel check --only static.api-surface --base <commit>`.
5. **Result.** Score `{value: breaking paths, unit: "breaking", n: paths compared}` (paths on either side), metrics `breaking` and `widened`, both summed across languages; status `fail` when anything broke, else `pass`.

6. **`no-breaking`** (replacing ADR-060 §6's clause): a finding is *breaking* when the last segment of its kind is `breaking`. The rule passes when its check reported no breaking finding above `info` (suppressed findings are `info`), `actual` the number it reported, and is `skipped` like every rule when the check did not run. A `no-breaking` rule on a check whose manifest declares no breaking kind is a configuration error at load (exit 2), so it can no longer fail as `unsupported`; the `unsupported` rule result stays in the schema for operators added later.

## Consequences

- `[gate.changed] "static.api-surface" = "no-breaking"` (design §6.7's example) now evaluates; `[gate.tree]` already refuses a check that needs a range (ADR-060).
- One check serves every language with an `ApiSurface` producer; a language whose signatures normalize more (defaults, generics) gets fewer false breaks without a change here.
- Optional and defaulted parameters added to a function are reported as `api.breaking` in every language; a producer-supplied compatibility field is the follow-up that would refine it.
- `cargo-semver-checks` and `api-extractor` are not wrapped. A producer that wraps one would provide a new fact (or a richer `ApiSurface`) and needs its own ADR.
- An item cannot move into an unchanged file: adding it there changes the file, which puts the file in scope.
- The matrix runs fixtures as whole trees, so its goldens for this check record the `needs-range` skip; the findings are proven by a test that builds a git history per language and runs `--base` and `--changed` through the binary, as for `tests.weakening`.
