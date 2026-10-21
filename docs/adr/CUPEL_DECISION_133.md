# CUPEL_DECISION_133: Remediation hints for `clone.cluster`, `complexity.high`, and `dep.cycle`

**Date:** 2026-10-20 · **Status:** accepted · **Phase:** P9.4

## Context

`explain <finding-id>` returns a stored finding with its source excerpt, covering tests, nearest test file, and test skeleton (design §6.5); a finding's `action.hints` is an open map whose only keys so far are `nearest_test_file` and `covering_tests`. The owner's P9 follow-up item 4 asks three finding kinds for a remediation hint computed from the facts the finding was built on:

- `clone.cluster`: the largest normalized subtree every member shares, as an extraction candidate: its span in one member, the matching spans in the others, and the parameters that differ between members.
- `complexity.high`: the deepest nested block of the function: its span, its nesting depth, and the construct kinds on the path to it.
- `dep.cycle`: the cheapest edge to cut: the edge of the cycle whose removal breaks it and that carries the fewest imported symbols, with those symbols listed.

`explain` reads the fact store and never re-runs, so a hint must be computed by the check when it reports the finding and stored with it. The facts the checks read do not carry what the hints need:

1. `NormNode` has a label and children only. No node knows where it is in the file, and an identifier or literal is the marker `$id` or `$lit` with its text gone (ADR-025, ADR-050), so neither a subtree's span nor the names that differ between clones can be read.
2. `Decision` has a kind, a nesting, and weights, but no span, so the deepest block cannot be located.
3. `ImportEdge` carries the specifier and target but not the names the statement imports; ADR-058 records that `RawImport.names` does not survive into the fact.

Also open: where a hint's shape is declared (the run schema, `describe`), how text renders it, and what a hint says when the facts cannot support it.

## Options

**Where hints are computed.** In the checks, from the facts they already read, stored on the finding: `explain` stays a reader, and `check --format json` carries the same hint an agent would get from `explain`. Or in `explain`, re-reading facts or source: it would need the facts of a past run (the store keeps findings, not facts) or a re-parse, which design §6.5 forbids.

**Extending the facts.** (a) New fact kinds for spans and imported names: plan §2.5 requires an ADR and a producer for every language per kind, for data that belongs to existing facts. (b) Optional fields on the existing payloads: `NormNode.span` and `.text`, `Decision.span`, `ImportEdge.names`, absent from JSON when empty. Every language's producer is one of three normalizers, one decisions producer, and six resolvers, all in `cupel-lang`, so (b) touches each producer once and no check other than the three. A payload without the fields (a plugin producer, a hand-written test fact) still deserializes; its hints are absent. The fact versions stay at 1: the fields are additive, nothing caches facts across binaries (`CacheKey` is not used outside the store's tests), and a plugin that provides version 1 stays valid.

**Keeping the structural hash.** `structural_hash` hashed `json(root)`. Spans and texts in that JSON would change every symbol hash, so every finding ID, baseline, and suppression. The hash is instead taken over the shape alone, serialized exactly as before (`{"children":[…],"label":…}`), so every hash is unchanged.

**Typing hints.** (a) Turn `Action.hints` into a struct: dozens of construction sites and the open map that plugins and the two test hints use would change. (b) Keep the map; the three hints are core types (`cupel_core::hint`) serialized under fixed keys, the run schema's `hints` names every known key with its schema (each optional, the map still open), and each finding kind's manifest declares the hints it carries (`FindingKindSpec.hints`, absent when empty), so `describe finding <kind>` documents them.

**Common subtree identity.** ADR-050's depth-2 fingerprints measure similarity but cannot identify a whole shared subtree. A complete-subtree shape hash (label and children, Merkle-style, confirmed by comparing shapes) can.

## Decision

