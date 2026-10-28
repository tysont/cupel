# CUPEL_DECISION_153: The two network-dependent tests: one hermetic, one behind `make test-network`

**Date:** 2026-10-28 · **Status:** accepted · **Phase:** P10.4

## Context

Two tests of `make test` reach the network, and a tree dogfood (whose baseline is Cupel's own suite under coverage) fails when the machine is offline or on battery, as it did during the 2026-10-25 DNS outage and again on 2026-10-27 (P9 Notes "P9 — v0.2.0"):

- `fix_with_install_installs_the_runner_into_a_scratch_venv_and_passes` (`crates/cupel-cli/tests/install.rs`, ADR-081) runs `doctor --fix --install` on a repository whose runner is missing, so Cupel runs the toolchain table's install command, `python -m pip install pytest`, into a scratch virtual environment, and the test checks that `doctor` then finds pytest and reports its version. The network access is the test's subject: the real install of the real tool.
- `each_wrapper_refuses_a_bad_checksum_downloads_once_and_forwards_the_exit_code` (`crates/cupel-cli/tests/installers.rs`, ADR-125 §6) installs the npm and pipx wrappers as their package managers do and runs them against a fake release on a local HTTP server. The npm half is offline (`npm pack`, then `npm install --offline`). The pipx half runs `pip install --target <dir> dist/pipx`, and pip builds the package in an isolated environment into which it installs the build backend, `flit_core`, from PyPI. The network access is incidental: the test's subject is the wrapper's logic, and every byte the wrapper itself downloads comes from the local server.

Plan P10.4 asks, per test, for a hermetic test where the subject is logic and a network-gated one where the subject is the download, with both still run in CI on every push.

## Options

**The install test.** (a) Hermetic through a local index: a `pytest` wheel served from the test's temporary directory with `PIP_NO_INDEX`/`PIP_FIND_LINKS`. A real pytest wheel vendored in the repository is a binary of several hundred kilobytes; a hand-made wheel is a fake tool, and the test would then check that `doctor` reads the fake's `--version`, proving nothing about the install command the table gives. (b) Gated: the test runs only when an environment variable says the network is available, reports itself skipped otherwise, and a `make` target sets the variable; CI's `installers` job, which already runs on a clean image with Python, runs the target. The real install stays tested on every push, and `make test` and the dogfood baseline never reach PyPI.

**The wrapper test.** (a) Gate it as well: the wrappers' offline behavior (the checksum refusal, the single download, the forwarded exit code) would then leave the baseline too, for one build step that needs nothing from the network but a build backend. (b) Build the pipx package without pip's isolated environment (`--no-build-isolation`), with `flit_core` installed as one more pinned development tool (ADR-139): the real packaging path (`pyproject.toml`'s `[tool.flit.module]`, the `cupel` console script) runs offline, and `--no-index` makes any attempt at the network a failure rather than a silent dependency. Without `flit_core` the test reports `SKIPPED`, as it does without node or python3, and fails under `INSTALLER_TESTS_REQUIRE_TOOLS`, as CI's `installers` job sets it. (c) Write the wheel by hand from `pyproject.toml`'s metadata and install that: offline too, but flit's reading of the metadata is what the install should exercise.

## Decision

1. **`fix_with_install_installs_the_runner_into_a_scratch_venv_and_passes` is network-gated.** It runs only when `CUPEL_NETWORK_TESTS` is set; otherwise it prints `SKIPPED: the network is not available to tests; set CUPEL_NETWORK_TESTS=1 or run make test-network` and returns. `make test-network` runs `crates/cupel-cli/tests/install.rs` with the variable set. CI's `installers` job runs `make test-network` after `make installers` on both of its images, so the real install runs on every push; `make test`, `make check`, and the dogfood baseline never set the variable.
2. **`each_wrapper_refuses_a_bad_checksum_downloads_once_and_forwards_the_exit_code` is hermetic.** The pipx wrapper is installed with `python3 -m pip install --no-index --no-build-isolation --target <dir> dist/pipx`, so pip builds it with the `flit_core` already installed and touches no index. `flit_core==3.12.0` (within `pyproject.toml`'s `flit_core>=3.11,<4`) joins the pinned Python packages of `scripts/dev-setup.sh`, the CI tools action, and the `installers` job. A `python3` that cannot `import flit_core` skips the pipx wrapper with `SKIPPED`, or fails under `INSTALLER_TESTS_REQUIRE_TOOLS`.

## Consequences

- `make test` and the dogfood baseline run without a network; an offline or battery-powered tree dogfood no longer fails on these two tests.
- The real `pip install pytest` is still exercised on every push, in the `installers` job, and locally by `make test-network`; a developer who has not run it has not run that test, which `make test`'s output says.
- One more pinned tool (`flit_core`) is installed by `make dev-setup`, the tools action, and the `installers` job; `crates/cupel-cli/tests/dev_setup.rs` holds the three to the same version.
- `CUPEL_NETWORK_TESTS` is the one switch for tests that need the network; a future test that does uses the same variable and the same skip line, and `make test-network` grows to run its file.
