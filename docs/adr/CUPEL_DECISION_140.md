# CUPEL_DECISION_140: Cupel's own tree gate ratchets against a committed baseline

**Date:** 2026-10-20 · **Status:** accepted · **Phase:** P9

## Context

ADR-130 gives a gate rule a `base` operand: the metric's value in the reference run, which for a tree run is the store's `default` baseline. P9.1 meant Cupel's own clone ratchets (`static.clones.pairs`, `static.clones.clusters` in `[gate.tree]`) to read `<= base`, but left them as literals (`<= 169`, `<= 19`) because a fresh checkout, as every CI runner is, has no fact store, so a `base` rule there is skipped for want of a reference and fails the gate. The reference has to come from somewhere every checkout can reach.

## Options

**A cache of the store across CI runs.** `actions/cache` could keep `.cupel/cupel.db` between runs of the `dogfood` job, keyed by commit with a `main` restore key, and the job could set the baseline after each green run on `main`. The ratchet would then move by itself. But the reference would live only in that cache: a local tree dogfood would compare with whatever the local store holds, so a merge could pass locally and fail on `main`, and a cache evicted after a week of silence would skip the rules and fail the gate until someone bootstrapped it by hand.

**A committed baseline.** `cupel baseline export` writes the baseline's run as JSON and `cupel baseline import FILE --write` stores it and points the baseline at it, in any store, idempotently. A run of the whole tree on `main`, exported to `.cupel/baseline.json` and committed (the rest of `.cupel/` stays ignored, as `suppressions.toml` already is), is the same reference in every checkout: the tree dogfood imports it before it grades, so a fresh store has it, and a developer who removes clone groups refreshes it with `make baseline` in the same commit, where the review sees the ratchet move. The file is about 1.5 MB (the run's 1 100 findings), which git stores as a 100 KB delta per refresh; refreshes are occasional. It also gives every tree run on `main` a `new_since_baseline` against the last refresh.

## Decision

1. `.cupel/baseline.json` is committed: the `default` baseline's run, a tree run on `main` that `make baseline` exports after a green `make dogfood DOGFOOD_SCOPE=tree` (`cupel baseline set --write`, then `cupel baseline export .cupel/baseline.json --write`).
2. `make dogfood DOGFOOD_SCOPE=tree` runs `cupel baseline import .cupel/baseline.json --write` before the gate, so the reference is the committed file in every checkout, local or CI.
3. `[gate.tree]` reads `"static.clones.pairs" = "<= base"` and `"static.clones.clusters" = "<= base"`; the literals are gone. `crates/cupel-cli/tests/dogfood.rs` pins the recipes, the rules, and that the committed file is a clean tree run on `main` with the clone metrics.

## Consequences

- A tree run compares with the committed reference, never with a value another machine happened to hold; a merge that passes the tree dogfood locally passes it on `main`.
- Removing clone groups is a two-line change plus `make baseline`; adding one fails the tree gate until it is removed or the baseline is refreshed deliberately, which the diff shows.
- The changed gate's `static.clones` rule stays `none`: a range's recomputed base scores whole files while the current side scores the changed lines (P9.1 open item 2), so `<= base` would not compare like with like there; follow-up item 18 covers pairs that existed at the base.
- The baseline's deep metrics (coverage, CRAP, property counts) are available to `base` rules too; they stay literal until a reason to ratchet them appears.
