# CUPEL_DECISION_002: Git access shells out to the `git` on `PATH`, from one module in `cupel-core`

**Date:** 2026-10-13 · **Status:** accepted · **Phase:** P2.1

## Context

P2.1 resolves `--changed` (the merge base with the default branch, plus the working tree) and `--base/--head`, and fills `run.git` (`sha`, `dirty`, `base`, `branch`) in every `check` envelope. P0.8 added a temporary shell-out in `cupel-cli/src/git.rs` for `init`'s `default_branch` (ADR-021 §7) and left the real choice to this ADR. The plan (§2.5) asks to measure before choosing between shelling out and a library (`gix` or `git2`).

What Cupel asks of git is small and read-only: the default branch, `HEAD`'s SHA and branch name, whether the tree is dirty, a merge base, and the names of files changed between two revisions or between a revision and the working tree (with untracked, non-ignored files). Every question is answered once per run.

## Options

**Shell out to `git`.** Each question is one `git` process in the repository root. Measured on the dev machine (Apple M2, git 2.50.1, the 457-file Cupel repository, warm caches): `git rev-parse HEAD` 16 ms, `git merge-base HEAD main` 18 ms, `git diff --name-only --relative HEAD` 17–20 ms, `git ls-files --others --exclude-standard` 29 ms, `git status --porcelain` 31 ms. Almost all of it is process start-up. A `--changed` run asks seven or eight questions, about 150 ms; a whole-tree run asks three (`run.git`), about 60 ms. Against the design's targets (`check --depth quick` 2–5 s cold on 500k LOC) this is noise, though it is three times the 50 ms "CLI start-up to first byte" budget, which `describe` and `--describe` never pay. Shelling out adds no dependency, no compile time, and no binary size, and it uses exactly the user's git: their config, `safe.directory`, worktrees, sparse checkouts, `core.fsmonitor`, credential-free local operation, and any newer repository format. A missing `git` is an environment problem `doctor` already reports (P0.8).

**`git2` (libgit2 bindings).** In-process calls take microseconds, but libgit2 is a C library built by `cc` (or linked from the system), which complicates the cross-platform release builds of P8.1, and libgit2 trails git in features (worktree and sparse-checkout edge cases, newer index and object formats, `fsmonitor`). It adds roughly 1 MB to the binary and a C toolchain to every build.

**`gix` (gitoxide).** Pure Rust and fast, but a large dependency tree (dozens of crates, a noticeable share of a clean build's compile time) for five read-only questions, and its status/diff-with-worktree APIs are still evolving. Neither library is in the local cargo cache, so its cost could not be measured end to end here; the compile-time and size costs above are the published figures' order of magnitude, not a measurement.

**Where the code lives.** Scope resolution is in `cupel-core` (design §8.2 lists "scope" among core's responsibilities), and the scope-resolution unit tests need a real repository. Keeping the git module in `cupel-cli` would push those tests into the binary crate and keep two git layers. `cupel-core` already performs I/O (the `Writer`), and the crate-graph rule (plan §2.1) is about crate dependencies, not about `std::process`; the `disallowed_methods` restriction applies to `cupel-checks` only.

## Decision

- Cupel shells out to the `git` found on `PATH`. Every call runs in the repository root (`--cwd`), with stdin closed and stderr captured, and is read-only.
- All git access lives in `cupel_core::git` (`Git::open(root)`, `default_branch`, `head`, `merge_base`, `changed_since`, `changed_between`). `cupel-cli/src/git.rs` is removed; `init` and `doctor` call the core module.
- Paths come back relative to the repository root (`git diff --relative`, `git ls-files` from the root), `/`-separated, NUL-delimited on the wire so no file name needs unquoting.
- Outside a git repository, or without `git`, `Git::open` returns `None`. `--changed` and `--base` then fail with a usage error that says so (exit 2); `run.git` is `null`.
- The default branch is `default_branch` from `cupel.toml` when set, else the branch `origin/HEAD` points at, else a local `main` or `master`, else the branch `HEAD` names (the P0.8 rule).

## Consequences

- No new dependency; every git feature the user's git supports works.
- A `--changed` run spends about 150 ms in git on the dev machine. If the daemon (P4.2) needs warm git answers, it can cache them per index mtime or replace the module's internals with `gix` without changing its API; that would be a superseding ADR with measurements.
- `doctor` already reports a missing `git` (not required); `--changed` without git is a usage error naming the fix.
