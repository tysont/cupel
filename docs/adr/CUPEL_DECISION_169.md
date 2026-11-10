# CUPEL_DECISION_169: Analyzers in member roots: ESLint's config may be above the root, and a failure in one root keeps the others' results

**Date:** 2026-11-10 · **Status:** accepted · **Phase:** v0.2.6, Molten field trial Area 5 · Amends [102](CUPEL_DECISION_102.md) §3–4 (and the ESLint reader)

## Context

Since ADR-167, a workspace that delegates its tests is analyzed one member root at a time. On the owner's Molten repository (eight roots), `static.analyze` passed with no findings while analyzing nothing:

- **ESLint was "unconfigured" in every root.** Its detection required a flat config (`eslint.config.js` and siblings) in the root itself. Molten keeps one `eslint.config.js` at the repository root, and ESLint finds it from any member: ESLint looks in the directory it runs in and then the directories above. Run by hand from the repository root, Molten's lint reports 424 problems.
- **tsc was "failed" with a detail that named no root.** `tools/discovery` is not a pnpm workspace member and its own dependencies were not installed, so `tsc -p .` stopped with TS2688 (no `@types/node`). The note read "tsc could not check the project" without saying which project. A failure in one root also replaced the analyzer's status for all of them, and the check reads results only from an analyzer whose status is `ran`. So a failure in one root would have hidden the other roots' results; on Molten those five roots happened to be clean.

## Decision

1. **A `file` detection rule can look above the root.** `{ file = "…", above = true }` holds when the file is in the root or any directory above it. ESLint's six flat-config rules use it, matching how ESLint finds its config. Without `above`, the rule is unchanged.
2. **An analyzer's failure names its root and stops only that root.** Each root where an analyzer fails adds `<root>: <problem>` to its detail, one line per root. The analyzer's status is `failed` when any root failed. Results from the roots where it ran are kept.
3. **`static.analyze` reports what ran.** Every result in the fact is a finding, whatever its analyzer's status. An analyzer counts toward the score's `n` when it ran in every root or returned results. Any status other than `ran` is still a note (`<tool> <status>: <detail>`), as ADR-102 says.

4. **ESLint skips what its config ignores, and names rule-less messages.** Cupel names the files in scope, so ESLint warned "File ignored because of a matching ignore pattern" for each one the project's config ignores. Cupel reported these as `parse-error`: 23 on Molten. ESLint runs with `--no-warn-ignored`, so those files are skipped silently, as in the project's own `eslint .`.

   A message without a rule is `parse-error` only when ESLint marks it `fatal`. Otherwise it reports an unused `eslint-disable` comment and is `unused-disable-directive`.

## Consequences

- On Molten, ESLint now runs in each member root with the repository's config, and the tsc note names `tools/discovery`. Cupel's results match what the repository's own `eslint .` reports.
- A config above the repository root also counts, as it does for ESLint itself. A stray `eslint.config.js` in a home directory would make ESLint run where it previously did not.
