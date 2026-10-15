# CUPEL_DECISION_081: Installing tools: where install commands live, who may install, and what is reported

**Date:** 2026-10-15 · **Status:** accepted; supersedes §6 (`--fix`) of [022](CUPEL_DECISION_022.md) · **Phase:** P3.7

## Context

Design §6.1 rule 10 and §6.3 make `--install | --no-install` global flags ("auto-install producer tooling (default: --no-install unless TTY)"), §6.5 says `doctor --fix --install` attempts installs, §6.8 gives every dry-run plan a `would_install` list, and §8.3.3 gives `Availability::MissingTool` an `install` that is "a command or hint". The decisions log adds "Auto-install: off unless TTY — agents must not mutate someone's toolchain silently". Left open:

1. Where each tool's install command is written, given that the same program (`pytest`, `vitest`) is installed differently depending on the producer that needs it (`pytest-cov`, `@vitest/coverage-v8`) and on the project's package manager.
2. How a hint differs from a command: `go`, `cargo`, and `python3` are toolchains, not tools Cupel should install, and a hint must never be run.
3. Where an install puts the tool (`pip`'s prefix, `cargo install`'s root, a global or a project `npm` install).
4. Which verbs install, and when "unless TTY" asks rather than installs.
5. How an install is run, and what the envelope says about it.
6. Whether an install that edits `package.json` or `pyproject.toml` needs `--write` (rule 6: nothing is written outside `.cupel/` without it).

## Options

**Where commands live.** (a) Each producer hard-codes its command in its `RunnerSpec`/`CoverageSpec`. The package-manager choice (npm, pnpm, yarn; pip, uv, poetry) is already toolchain-table knowledge (its launchers), so producers would repeat the launcher detection. (b) The toolchain table, on the producer's row, as a template filled with a launcher value, the way default commands already are. One file then holds everything Cupel knows about a tool: detection, commands, installs, and `describe language` can show them.

**Hints.** (a) Keep `install: Option<String>` and let producers put prose in it: `doctor` could not tell prose from a command without guessing. (b) `install: Option<Install>` with `Install::Command(String)` and `Install::Hint(String)`: one type change in `cupel-core`; every existing `install: None` still compiles.

**Prefix.** (a) Cupel chooses a prefix (`--root`, `--target`, `--prefix`) and puts it on `PATH`: Cupel would own an installation area nobody else knows about, and every runner would need it on its `PATH`. (b) The environment chooses: `cargo install` installs into `CARGO_INSTALL_ROOT`/`CARGO_HOME`, `python -m pip` into the interpreter `PATH` finds (an activated virtual environment), and the JavaScript package managers into the root's `node_modules`. Cupel's runners find tools exactly where the user's shell does, and a test controls the prefix through the same variables.

**Verbs.** (a) Only `doctor --fix`. (b) `doctor --fix`, plus `check` and `survey` before they schedule, since §6.3 makes the flags global and §6.8 gives `check --dry-run` a `would_install`. Other verbs run no tool producers that install could help, so they accept the flags and install nothing.

## Decision

