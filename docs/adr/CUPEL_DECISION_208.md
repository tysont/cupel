# CUPEL_DECISION_208: A Python project that chooses its coverage files gets a bare `--cov`

**Date:** 2026-11-13 · **Status:** accepted · **Phase:** v0.2.9, item V7 · Details [152](CUPEL_DECISION_152.md) (the Python root's coverage source)

## Context
ADR-152 made the Python root's `packages` its coverage source, `src` or `.`, so `init` writes `pytest --cov=<source> …`, the joint `tool.pytest-cov-contexts` passes `--cov=<source>` when the configured command names none, and coverage.py lists every file under the source, loaded or not. The v0.2.9 validation on encode/httpx found the case ADR-152 did not consider: the project configures coverage itself. httpx's `pyproject.toml` has `[tool.coverage.run] include = ["httpx/*", "tests/*"]` and `[tool.pytest.ini_options] filterwarnings = ["error", …]`. With a source, coverage.py ignores `include` and warns `--include is ignored because --source is set (include-ignored)`; pytest turns the warning into an error, so `init`'s coverage command and the joint both failed before any test ran.

coverage.py reads its run settings from `.coveragerc` (`[run]`), `setup.cfg` and `tox.ini` (`[coverage:run]`), and `pyproject.toml` (`[tool.coverage.run]`). Three of them choose the files to measure: `source` and `source_pkgs` (directories or packages, every file under them listed, loaded or not) and `include` (patterns, only loaded files that match listed). Any other key (`branch`, `omit`, `data_file`) leaves the choice to the command.

Open: which projects count as choosing their files, what the command passes instead, and what the coverage map says about files it does not list (ADR-152 §4's `listing`).

## Options
**Detection.** (a) Only `include` in any of the files: that is the key that conflicts with a source. (b) Any of `source`, `source_pkgs`, `include`: a project that names a source already says what to measure, and Cupel's `--cov=.` would override it (coverage.py takes the command line's source over the file's). (c) Any coverage section at all: a `[run] branch = True` would lose ADR-152's listing for nothing.

**The command.** (a) Bare `--cov`: pytest-cov starts coverage and the project's configuration chooses the files. (b) `--cov=<each include pattern's directory>`: back to a source, and the warning.

**The listing of such a map.** With bare `--cov`, ADR-152 reads the map as `Loaded`, and `tests.coverage` reports every function of a file the map does not list as `coverage.unloaded-file`. Under the project's configuration a file can be missing for two reasons: no test loaded it (a module under `include` nothing imports), or the project chose not to measure it (a script outside `include` or `source`). (a) `Loaded`: reports both, the second a false positive on every file the project deliberately leaves out. (b) `All`: reports neither; with `source` it is exactly right, since coverage.py lists every file under the source at 0 %; with `include` alone an unimported module under the patterns is unscored, as any unlisted file of a tool that lists every file is. (c) Read the patterns and decide per file: a second implementation of coverage.py's matching.

## Decision
1. **A Python root's `packages` is empty when the project's coverage configuration chooses its files**: `source`, `source_pkgs`, or `include` set in `.coveragerc`'s `[run]`, `setup.cfg`'s or `tox.ini`'s `[coverage:run]`, or `pyproject.toml`'s `[tool.coverage.run]` (`crates/cupel-lang/src/langs/python/project.rs`). Otherwise ADR-152 holds: `src` for a src layout, else `.`.
2. **An option given `<packages>` is bare when there are none**: the toolchain table's `--cov=<packages>` fills as `--cov` (`toolchain::fill`), so `init` writes `pytest --cov --cov-report=lcov:{out}` for such a root. The joint already passes the bare `--cov` when the configured command names no source and the root has no packages.
3. **A coverage map of a root without packages lists every file** (`Listing::All`) for `tool.pytest-cov`, `tool.coverage-py`, and the joint, whatever the command: the project's configuration chose the files, and a file it leaves out is not one the suite failed to load. `CoverageSpec.listing` takes the root's packages with the command.
4. **The per-test producer is unchanged**: `tool.pytest-cov-per-test` runs with its own `--cov-config` and `--cov=.`, so the project's run settings do not apply to it and nothing conflicts.
5. **Fixture**: `crates/cupel-fixtures/repos/coverage-config/python/` (`include` and `filterwarnings = ["error"]`, an included module with a tested and an untested function, a script outside `include`), with `crates/cupel-fixtures/tests/coverage_config.rs`.

## Consequences
- httpx-shaped projects get a coverage command that runs: `init` writes the bare `--cov`, and `tests.coverage` and the joint run under the project's own configuration.
- A `cupel.toml` written before this decision keeps its `--cov=.`; the configured command is the user's, and the joint keeps a configured source. Running `init` again, or deleting `=.`, applies the decision.
- With `include` alone, a module under the patterns that no test imports is not scored (the false negative of option (b)); a project that wants such modules reported names a `source`, which coverage.py walks.
- The Python `project-info` fact of such a root shows `packages: []`; nothing else in the fact changes.
