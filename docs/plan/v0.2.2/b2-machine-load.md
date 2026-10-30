# B.2 — Machine load and disk

Branch `b2-machine-load` (worktree `.claude/worktrees/b2-machine-load`), after B.1 is on `main`. Read `common.md`, the plan's `#### B.2`, P10 Notes "P10 — machine load", ADR-139 (dev setup), and `scripts/heavy.sh`. ADR number: **156**.

## The owner's ruling

Agent builds across worktrees share one `CARGO_TARGET_DIR` per machine, so cargo's lock serializes them and disk is held once. Cap `-j` at cores/2 for agent-driven builds. `make check` takes the heavy lock only around its build step. Worktree removal cleans any per-worktree target dir. ADR.

## What to build

1. **ADR-156** first. It decides where the shared directory lives (outside every checkout, such as `~/.cache/cupel/target`, keyed by nothing or by the toolchain), how `make dev-env` exports it, and how a by-hand `cargo` in a worktree picks it up, the same way it picks up the pinned tools today (ADR-139). It also records what sharing costs: two branches building alternately rebuild what differs, and cargo's lock makes one wait for the other. That is the point.
2. **The cap.** `make dev-env` and every `make` target that builds or tests set `CARGO_BUILD_JOBS` to half the cores and nextest's `--test-threads` likewise, for agent-driven builds. CI keeps all its cores: the variable is the dev environment's, not the workflow's.
3. **`make check` and the heavy lock.** Only its build step (`cargo build --workspace --all-targets` or the step that compiles) runs under `scripts/heavy.sh`. The tests then run outside the lock, against what was built. Check the lock's priority and turnstile rules still hold for a dogfood waiting behind it.
4. **Worktree removal.** Removing a worktree also removes any target directory that belongs to it alone. Choose the entry point (a `make` target, a `scripts/` command the integrator runs instead of `git worktree remove`, or both) and document it in the README's development section.
5. **The coverage build** (B.1's, per checkout or shared) and the mutation slots (`$TMPDIR/cupel-mutate`, ADR-150): say in the ADR whether they share too, and keep `make reclaim` and the 40 GB floor meaningful.
6. **Tests:** what can be tested in `make test` (the Makefile's variables, the scripts' behaviour on a temporary directory). Plus one real run: two worktrees building at once, shown serialized by cargo's lock, with disk held once.

## Leave alone

The product (`crates/**`), except where a test needs the shared directory; CI's workflows, except where a `make` target they call changes.
