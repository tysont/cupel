# CUPEL_DECISION_041: Scheduler v1: a per-language fact DAG, shared facts, and one result per check

**Date:** 2026-10-13 · **Status:** accepted; the per-language `skipped` entries of the Results bullet superseded by [064](CUPEL_DECISION_064.md) (one `checks[]` entry per check) · **Phase:** P2.1

## Context

Design §8.9 lists the scheduler's steps; §8.3.4–§8.3.5 say a check receives a `Facts` serving exactly its declared kinds, and that multi-language scopes run each check once per language present. P0.2 left `ProducerRegistry` with an erased `produce` returning `Produced` (ADR-023), and P1.1 noted that every producer that parses a file reports its parse errors, so the scheduler must de-duplicate them. What the design does not fix:

1. How a fact made once reaches several checks, given `Produced` owns a `Box<dyn Any>`.
2. Which producer makes a fact when several could (§8.3.3: config pin > toolchain order > first `Ready`).
3. How the per-language results of one check become the envelope's `checks[]`, which the design shows with one entry per check, and how a score is combined across languages.
4. What a check's status is when it emits findings but no gate exists yet.
5. How cancellation and failures surface per check.
6. What `--dry-run` estimates without history (step 4's "historical durations" need the store).

## Options

For (1): clone facts (every payload is `Clone`, but the erased box is not, and cloning `Symbols` per check is waste), or turn the box into an `Arc<dyn Any + Send + Sync>` once and let `Facts` hold `Arc`s. For (3): one entry per (check, language), which needs a new `lang` field on `CheckResult` and gives gates five scores to read, or one aggregated entry per check plus one `skipped` entry per skipped language, with the check itself saying how its per-language scores combine (a maximum for `static.complexity`, an `n`-weighted mean for ratios such as a mutation score). For (4): `pass` unless a gate fails (then a check with findings reads as passing), or `fail` when the check reports any finding of severity `warn` or `fail`, independent of the gate, which decides the envelope's status and exit code.

## Decision

- **The DAG.** For each selected check and each language in scope, the support matrix (ADR-015) decides: `supported` pairs become tasks, the rest become `skipped` entries with the matrix's reason, fact, tool, and `see: cupel describe matrix`. A task needs its manifest's `needs` plus each `wants` the language can produce. A fact for a language is made by the first registered producer for it whose own needs are producible and which is `Ready`, the rule the matrix uses; producer needs are resolved first, depth-first. Each `(fact kind, language)` is produced at most once per run and kept as an `Arc`, and each check gets a `Facts` built from exactly its declared kinds. Config pins and toolchain order join the rule when tool producers arrive (P3). Passing upstream facts into a producer joins `ProduceCtx` with the first producer that has needs (P3.4); no P1 producer has any.
- **Scope first.** `Symbols` is produced for every language in scope before anything else (and `NormTree` when `--symbols` names a hash), to resolve the symbol set (ADR-040, design §8.9 step 1); checks that need them reuse the same facts.
- **One file walk.** The CLI walks the tree once per run; every producer receives that file list in its `Scope` and filters it to its language.
- **Execution** is sequential over facts, then checks, in registry order and language-ID order; producers parallelize inside with rayon. The cancellation token is checked before every producer and every check. Work not started when it fires is a `skipped` entry with reason `timeout`; a check or producer that stops with `Cancelled` leaves its check `partial`. Any of these makes the run `partial`, exit 5, with all completed results kept (ADR-020 §5).
- **Findings.** A producer's findings (today only `lang.parse-error`) are kept once per finding ID across producers and languages. Checks' findings stream to `--events` as emitted and are sorted in the envelope by file, line, column, check, kind, and ID.
- **Results.** Each check contributes one entry per run: its per-language results combined (status: `error` > `partial` > `fail` > `pass`; findings and durations summed; scores combined by `Check::combine`, which defaults to the `n`-weighted mean and which `static.complexity` overrides with the maximum), followed by one `skipped` entry per skipped language. A check with no language in scope is one `pass` entry with no score. A check's status is `fail` when it reported any finding of severity `warn` or `fail`. Until gates exist (P2.9) the envelope's status is `pass` (exit 0) unless the run was cut short; `summary.gate` is null and `new`/`resolved` are 0 until baselines (P2.2).
- **Errors.** A check whose `plan` rejects its configuration fails the whole run as a usage error before any work starts (thresholds are configuration). A producer or check that fails otherwise makes its check `error` for that language; the run continues.
- **`next_actions`** are the findings ranked by severity (fail, warn, info), then tier (higher first), then envelope order, at most ten, each carrying the finding's ID, action kind, and action summary. De-duplication by action and location and `rerun` actions arrive with P2.9.
- **Events.** A real run emits `plan` (selected checks, estimate), `skip` for each skipped pair, `check.start` (targets summed over languages), `finding` as emitted, `progress` after each language, and `check.end`; the CLI adds `run.end`.
- **Estimates** come from the manifest's cost class until the store has history (P2.2/P4.3): per target, `quick` 1 ms, `standard` 100 ms, `deep` 1 s, `exhaustive` 10 s. A dry run resolves the scope (parsing only; it writes nothing) so its `would_run` targets are real symbol counts, lists `would_skip` with reasons, and lists the run record under `would_write` with a null diff.

## Consequences

- Two checks needing `Symbols` share one production; a test proves it with a counting producer.
- `CheckResult` keeps its shape; gates read one score per check. A plugin check (P5) must carry its combination rule in its manifest, since `Check::combine` is native-only.
- Per-language scores are not in the envelope; `explain <check-id>` (P2.2) can show them from the store.
