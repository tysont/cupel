# CUPEL_DECISION_136: Clone types 1, 2, and 3 from a token digest, with what differs in evidence

**Date:** 2026-10-20 · **Status:** accepted; amends the Type bullet of [050](CUPEL_DECISION_050.md) · **Phase:** P9.7

## Context

ADR-050 reports `clone_type` 2 when two normalized trees have the same shape and 3 otherwise, because `NormTree` kept no identifier or literal text, so a type-1 clone (only layout and comments differ) read as type 2. ADR-133 gave every `$id` and `$lit` leaf its source text (`NormNode.text`) and kept the structural hash over the shape alone. P9.7 (track (a) item 7) asks for a raw-token digest beside the fingerprints so a pair or cluster says 1, 2, or 3, and for the evidence to say what differs: the identifiers and literals for type 2, the inserted or removed statements for type 3. Left open: where the digest lives, what it covers, how a cluster is typed, and how "what differs" is computed and shaped.

## Options

**Where the digest lives.** (a) A new `NormTree` field computed by every producer: a fact change in three normalizers and every fact golden, for a value the check can compute from what the fact already carries. (b) A function in `cupel_core::similarity`, beside `Fingerprints`, folding the tree's labels and marker texts: no fact change, and a plugin producer's tree gets the same digest from the same fields. ADR-133 foresaw (b).

**What the digest covers.** The tree's labels (node kinds, operators, called names) in pre-order with each node's child count, and the text of every node that has one. Whatever normalization erased (layout, comments, the symbol's own name, and what a language's normalize table erases, such as Python annotations and decorators, Rust attributes and visibility, Java modifiers) is outside it, so two functions that differ only there are type 1. Reading the raw source instead would need the file text, which no fact a check may read carries.

**Missing texts.** A marker without text (a producer that does not record it) could hash as equal to any other marker, calling a renamed copy type 1. The digest is instead absent when any `$id` or `$lit` leaf has no text, and an absent digest never makes type 1.

**Typing a cluster.** By its pairs (the greatest type among the similar pairs) or by its members against the first member. A cluster can chain members that are not pairwise similar, so pair types can miss a member that differs from the others; comparing every member with the first is defined for every member and is what the evidence's differences are relative to anyway.

**What differs, type 2.** Equal shapes align node for node, so walking them in step and collecting each marker position whose texts differ gives exactly the renamed identifiers and changed literals. That is the `common_subtree` hint's `parameters` walk (ADR-133) applied to the whole tree, and its shape (`kind`, `values` one per member, `occurrences`) serves both a pair (`values` is the left and right text) and a cluster.

**What differs, type 3.** (a) A full tree edit distance: cubic, and its edit scripts (relabel, move) are more than a reader needs. (b) A top-down alignment: two nodes with different labels are a removal and an insertion; under two nodes with the same label, the children are aligned by a longest common subsequence of equal shapes, and between two aligned children an equal-length run of unmatched children is paired in order and compared again, while any other run is removed on one side and inserted on the other. A statement added to a block is one insertion; a changed statement narrows to the changed expression. (b) costs the product of the child counts at each level that differs, which is small for trees already at least `threshold` similar.

## Decision

1. **Digest.** `cupel_core::similarity::token_digest(root) -> Option<u64>`: the pre-order fold (`combine`, `mix`) of each node's label hash (FNV-1a as in the fingerprints), its child count, and its text's hash when it has one; `None` when a `$id` or `$lit` leaf has no text. It is computed by `static.clones` when it reads the tree; the `NormTree` fact and its version are unchanged.
2. **Type.** A pair is type 1 when both digests are present and equal and the shapes are equal, type 2 when the shapes are equal, and type 3 otherwise. A cluster is type 1 when every member's digest is present and equal to the first member's and every shape equals the first's, type 2 when every shape equals the first's, and type 3 otherwise. This replaces ADR-050's Type bullet; its other decisions stand.
3. **Evidence** (`clone.pair` and `clone.cluster`, both always present, empty when they do not apply):
   - `clone_type` on both kinds (the cluster gains it).
   - `renamed` (type 2): each identifier or literal position whose text differs between the members, as `kind` (`identifier` | `literal`), `values` (one text per member: left then right for a pair, evidence order for a cluster), and `occurrences` (positions with the same values, counted once), in order of first occurrence.
   - `edits` (type 3): for each member whose tree differs from the first member's (the right side for a pair), the subtrees the alignment above leaves unmatched, as `change` (`removed`: in the first member and not in `symbol`; `inserted`: in `symbol` and not in the first member), `symbol` (the member compared with the first), `nodes` (the subtree's normalized nodes), and `location` (the subtree's span in the member that holds it; absent when the producer gave the node no span). Edits are in pre-order of the alignment, removals before insertions within a run.
4. **Manifest and README.** The summary names the three types and what each means; the stated limitation becomes the one that remains: normalization's erasures count as layout, and two functions that differ only in literals are still clones (type 2, with the literals in `renamed`).

## Consequences

- A copy that differs only in layout, comments, or its own name now reads type 1, and a reviewer can tell an exact copy (extract and call) from a renamed one (extract with parameters, listed in `renamed`) and a modified one (`edits` say where).
- Every existing pair's `clone_type` is unchanged except where both trees carry equal texts: those read 1 where they read 2. Clusters gain `clone_type`, and both kinds gain `renamed` and `edits`; finding IDs, symbol hashes, and every other evidence value are unchanged.
- A type-1 pair can differ in what normalization erases (a Python annotation, a Rust attribute); the summary says so.
- A plugin producer that leaves marker texts out gets type 2 and 3 only, as before ADR-133, and `renamed` lists nothing for it.
- `edits` can repeat a removed subtree once per cluster member that lacks it; its size follows the cluster's.
