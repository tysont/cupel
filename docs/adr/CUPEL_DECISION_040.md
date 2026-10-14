# CUPEL_DECISION_040: Scope resolution and check selection

**Date:** 2026-10-13 · **Status:** accepted · **Phase:** P2.1

## Context

Design §6.4 gives the scope flags (`--changed`, `--base/--head`, `--paths`, `--symbols`), the rule `--symbols ⊂ --paths ⊂ --changed/range ⊂ tree` with intersection, and "an empty `--changed` returns `pass` with `scope.symbols: 0`". It gives the selection flags (`--depth`, `--only/--skip` with IDs or groups, `--tier N+`) and §5.8 maps depth to cost classes. It does not say what a path pattern is, how `ns::name` meets each language's separator, what happens when a pattern or selector matches nothing, how `--only` interacts with `--depth`, which `scope.mode` a combination reports, or what `scope.roots` lists.

## Options

**Patterns that match nothing.** Silently empty (a typo in `--paths scr/` then reports `pass` with nothing checked, which an agent loop reads as success) or a usage error (a legitimately empty directory errs).

**`--only` and depth.** Intersect `--only` with the depth's cost classes (`cupel check --only tests.mutation` at the default depth works, but `--only gen.fuzz` without `--depth deep` silently runs nothing), or let an explicit `--only` select regardless of depth.

**`ns::name`.** Require each language's own separator (`ledger.Account.Deposit` for Go, `account::Account::deposit` for Rust), or accept `::` and `.` interchangeably.

## Decision

- **Files.** The tree is the language walk (gitignore-aware, hidden and generated directories skipped). `--paths` patterns are relative to the repository root (`./` and a trailing `/` are dropped; `.` is the whole tree). A pattern matches a file that equals it, a file below it as a directory, or, when it contains `*`, `?`, `[`, or `{`, a file its glob matches (`*` does not cross `/`; `**` does). Patterns are unioned, then intersected with the range.
- **Ranges.** `--changed` is the files changed between the merge base of the default branch (ADR-002) and `HEAD`, plus the working tree (staged, unstaged, untracked and not ignored). `--base REF` is the files changed between `REF` and the working tree; with `--head REF` between the two commits. The files analyzed are always the working-tree versions. `--head` without `--base`, `--changed` with `--base`, a range outside a git repository, an unknown ref, and a missing default branch are usage errors (exit 2) naming the fix.
- **Symbols.** `--symbols` selectors are a structural hash (`sh_…`, matched against `NormTree`) or a display name, compared with `::` and `.` treated as the same separator. Symbols are resolved after the files: a run produces `Symbols` (and `NormTree` when a hash is given) for the files in scope and keeps the matching symbols.
- **Nothing matched.** A `--paths` pattern that matches no file of the tree is a usage error. A `--symbols` selector that matches nothing is a usage error unless a range is given, since an unchanged symbol is then legitimately out of scope. An empty range is not an error: it is a `pass` with `scope.symbols: 0`.
- **`scope.mode`** is `changed` for `--changed`, `range` for `--base`, else `symbols`, else `paths`, else `tree`: a range decides which gate applies (P2.9), so it wins. `scope.files` counts files in scope, `scope.symbols` the symbols in scope, `scope.languages` the languages that own at least one file in scope, `scope.roots` the detected project roots of those languages (`.` when none is detected).
- **Selection.** `--only` and `--skip` take check IDs or groups (the ID's first segment); an unknown one is a usage error listing the IDs and groups. Without `--only`, a check runs when its cost class is at or below the depth (§5.8). An explicit `--only` selects regardless of depth: naming a check is the strongest statement of intent. `--skip` and `--tier N` / `--tier N+` (tier ≥ N, N in 0–6) then filter. Selected checks keep registry order.

## Consequences

- An agent's typo in `--paths` or `--symbols` is an exit-2 envelope, never a vacuous pass.
- `cupel check --only <id>` always runs `<id>` when the support matrix allows; `--depth` matters only for group and default selection.
- Range scopes analyze the working tree, not the checked-out `--head`; checks that need the base side (P2.7 `tests.weakening`) read it through their own producers.
