# CUPEL_DECISION_022: What `doctor` checks, how it reports, and when it exits 3

**Date:** 2026-10-10 · **Status:** accepted; §6 (`--fix`) superseded by [081](CUPEL_DECISION_081.md) · **Phase:** P0.8

## Context

Design §6.5 says `doctor` "checks binary version vs `required_version`, every selected tool producer's presence and version, oracle reachability, config validity, write access to `.cupel/`. Each problem is a finding with `action.kind: install_tool | configure`. Exit 3 if anything required for the configured checks is missing; 0 otherwise. `--fix --install` attempts installs." In P0 there are no tool producers, oracles, or checks, and installing arrives with P3.1. Building `doctor` now requires answers to:

1. How `required_version` is parsed and compared.
2. How `doctor` reports configuration that does not load, given every other verb exits 2 on it.
3. How write access to `.cupel/` is tested.
4. Where tool checks plug in before the toolchain table exists, and whether `git` (needed by `--changed`) is one.
5. Which findings make the run exit 3, and the envelope's `status` then.
6. What `--fix` does before anything can be installed, and what `doctor --dry-run` plans.

## Options

For (1): the `semver` crate implements Cargo's requirement syntax (`>=0.4`, `^0.4`, `~0.4.1`, `0.4`, `*`, comma-joined ranges, pre-release rules) with no dependencies of its own; a hand-written `>=`/`<`/`^` comparator is a few dozen lines but silently disagrees with Cargo on bare versions, `~`, comma ranges, and pre-releases. The users who write `required_version` already know Cargo's syntax.

For (3): read permission bits (cheap, but wrong under ACLs, read-only mounts, and for root), or create and remove a file in `.cupel/` — the only test the operating system cannot answer wrongly.

For (4): leave the seam unused until P3 (untested code path), or route tool checks through one list of requirements now, with `git` as its first entry.

For (5): exit 3 only for missing tools (design wording), or for every `fail` finding. A configuration that does not load, a binary outside `required_version`, and an unwritable `.cupel/` each stop every configured check as surely as a missing tool.

## Decision

1. **Version.** `required_version` is a Cargo version requirement, parsed and matched with the `semver` crate against the binary's own version. Unsatisfied: `doctor.version-unsatisfied`, `fail`, `install_tool`, summary naming the requirement. Malformed: `doctor.version-invalid`, `fail`, `configure`. Both have subject `required_version` and evidence `{required}`; the binary's version is `cupel.version` in the same envelope, so findings never repeat it and goldens survive releases. Only `doctor` checks `required_version` today.
2. **Configuration.** `doctor` runs even when configuration does not load. The error becomes one `doctor.config-invalid` finding (`fail`, `configure`) whose subject, title, fix, and evidence are those of the `cli.usage` finding any other verb would have exited 2 with (ADR-016 subjects: `cupel.toml`, `--config`, `--profile`, `CUPEL_DEPTH`, …). The other checks then run against defaults; `required_version` is skipped because no file was read.
3. **`.cupel/`.** `Writer::probe()` creates `.cupel/` if needed, then creates and removes a uniquely named file in it; a recording writer only checks the path. A failure is `doctor.cupel-unwritable` (`fail`, `configure`, subject `.cupel`), whose evidence carries the operating system's message.
4. **Tools.** `doctor` checks a list of `Requirement { tool, needed_for, required, probe }`, where `probe` is a command that exits 0 when the tool is present. The list is `git` (`git --version`, needed for `--changed`, not required) followed by the requirements the toolchain table derives from the configuration, which is empty until the table exists (P1.9) and its producers land (P3). A missing tool is `doctor.tool-missing` with subject the tool ID, `install_tool`, and severity `fail` when required, `warn` otherwise.
5. **Exit and status.** Any `fail` finding makes the envelope `status: error`, exit 3 (`environment`). Otherwise `status: pass`, exit 0, with any `warn` and `info` findings attached. Findings use `check: "doctor"`, `FindingId::for_subject`, tier 0, confidence 1, and the run's repro.
6. **`--fix`.** It parses and is described with `pending_step: P3.1`. Given, it adds one `doctor.fix-pending` finding (`info`, `review`, evidence `{step: "P3.1"}`) saying nothing is installed yet; `--install` changes nothing further. `doctor --dry-run` checks nothing and plans nothing: an empty plan, since `doctor` writes only its probe and run record.

## Consequences

- One dependency, `semver`, joins `cupel-cli`.
- Agents get the configuration error and every other problem from one `doctor` run instead of fixing them one exit-2 at a time.
- P3 adds tool requirements by filling the toolchain-derived list; the finding shape, severity rule, and exit rule are already tested with a fake requirement.
- When `git` is missing `doctor` warns and still exits 0; `--changed` itself will exit 3 when P2.1 needs git.