1. **Facts** (version unchanged, fields optional and omitted when empty):
   - `NormNode.span`: the span of the syntax node a normalized node came from; every node a normalizer builds from a syntax node has one. `NormNode.text`: the source text of an identifier or literal leaf (`$id`, `$lit`), the one place a name survives normalization.
   - `Decision.span`: the span of the decision's construct node.
   - `ImportEdge.names`: the names the statement imports from its target as written (the imported name, not a local alias), empty when it imports the module whole or the language's import names nothing. Rust: the path segments after the module the path resolves to (`super::beta::BETA` → `BETA`; `*` and `self` excluded). Python: the `from … import` name (`*` excluded). Java: the type or static member a single import names (the specifier's last segment; on-demand imports none). JavaScript and TypeScript: the names of `import { a, b as c }`, `export { a } from`, and `default` for a default import; a namespace import, `require`, or dynamic `import()` names none. Go imports packages and Clojure libspecs are read without `:refer`, so both carry none (an open item for Clojure).
   - `structural_hash` hashes the shape only; `NormNode::same_shape` compares labels and children, and `static.clones` uses it for `clone_type`.
2. **Hint shapes** (`cupel_core::hint`, keys in `action.hints`):
   - `common_subtree` on `clone.cluster`: `nodes` (the subtree's normalized nodes), `share` (of the first member's nodes, three decimals), `location` (in the finding's member, the first), `others` (each other member's `symbol` and `location` of its matching subtree, in evidence order), and `parameters`: each identifier or literal position whose text differs between members, as `kind` (`identifier` | `literal`), `values` (one text per member, first member first), and `occurrences` (positions with the same values, counted once).
   - `deepest_block` on `complexity.high`: `location`, `nesting`, `path` (the construct kinds from the outermost enclosing block to the block itself), and `cyclomatic` and `cognitive`: what the decisions inside the block contribute to the function's complexity now, so what extracting it moves out.
   - `cheapest_cut` on `dep.cycle`: `from`, `to`, `line` (the first import statement), `specifiers` (as written), `names` (sorted, distinct), `unnamed` (statements that name nothing), and `breaks_component` (whether removing every import from `from` to `to` leaves the cycle's modules with no cycle at all).
3. **Choice rules.**
   - Common subtree: among the non-leaf nodes of the first member that have a span, the one with the most nodes whose shape occurs, with a span, in every other member; ties go to the first in pre-order; in each other member the first occurrence in pre-order matches. Parameters come from walking the matching subtrees in step, comparing the `text` of each leaf that has one.
   - Deepest block: among the function's structural decisions (branch, loop, case, catch, ternary: the kinds that nest) with a span, the greatest nesting; ties go to the most decisions inside its span, then the first in source order. The path holds, for each nesting level below it, the last structural decision at that level whose span contains it.
   - Cheapest cut: among the edges of the reported shortest cycle, those that break the whole component first, then the fewest unnamed statements, then the fewest names, then cycle order.
   - A hint whose facts are missing (no spans, no shared subtree, no decision with a span) is absent, never a placeholder.
4. **Rendering.** JSON carries the hints in `action.hints` wherever the finding appears (`check`, `survey`, `explain`). Text renders each typed hint on one line under its finding, and `explain` renders it in full: every location, every parameter with its values, every name.
5. **Schema and `describe`.** The run schema's `action.hints` lists `nearest_test_file`, `covering_tests`, and the three hints, each optional, with further keys allowed. `FindingKindSpec.hints` (`x-cupel-optional`) maps each hint a kind carries to its JSON Schema; `describe finding <kind>` and `describe <check-id>` show it.

## Consequences

- Each `clone.cluster` says what to extract and with which parameters, each `complexity.high` which block to extract first, and each `dep.cycle` which import to remove and what moves with it, without re-running anything.
- The NormTree, Decisions, and ModuleGraph facts grow (a span per normalized node is most of it); fact goldens show the new fields for every language. Symbol hashes, finding IDs, and every existing evidence value are unchanged.
- A cycle edge in Go or Clojure has only unnamed statements, so the cut there is chosen by component breaking and cycle order alone. Clojure `:refer` names are a follow-up.
- P9.7 (a raw-token digest on `NormTree`) can derive the digest from the leaf texts this ADR adds rather than adding another field.
- `clone.pair` could carry the same `common_subtree`; it is left out because the follow-up names clusters, and adding it is one line in the check.
