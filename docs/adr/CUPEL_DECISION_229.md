# CUPEL_DECISION_229: Cupel's pytest runs pass the arguments `[lang.python] test` gives pytest

**Date:** 2026-11-14 · **Status:** accepted · **Phase:** v0.3.0, item 7 (Q-py-2) · Details [084](CUPEL_DECISION_084.md) (the joint run), [149](CUPEL_DECISION_149.md) (its incremental reruns)

## Context
Validating v0.2.9 on encode/httpx, three tests marked `@pytest.mark.network` (`tests/test_proxies.py:104,114`) fail without network, and httpx documents the marker for network-disabled builds. The joint suite run (`tool.pytest-cov-contexts`, ADR-084) builds its own `pytest …` argument list and ignores `[lang.python] test`, so a person cannot deselect tests from `cupel.toml`; only `PYTEST_ADDOPTS='-m "not network"'` or pytest's own configuration reached the run (NOTES Q-py-2: "recommend the joint run appends the configured test command's extra pytest arguments"). The pytest runner (`tool.pytest`) and the per-test producer (`tool.pytest-cov-per-test`) build their argument lists the same way and ignore it too.

The owner's ruling: "append [lang.python].test arguments to the joint run."

Open: which words are the arguments, which runs get them, and what happens to a run Cupel narrows to some tests.

## Options
1. **The arguments.** (a) The command split on whitespace: `-m "not network"` would reach pytest as `-m`, `"not`, `network"`. (b) The words after `pytest` (the program, or `-m pytest`'s module) as `sh` splits them (single and double quotes, backslashes), up to the first control operator (`&&`, `||`, `;`, `|`, `&`).
2. **Which runs.** (a) The joint alone, as the ruling's words say. (b) Every Cupel pytest run: the joint, the runner (the baseline without the joint, `tests.flaky`'s reruns, the mutation engine's runs and its listing), and the per-test producer. The joint stands in for exactly the runner and per-test producer the store would choose (ADR-084 §1) and must make the facts they make; with (a), a test the joint leaves out would run in every mutant's run and every rerun, and a network test would fail each.
3. **A narrowed run** (node IDs or files Cupel names: an incremental joint run, a mutant's covering tests, a rerun). pytest runs the union of every path it is given, so a configured `tests/` beside node IDs would run the whole directory. (a) Pass every argument. (b) Pass the options and leave out the positional words: a word that does not start with `-` and is not the value of the option before it (an option without `=`, such as `-m` or `--deselect`, is taken to take the next word).

## Decision
1. **(b).** The arguments are the words after `pytest` in `[lang.python] test`, split as `sh` splits them, up to the first control operator. `python -m pytest`, `uv run python -m pytest`, and a bare `pytest` (or a path ending in `/pytest`) all name it; a command that does not run pytest gives none.
2. **(b).** Every pytest run Cupel makes appends them after its own options and before the tests it names: the joint's run and its listing, the runner's runs and its listing, and the per-test producer's runs and its listing.
3. **(b).** A narrowed run passes the options with their values and leaves out the positional words; a whole-suite run and a listing pass every argument. A word after an option without `=` is kept as that option's value even when the option takes none (`--lf tests/`): the narrowed run then also runs `tests/`, which costs time but drops nothing.

## Consequences
- `test = "python -m pytest -m 'not network'"` keeps httpx's network tests out of the baseline, the joint's coverage and per-test map, every rerun, and every mutant's run.
- The `test` command `init` writes (`<m> pytest`) has no arguments, so a run of it is unchanged.
- An option that conflicts with one Cupel passes (`--junitxml`, `-p cacheprovider`, `--cov…`) is passed as written, after Cupel's, and pytest decides what the pair means.
- Other languages' `test` commands are still not read by their runners; this decision is about pytest's.
