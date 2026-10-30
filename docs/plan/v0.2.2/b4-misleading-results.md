# B.4 — Two misleading results

Branch `b4-misleading-results` (worktree `.claude/worktrees/b4-misleading-results`), after B.3. Read `common.md`, the plan's `#### B.4`, ADR-078 (the flake engine), ADR-050 and ADR-136 (clones), ADR-138 (the external dogfood method), and P10 Notes "P10 — trivial setters as clones". ADR number: **158**.

## The owner's ruling

- `tests.flaky` on a nextest project reports status `skipped` with a reason, never `error`.
- `static.clones` skips forms whose body is a single statement, and the default `min_nodes` rises so trivial accessors and setters cannot pair. Re-run the external dogfood fixtures to confirm no true positive is lost.
- Each with a fixture case.

## What to build

1. **`tests.flaky` on nextest.** Today the flake engine (`crates/cupel-adapt/src/flake/mod.rs`) returns `Error::InvalidValue` when the baseline's runner is `tool.cargo-nextest`, whose ordering is `Fixed`: nextest runs each test in its own process, so order cannot matter (ADR-078). The check becomes `skipped` with the runner named, through the existing skip mechanism (a `SkipReason`, likely `unsupported` with the fact and the runner, or a new one, decided in ADR-158), and no error. Fixture: a Rust fixture whose `[lang.rust] runner` is nextest, with a golden showing the skip.
2. **`static.clones`.** A form whose body is one statement is not a clone candidate. Raise the default `min_nodes` so `self.x = Some(x); self` and its kin cannot pair (P10's case was 22 nodes and 4 lines). Pick the value from the data: the node counts of the trivial pairs on Cupel's tree and of the true positives in the external dogfood reports.
3. **The external dogfood fixtures.** Re-run `static.clones` with the new defaults on the repositories of `docs/dogfood/` (ADR-138's method, through `scripts/heavy.sh`, the clones only, so no suite runs). Compare with each report's clone rows: every row judged a true positive must still be found. Record the before/after counts per repository in ADR-158.
4. **Cupel's own tree:** `[gate.tree] static.clones.pairs <= base` and `clusters <= base` stay as they are. The base falls with the new defaults, which the ADR notes.

## Leave alone

The clone detector's similarity measure and threshold (ADR-136), and the flake engine's behaviour on runners it does rerun.
