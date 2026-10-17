# CUPEL_DECISION_098: `gen.metamorphic`: the finding, its evidence and scope, the unconfigured skip, and what `--cases` and `--seed` do

**Date:** 2026-10-16 · **Status:** accepted · **Phase:** P5.5

## Context

ADR-097 puts relations in `[lang.<id>.relations]` and their evaluation in `engine.metamorphic`, which produces `RelationResults`. The check still has to decide what it reports and where, how a repository with no relations is told so, what tier its results carry, and what the generative flags do. `--seed N` is recorded in every envelope and reaches producers (ADR-078); `--cases N` ("generative case budget", design §6.4) has parsed since P0.3 and been ignored, pending P5.4 (P3 follow-up (9)).

## Options

**Skip with no relations.** (a) The check passes over zero relations: indistinguishable from a repository whose relations all hold. (b) The check reports a `metamorphic.missing` finding: noise in every repository that never meant to use relations, and a kind the plan does not list. (c) The engine is `unconfigured` until a relation is declared (design §8.3.5, ADR-015), so the scheduler skips the check with reason `unconfigured`, the configuration key, and `see: cupel describe matrix`, as it does for a coverage producer without a command (ADR-084 §3): nothing runs, and the envelope says what to set.

**Finding identity.** By the minimized input (a different finding for every input that breaks the relation, so a seed change re-files it), by the function's symbol hash (needs `Symbols` and a match from the harness's location to a symbol, and moves when the function is edited — which is when it is fixed), or by the relation's name and function (a subject finding, ADR-016: stable across seeds, inputs, and edits; a new relation or a renamed function is a new finding).

**`--cases`.** Leave it to P5.4, or wire it now: it is one value from the command line to every producer, exactly as `--seed` is, and `gen.metamorphic` is the first check that reads it.

**Tier.** Design §4 puts `gen.metamorphic` at tier 4 (generative, independent oracle). Unlike `gen.differential` it needs no oracle (the relation is its own), so nothing caps it (ADR-091 applies to checks that use an oracle).

## Decision

1. **`gen.metamorphic`** (`cupel-checks/src/gen_metamorphic.rs`): tier 4, cost `deep`, stability experimental, needs `relation-results`, no settings (relations and their budgets are `[lang.<id>.relations]`'s). It runs at `--depth deep` and above, or when selected with `--only`.

2. **One finding per violated relation**, kind `metamorphic.violated`, severity `fail`, confidence 1 (the input reproduces it), action `fix_bug` (priority 1) whose summary names the function, the relation, and the minimized input. Title: ``` `<function>` breaks the relation `<name>` ```. Identity: a subject finding with subject `"<name> <function>"`. Location: the function's definition as the harness found it, else none; no symbol. Repro: `cupel check --only gen.metamorphic --seed <seed>` (the inputs depend on the seed, the relation's name, and the case index alone, ADR-097 §3, so the same seed regenerates the violating case whenever the budget reaches it). Evidence (each evaluation as an object with exactly one of `value`, `raised`, `repr`, inlined, ADR-043):

```text
{relation, function, input_type, input, transformed, source_output, followup_output (null when the transform raised),
 relation_raised (null unless the relation raised), original_input, case, shrinks, seed}
```

3. **Scope.** A relation is in scope when the run's files are not narrowed, when its function's file is in scope, or when the harness could not locate the function. Relations out of scope are neither reported nor counted.

4. **Result.** `status: fail` with any violated relation in scope; score `{value: violated, unit: "violated", n: relations}`; metrics `violated` and `relations` (scope metrics, gate keys such as `"gen.metamorphic.violated" = "== 0"`), summed across languages.

5. **No relations, no run.** The engine is `unconfigured` with key `lang.<id>.relations` until one is declared for the language, so the check is skipped `unconfigured` for it.

6. **`--seed N`** fixes every relation's inputs (ADR-097 §3) and is in every finding's repro; without it the run draws a random seed, recorded in `run.seed`. **`--cases N`** is the case budget of every generative producer: `Job.cases` → `Focus.cases` (`None` without the flag), and the metamorphic engine generates `N` inputs per relation, over each relation's `cases` and the default of 100. The flag is no longer pending.

## Consequences

- A repository that declares no relations sees `gen.metamorphic` skipped at `deep`, never failed or passed; `describe matrix`, which knows no repository's configuration, shows `unconfigured(lang.python.relations)` for Python.
- The matrix harness now computes each fixture repository's support with its `cupel.toml`'s `[lang.<id>]` tables, as a run there does (`fixture_ctx` in `cupel-fixtures/src/matrix.rs`), and runs checks on declared facts with them; otherwise the metamorphic fixture would be `unconfigured` in the matrix and run in the golden.
- P5.4's `gen.property` reads the same `Focus.cases`; a property tool's own budget flag is its producer's mapping of it.
- Every relation is evaluated even when the scope excludes its function, because only the harness knows where a function is defined; the cost is the engine's, the report is the scope's.
- A violated relation stays one finding however many inputs break it; fixing the function for the reported input and rerunning reports the next minimized input under the same ID.
