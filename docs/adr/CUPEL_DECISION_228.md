# CUPEL_DECISION_228: `init` writes coverage.py's command for a pytest root that does not declare pytest-cov

**Date:** 2026-11-14 · **Status:** accepted · **Phase:** v0.3.0, item 7 (Q-py-1) · The Python analogue of [197](CUPEL_DECISION_197.md); [208](CUPEL_DECISION_208.md) unchanged

## Context
Validating v0.2.9 on encode/httpx, `init` wrote `coverage = "python -m pytest --cov=. --cov-report=lcov:{out}"` (`tool.pytest-cov`) although httpx declares no pytest-cov: its `requirements.txt` pins `pytest` and `coverage[toml]==7.10.6`. The survey then refused (ADR-076) on `pytest: error: unrecognized arguments: --cov=. --cov-context=test`. V6 made `doctor` name the missing plugin, and left which row `init` picks to the owner (NOTES Q-py-1: "recommend coverage.py (`coverage run -m pytest`), as ruling 1 did for c8 vs node --test").

The owner's ruling: "init uses coverage.py when pytest-cov is not declared."

Open: what "declared" means, what the coverage.py command is, and what a run does with a `cupel.toml` that already holds pytest-cov's command.

## Options
1. **Declared.** (a) Installed in the interpreter (doctor's V6 probe): `init` would depend on the machine it runs on, and the written file would differ between a developer's venv and CI. (b) Named in a manifest Cupel already reads for the pytest runner: `pyproject.toml`, `setup.cfg`, or `requirements*.txt`, in the root or a member, as the `pytest` and `pytest-benchmark` rules read them.
2. **How the table says it.** (a) A detection rule on `tool.pytest-cov`: detection also decides at run time which coverage producer applies, so a project that installs pytest-cov without declaring it (httpx's venv after V6's advice) would lose coverage for its configured `--cov` command. (b) A `defer` rule, which only `init`'s selection reads for coverage producers (ADR-162, as ADR-197 defers Node's coverage to c8): `tool.pytest-cov` is set aside where no manifest mentions `pytest-cov`, so selection falls to the next row with a command for pytest. That needs a rule that holds when another does not.
3. **The command.** (a) `<m> coverage run -m pytest && <m> coverage lcov -o {out}`, the unittest row's form with the runner swapped: LCOV, which `tool.coverage-py` already reads. (b) With `--source=<packages>`: the table's `=<packages>` empties to a bare option without packages (ADR-208), and a bare `--source` takes the next word as its value, so the command would break for a project whose coverage configuration chooses its files.

## Decision
1. **(b).** pytest-cov is declared when `pyproject.toml`, `setup.cfg`, or a `requirements*.txt` of the root or a member mentions `pytest-cov` as a whole package name.
2. **(b).** Detection rules gain `{ not = <rule> }`, which holds when its rule does not. `tool.pytest-cov` has `defer = [{ not = { mentions = "pytest-cov", in = ["pyproject.toml", "setup.cfg", "requirements*.txt"] } }]`; `tool.coverage-py` gains a command for `tool.pytest`.
3. **(a).** A pytest root without pytest-cov gets `coverage = "<m> coverage run -m pytest && <m> coverage lcov -o {out}"` (`python -m`, `uv run python -m`, or `poetry run python -m`). With pytest-cov declared, `init` writes `<m> pytest --cov=<packages> --cov-report=lcov:{out}` as before.
4. **At run time** the configured command decides: `tool.pytest-cov` runs only a command with a `--cov` option (`--cov` or `--cov=…`), so the coverage.py command goes to `tool.coverage-py`, and a `cupel.toml` holding pytest-cov's command keeps it whatever the manifests say.

## Consequences
- httpx's `init` writes `python -m coverage run -m pytest && python -m coverage lcov -o {out}`, which runs with what it declares.
- coverage.py without a `--source` lists only the files the suite loaded (ADR-152), as for unittest; a file no test imports is `coverage.unloaded-file`.
- The coverage.py command does not fuse with `tool.pytest-cov-contexts` (ADR-084): such a root runs the suite for its results and again for coverage, and its per-test map, which needs pytest-cov's contexts, is unavailable without the plugin, as before.
- `describe language python` shows the `defer` rule in words: `not (`pyproject.toml` or `setup.cfg` or `requirements*.txt` in the root or a member mentions `pytest-cov`)`.
- Fixtures: `repos/detect/python-coverage` (httpx's `requirements.txt`) and its `init --dry-run` golden; `repos/detect/python-pyproject` and `repos/coverage-config/python` declare pytest-cov, so their goldens and tests keep pytest-cov's command; `repos/detect/monorepo`'s `ml` root, which declares no pytest-cov, now gets coverage.py's.
