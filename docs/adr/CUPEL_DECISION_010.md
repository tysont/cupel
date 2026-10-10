# CUPEL_DECISION_010: `make clean-check` compares the tree before and after the build

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P0.1

## Context

Plan §2.3 defines `make clean-check` as "fails if `git status --porcelain` is non-empty after a build" and makes it part of `make check`, which §1.2 and §2.6 make the pre-commit bar. Taken literally the two rules contradict each other: before a commit the tree always contains the uncommitted work being checked, so `git status --porcelain` is never empty and `make check` could never pass at the moment it is required to. The purpose stated in §2.3 is narrower: catch generated files that escaped `.gitignore`.

## Options

**Literal rule.** `clean-check` requires an empty porcelain status. Correct in CI on a fresh checkout, impossible locally before a commit, so the pre-commit bar would be ignored in practice.

**Run `clean-check` only in CI.** Keeps the literal rule but drops it from the local `make check`, so a build that dirties the tree is only discovered after pushing.

**Before/after fingerprint.** `make check` fingerprints the working tree (porcelain status including untracked files, plus the content diff) before linting, testing, and documenting, and `clean-check` fails if the fingerprint changed. Run on its own with no recorded fingerprint, `clean-check` requires a completely clean tree, which is the literal rule.

## Decision

Before/after fingerprint. The fingerprint is `git hash-object` over `git status --porcelain --untracked-files=all` followed by `git diff`, stored under `target/` (gitignored) for the duration of `make check`.

## Consequences

- `make check` is runnable before every commit and still fails when a build writes a file `.gitignore` does not cover (for example a stray `.snap.new`).
- In CI the starting tree is clean, so the check is identical to the literal rule.
- A stray file created by hand before `make check` is not flagged locally; it is caught by CI's clean checkout and by reviewing `git status` before committing, which plan §2.6 already requires.
