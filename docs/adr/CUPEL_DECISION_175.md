# CUPEL_DECISION_175: A range's recomputed base scores the symbols the range touched at the base

**Date:** 2026-11-10 · **Status:** accepted · **Phase:** v0.2.6, Molten field trial Area 8 · Amends [130](CUPEL_DECISION_130.md)

## Context

ADR-130 recomputes a range's `base` by running the rule's checks over the base versions of the changed files. Its consequences record the asymmetry this leaves: the current side scores what the range touched (ADR-142's symbols under the changed lines), while the base side scores the base files whole. "Comparing changed lines with their base counterparts needs the base side of each hunk; it is not done."

Area 8 replayed ten Molten PRs with a candidate `[gate.changed]` whose ratchets were `static.complexity.cyclomatic_max`, `static.deps.cycles`, and `static.clones.pairs` `<= base`. Each bound was the worst value in any touched file, not in the touched code:

- #81a72912 changed 15 symbols, whose worst cyclomatic complexity was 3. The bound was 447: `agent-do.ts`'s `runAgentLoop`, which the PR did not touch, but whose file it did.
- Its clone pairs were 1 against a bound of 126, which counted every pair in the touched files.
- Across the ten PRs, the complexity bound was 296, 445, or 447 in eight. A PR could raise a touched function's complexity from 5 to 200 and pass.

For a max or count metric the ratchet admitted almost anything. For a ratio it went the other way: ADR-130's own test failed an untested change to an untested function because its file's other functions were tested (0.0 against 0.6).

## Options

**Leave it, and recommend literal rules for ranges.** The documented limitation stays, and a `<= base` ratchet on a range means "no worse than the worst of the touched files". That is not what a ratchet reads as. Literals are what ADR-130 set out to replace.

**Score the base with the old side of the same diff.** `git diff -U0 base head` already gives each hunk's old range (`-a,b`). Giving those ranges to the base run as its changed lines narrows the base symbols exactly the way ADR-142 narrows the head symbols: a base symbol is in scope when an old-side range falls within its span. A pure insertion (`b` is 0) is the lines around where it went, as a pure deletion is on the new side. A file the range deleted has its whole old content as one range. A file the range added has no base side, as before.

**Match symbols by name across the sides.** This handles a renamed function differently from the diff. But the head side is line-based, so a name match would make the two sides disagree about what was touched.

## Decision

1. **The old side of a hunk.** `Git::base_lines(base, head)` reads the same `git diff -U0` as `Git::changed_lines`, and returns each file's old-side ranges. The hunk reader reads a file's `---`/`+++` names only in that file's header, before its first `@@`. A removed line beginning `-- ` or an added line beginning `++ ` is not taken for a file name.
2. **The base run's changed lines.** `reference::recompute` passes the range's base lines to the base run (`check::run_touching`). That run's `Job.changed` is those lines in place of its own scope's, which is `--paths` and has none. Every check that narrows by changed lines then narrows the base the way it narrows the head: the symbols in scope (ADR-142) and the mutants planned (ADR-141).
3. **Unchanged.** `--against` still names a stored run as it is. A range whose changed files are all new still has no reference. The base copy, `--only`, and `--jobs` are as ADR-130 §3 has them.

## Consequences

- A range's `<= base` ratchet compares the touched code with what it replaced. In the test, a function grown from complexity 1 to 2 beside an untouched one of 2 fails `cyclomatic_max <= base` with bound 1.
- `tests.mutation.score >= base - 0.02` on a range no longer fails an untested change to code that was untested before it. It fails a change that leaves the touched code less protected than it was. In the test, `discount` scores 0.75 at the base and 0.5 after the test of its discounted branch is deleted. To require tested changes regardless of the base, a literal rule (`>= 0.80`) remains the way.
- A count such as `static.clones.pairs` counts the pairs that involve a touched symbol on each side. A clone of untouched code elsewhere in the file no longer raises the bound.
- A symbol whose span moved without its lines changing is outside both sides, as it already was on the head side.
- ADR-130's consequence bullet on "the same files, not of the same lines" and the plan's P9.1 open item (2) are resolved by this decision.
