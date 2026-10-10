# CUPEL_DECISION_016: Findings without a symbol are identified by their subject

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P0.3

## Context

Design §7.2 derives a finding's ID from `blake3(check ∥ kind ∥ symbol.hash ∥ site_hash)`, and ADR-013 §8 fixed the byte encoding but left open how to identify a finding that is about no symbol at all. P0.3 emits the first such findings: usage errors, `cli.not-implemented`, and internal errors. `doctor` (P0.8) will follow with findings about missing tools and bad configuration. Every finding carries a required `id`, `check`, and `kind`, and the ID must stay stable for the same problem so baselines, suppressions, and agents can track it.

## Options

**Make `id` nullable for symbol-less findings.** Avoids inventing an identity, but every consumer (`explain`, `suppress`, `next_actions[].finding`, SARIF fingerprints) would need a second code path, and `doctor` findings genuinely benefit from being suppressible.

**Hash the message text.** Gives every finding an ID, but titles and details are prose that we will reword; the ID would change with wording, which is exactly what the structural ID exists to prevent.

**Hash a subject: the structural name of what the finding is about.** The subject is the thing the fix touches — a flag (`--format`), a verb (`check`), a describe target (`describe matrix`), a tool (`cargo-llvm-cov`), a configuration key (`required_version`). It is a name, never a supplied value, a message, a path outside the repository, or a line number. Encoded in the symbol-hash position as the empty string, with the subject in the site position, the input is `check NUL kind NUL "" NUL subject`. A symbol's hash always starts with `sh_`, so it is never empty, and the two input spaces cannot collide.

## Decision

Subject-based identity. `FindingId::for_subject(check, kind, subject)` hashes the UTF-8 strings `check`, `kind`, the empty string, and `subject`, NUL-separated, and keeps the first three bytes as hex after `fnd_`, exactly as `FindingId::compute` does. Subjects come from the command line and identifiers, neither of which can contain NUL.

Findings that no check produced name their emitting component in `check`: `cli` for the command line (kinds `cli.usage`, `cli.not-implemented`, `cli.internal`). `doctor` uses `doctor` when it lands in P0.8.

## Consequences

- Every finding has a stable, well-formed ID; no consumer special-cases symbol-less findings.
- Two usage errors on the same flag with different bad values share an ID. That is intended: they have the same fix.
- The existing known-answer vectors for `FindingId::compute` are unchanged; `for_subject` gets its own vector, computed independently with the Python `blake3` package.
- `doctor` (P0.8) chooses subjects by this rule: the tool ID or configuration key the finding asks the user to fix.