1. **The table holds install commands.** A runner or coverage row of `toolchain.toml` may carry `install`, a command template filled with the launcher detected at the repository root (the launchers gained an `add` value: `npm install --save-dev`, `pnpm add --save-dev`, `yarn add --dev`, `python -m pip install`, `uv add --dev`, `poetry add --group dev`). Rows: `tool.cargo-nextest` `cargo install cargo-nextest --locked`; `tool.cargo-llvm-cov` `cargo install cargo-llvm-cov --locked && rustup component add llvm-tools-preview`; `tool.pytest` `<add> pytest`; `tool.pytest-cov` `<add> pytest-cov`; `tool.coverage-py` `<add> coverage`; `tool.vitest` `<add> vitest`; `tool.vitest-coverage` `<add> vitest @vitest/coverage-v8`; `tool.jest` and `tool.jest-coverage` `<add> jest`; `tool.c8` `<add> c8`. A new `[toolchains]` table maps the programs Cupel does not install (`cargo`, `go`, `python3`) to a hint. `cupel_adapt::toolchain::install(producer, program, root)` returns the row's command, else the program's hint, else nothing; every tool producer fills `MissingTool.install` through it. `describe language` shows each producer's `install` template.
2. **`Install::{Command, Hint}`** in `cupel-core` (`producer.rs`); `MissingTool { name, install: Option<Install> }`. Only a `Command` is ever run or planned; a `Hint` becomes the `doctor.tool-missing` finding's fix.
3. **Cupel chooses no prefix** (option b). The command runs with Cupel's own environment, so `PATH`, `VIRTUAL_ENV`, `CARGO_HOME`, and `CARGO_INSTALL_ROOT` decide where the tool lands.
4. **Permission.** `--no-install`: never. `--install`: every planned install, without asking. Neither: ask when stdout is a terminal, else never (rule 10). Asking writes `cupel: install <tool> with `<command>`? [y/N] ` to stderr and reads one line from stdin; `y` or `yes` accepts, anything else (end of input included) declines.
5. **Which verbs install.** `doctor --fix` installs the missing tools `doctor` found that have a command; `doctor` without `--fix` installs nothing whatever the flags. `check` and `survey` install, before scheduling, the missing tools of the producers the scheduler would choose for their selected checks' needs in the scope's languages (doctor's resolution, required tools only). Other verbs accept the flags and install nothing.
6. **Running.** Each install is `sh -c <command>` in the repository root, one at a time, stdout and stderr captured separately, under the run's cancellation (`--timeout`). After the installs, `doctor` probes every tool again, and `run.toolchain` and the `doctor.tool-missing` findings reflect the probes after installing.
7. **Findings** (check `install`, subject the tool): `install.succeeded` (`info`, evidence `tool`, `command`, `version`) when the command exited 0 and the probe now finds the tool; `install.failed` (`fail`, `install_tool`, evidence `tool`, `command`, `exit_code`, `stderr`, the last 40 lines) when it did not exit 0, or exited 0 but the tool is still not found; `install.skipped` (`info`, evidence `tool`, `command`, `reason`) when `doctor --fix` installed nothing because of `--no-install`, a stdout that is not a terminal, or a declined question (`check` reports only declined questions; its `unavailable` skips already name the tool). A failed install keeps `doctor` at exit 3. `doctor.tool-missing` gains `install` evidence (the command or hint, or null), and its fix names the command and `cupel doctor --fix --install`.
8. **`would_install`** lists, in a dry run, the installs the same command line would attempt under the same permission: `doctor --fix --dry-run` and `check`/`survey --dry-run` fill it; with permission "never" it is empty. `doctor --dry-run` without `--fix` stays an empty plan (ADR-022 §6), now apart from `would_install` under `--fix`.
9. **`--write` is not needed.** An install is the user's package manager acting with the user's explicit permission (`--install` or a confirmed question); rule 6 governs the files Cupel itself writes.
10. ADR-022 §6 (`doctor.fix-pending`) is replaced by §5–7 here.

## Consequences

- Adding a tool producer adds its `install` beside its detection rules; `cupel-adapt/tests/installs.rs` fails when a registered tool producer's program has neither a command nor a hint.
- `pip` installs into whatever `python` is first on `PATH`. Outside a virtual environment that is the user's interpreter (or an "externally managed" refusal, which becomes `install.failed` with pip's stderr).
- `uv add` and `poetry add` install into the project's virtual environment, which is not on `PATH` unless activated, so `doctor` can report `install.failed` ("still not found") after a successful `uv add` when the environment is not activated. The fix is to run Cupel inside the environment, as its runners already require.
- `PerTestMap` producers (`tool.*-per-test`) still report `install: None`; they need the same programs as their runners, whose rows `doctor` installs first.
- An install changes the repository's dependency files for the JavaScript and uv/poetry launchers; that is visible in `git status` after `--install`.
