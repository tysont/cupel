# CUPEL_DECISION_025: Structural identity: normalization rules, `NormTree` serialization, and 64-bit symbol and site hashes

**Date:** 2026-10-10 · **Status:** accepted; the `erase` rule amended on 2026-10-26 (P9.9 group 3, [ADR-138](CUPEL_DECISION_138.md) §10): nothing below a kind of `[normalize] keep` is erased (TypeScript type declarations) · **Phase:** P1.4

## Context

Design §8.4.1 defines `Symbol.hash = blake3(lang ∥ serialize(NormTree))` with a language-independent serialization and dryer's normalization rules (§3: drop locals, fields, literals, keywords; keep call names, operators, tree shape). P0.2 left the digest length open (`SymbolHash` accepted `sh_` plus any amount of hex) and gave `SiteHash` no length either. P1.4 computes the first hashes, so the length, the byte encoding, and what "keep call names" means on a syntax tree must be fixed now: every finding ID and baseline is derived from them.

## Options

**Length.** 32 bits (8 hex) is short in output but collides with probability ~1% at 10⁴ symbols. 64 bits (16 hex) makes accidental collision negligible (~3·10⁻⁸ at 10⁶ symbols) and still fits a terminal line. 128 or 256 bits buy nothing a repository can use and make every envelope and `--symbols` argument longer.

**Serialization.** A hand-rolled S-expression is compact but needs an escaping rule; compact JSON of `NormNode` (`{"label":…,"children":[…]}`, fields in declaration order) is unambiguous, language-independent, and already the fact's wire form.

**Call names.** Keeping the whole callee text keeps receivers (`self.items.push`) and makes `a.push` and `b.push` differ, which contradicts "drop locals". Keeping only identifier leaves in the callee's name position, while normalizing receiver subtrees as usual, keeps `push` and drops `self.items`.

## Decision

- `SymbolHash` is `sh_` plus exactly 16 lowercase hex digits: the first 8 bytes of `blake3(lang ∥ 0x00 ∥ json(root))`, where `lang` is the language ID and `json` is the compact serde JSON of the root `NormNode`. `SiteHash` is exactly 16 lowercase hex digits (first 8 bytes of its BLAKE3 digest; its inputs are fixed by P1.5). Both types reject any other length.
- `TableNormalize` builds a symbol's `NormNode` from its definition node, driven by `[normalize]` in `tables.toml`: the symbol's own name node is dropped; comments (tree-sitter extras) and node kinds in `erase` are dropped with their subtrees; kinds in `flatten` are replaced by their children; named kinds in `identifiers` become `$id` leaves and kinds in `literals` become `$lit` leaves; other named nodes keep their kind as label; anonymous tokens are kept, as leaves labeled with their text, only when they are operators: made only of `+ - * / % = ! < > & | ^ ~ ?`, or listed in `operators` (word operators such as Python's `and`). Inside the callee field of a node kind listed in `calls`, identifier leaves keep their text, except within the callee's `receivers` fields, which normalize as usual.
- A language has a `NormTree` producer when it has `symbols.scm` and a `[normalize]` table.

## Consequences

- Hashes are invariant under renaming the symbol, its parameters, locals, and fields; under reformatting and comments; under literal changes; and under moving the file. They change with operators, control structure, and call targets. Tests assert each property.
- Two structurally identical symbols (two trivial getters of different fields) share a hash, and therefore finding IDs for the same check and site. This is dryer's notion of sameness; a check that must tell such symbols apart has the `SymbolKey` within one run, and a later ADR may add a disambiguator if findings collide in practice.
- The known-answer vectors in `cupel-core/tests/ids.rs` use 16-digit hashes, recomputed with the Python `blake3` package.
- Because `lang` is hashed, a function moved from `.js` to `.ts` changes identity even when its `NormTree` is unchanged; family-level comparison (`static.clones`) compares trees, not hashes.
