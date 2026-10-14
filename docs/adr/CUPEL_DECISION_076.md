# CUPEL_DECISION_076: Runner producers, the baseline test run, and exit 4

**Date:** 2026-10-14 · **Status:** accepted · **Phase:** P3.1

## Context

P3.1 makes `TestResults` real (ADR-075). Design §8.9 says the baseline test run happens once, in the real tree, before any mutant, and that a red baseline is exit 4; §6.6 says exit 4 is mandatory and no scores may be produced; §8.3.3 orders producer selection "a config pin in `[lang.<lang>]` > toolchain-table default order > first `Ready`". None of these says where the baseline lives (a producer, a check, the CLI), how a pin is spelled, how a runner process is run and cancelled, what an exit-4 envelope carries, or how to prove the contract while no shipped check needs `TestResults` (`tests.mutation` is P3.4).

## Options

**Baseline as a producer concern.** Each runner refuses to produce a red fact. Then a red suite is indistinguishable from a missing tool, and the refusal could not stop the whole run.

**Baseline in the CLI.** The CLI would have to know which checks need test execution, which is the scheduler's fact DAG.

**Baseline in the scheduler.** Before any check runs, the scheduler produces `TestResults` for every language whose selected checks need it, directly or through a producer's needs, and refuses the run when one is red.

For proving exit 4 through the CLI with no shipped check that needs `TestResults`: a hidden debug variable that registers a check (as `CUPEL_DEBUG_PANIC` and `CUPEL_DEBUG_SLEEP_MS` exercise exits 6 and 5); a test-only check compiled into the binary behind a feature (every registry-derived test and the README tables would see it); or running the CLI's own entry point in-process from a test binary that submits a test-only check through `inventory` (ADR-074), as `cupel-adapt`'s registration test submits a producer.

## Decision

1. **Selection** (`ProducerRegistry::candidates`, used by the scheduler and the support matrix alike). A fact kind may have a pin key: `TestResults` → `runner`. `[lang.<id>] runner = "<name>"` keeps only the producer whose ID is `<name>` or `tool.<name>` (`runner = "unittest"` selects `tool.unittest`); a pin that names no producer of the fact for that language makes it `unconfigured` with key `lang.<id>.runner`. Without a pin, producers whose `Producer::detected` holds come first, in registration order, then the rest. `cupel_adapt::register` applies tool producers in toolchain-table order (then ID), so registration order is the table's default order, and a runner's `detected` is its table row's detection rules at the repository root. `Ctx` carries the `[lang.*]` tables for this.
2. **Availability** of a runner is its program on `PATH` (`MissingTool`, so the matrix shows `unavailable(<tool>)`); JavaScript runners also accept `node_modules/.bin/<tool>` in the repository root. `Producer::tool` names the program and its version command, which `doctor` uses.
3. **Processes** run through `cupel_core::Process`: program, arguments, directory, extra environment, optionally stderr merged into stdout in order; output captured on threads; the child polled against the run's `Cancellation`, so `--timeout` kills it (its whole process group on Unix) and the producer returns `Error::Cancelled` (exit 5).
4. **Upstream facts.** `ProduceCtx` gains `upstream`, the facts the producer's `needs` named, produced first for the same language. Runners need `ProjectInfo` and run once per project root of their language (the repository root when there is none).
5. **The baseline is the scheduler's.** `Scheduler::run`, after planning and before running any check, produces `TestResults` for each language in which a selected task's needs reach `TestResults` (through check needs and producer needs). A red result (ADR-075 `is_green`) ends the run: the outcome carries `refused: Some(Unevaluable { lang, results })` and no check result, because no check ran, so no score exists. Every production of `TestResults` goes through the same gate, which withholds a red fact (`AbsentFact`), so a later consumer cannot see one either. The refusal is part of the outcome, not a `cupel_core::Error` variant, so `error.rs` stays free of fact payloads (an `Error` variant holding `TestResults` made `error → test_results → id → error` a module cycle).
6. **Exit 4.** The CLI maps `Outcome::refused` to `status: error`, `exit_code: 4`, an empty `checks[]`, and one `fail` finding per reason: `baseline.test-failed` per failed or errored test (subject `<lang>:<test id>`; evidence `lang`, `runner`, `test`, `file`, `outcome`, `output`; action `fix_bug`) and `baseline.run-incomplete` per invocation that did not complete (evidence `dir`, `command`, `exit_code`, `output`). These findings are the envelope's error list: the envelope has no separate `errors[]` (design §7.1 has none; an error envelope's findings already are its errors, ADR-018).
7. **The store.** Every `TestResults` a run produced, green or red, is indexed in `test_results` (migration 2: `run_id`, `lang`, `runner`, `test_id`, `file`, `outcome`, nullable `duration_ms`) when the run is recorded. The table was empty before, so migration 2 recreates it.
8. **Proof.** `cupel-cli` gains a library target holding everything but `main`, so a test binary can call `cupel_cli::run` — the same function the binary calls with the process's arguments — after submitting a test-only check (`needs = [TestResults]`) with `inventory::submit!`. The test runs `check --only <that check>` on a fixture whose `cupel.toml` names a runner, with the real runner on `PATH`. No hidden variable or flag is added; the shipped binary's registries are unchanged.

## Consequences

- `tests.mutation` (P3.4) and `tests.flaky` (P3.5) get exit 4 by declaring `needs` that reach `TestResults`; nothing else is needed.
- A JavaScript-and-TypeScript project in one root runs its runner once per language (two fact keys); collapsing them by root is left for when it matters.
- `[lang.<id>] test`, the command `init` writes, is still what `doctor` probes; runner producers build their own command lines (they need machine-readable output) and do not run it. A configured command that differs from the runner's is not reconciled yet.
- Detection runs at the repository root only; a monorepo whose JavaScript root is in a subdirectory falls back to registration order there.
- The library target is internal: `cupel_cli` exposes `run`, `Environment`, `Output`, and `capture_panics`, nothing else.
