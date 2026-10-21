# CUPEL_DECISION_050: `static.clones`: bounded-depth subtree fingerprints, MinHash and LSH parameters, pairs versus clusters

**Date:** 2026-10-13 · **Status:** accepted; the Type bullet amended by [136](CUPEL_DECISION_136.md) (types 1, 2, and 3 from a token digest, with what differs in evidence) · **Phase:** P2.4

## Context

Design §3 describes dryer: normalized trees, "structural fingerprints per subtree", Jaccard similarity, threshold 0.82, at least 4 lines and 20 nodes. §9.2 asks for MinHash signatures, LSH bands, and exact Jaccard only within buckets, "approximately linear". Plan P2.4 adds a recall bound: MinHash/LSH must never miss a pair whose true Jaccard is at least threshold + 0.05. §5.1 names the kinds `clone.pair` and `clone.cluster` and the clone types 1, 2, and 3; §7.2 requires both spans in a pair's evidence. Left open: what a "subtree fingerprint" is, the signature length and banding, how pairs relate to clusters, how a pair is identified, and how the types are told apart from a `NormTree`.

## Options

**Fingerprints.** (a) A Merkle hash of every complete subtree: one edit changes the fingerprint of every ancestor up to the root, so a 100-node function with one added statement scores about 0.7, and type-3 clones fall under 0.82 unless they are very large. (b) A hash of each non-leaf node with its descendants down to a fixed depth (the node, its children, its grandchildren): an edit changes the fingerprints of the nodes at most two levels above it, independent of tree height. (b) is still a subtree fingerprint, bounded. Sets lose multiplicity, so ten identical statements would count once; numbering repeated fingerprints by occurrence (`hash(shape, k)` for the k-th) makes set Jaccard equal multiset Jaccard, which MinHash estimates directly.

**Banding.** With signature length `k` cut into `b` bands of `r` rows, two sets of Jaccard `J` share a band with probability `1 − (1 − Jʳ)ᵇ`. More rows mean fewer false candidates (less quadratic work) and more misses. A fixed `(b, r)` holds the recall bound only for the default threshold; deriving `(b, r)` from the configured threshold keeps it for any threshold.

**Pairs and clusters.** Report every similar pair (a cluster of n symbols yields n·(n−1)/2 findings with one action) or report each connected group of similar symbols once: a pair when it has two members, a cluster when it has three or more.

**Clone type.** Type 1 (only layout and comments differ) and type 2 (names, literals, types differ too) normalize to the same tree, and `NormTree` keeps no identifier or literal text, so from the facts a check may read they cannot be told apart. Separating them needs a digest of the raw tokens on `NormTree` (a fact version bump in `cupel-core` and every language producer).

## Decision

- **Fingerprints** are option (b) with depth 2, over non-leaf nodes, labels hashed with FNV-1a and folded with the splitmix64 finalizer (stable across platforms and runs), repeated shapes numbered by occurrence. A symbol is compared when it spans at least `min_lines` lines (from `Symbols`) and its `NormTree` has at least `min_nodes` nodes, leaves included.
- **MinHash**: `k = 128` hash functions `x ↦ mix(x ⊕ seedᵢ)` with fixed seeds from a splitmix64 sequence.
- **Banding**: the largest `r` (fewest false candidates) with `b = ⌊128 / r⌋` such that a pair at `J = min(threshold + 0.05, 1)` is missed with probability at most 10⁻⁶. At the default 0.82 that is `r = 5`, `b = 25`: a pair at 0.87 is missed with probability 3·10⁻⁸, one at 0.82 with 10⁻⁵. Candidates whose set sizes cannot reach the threshold (`small < threshold · large`) are dropped before the exact Jaccard, which is a merge of the two sorted sets.
- **Grouping**: the similarity graph's connected components. A two-member component is one `clone.pair`; three or more are one `clone.cluster` whose evidence lists every member, the number of similar pairs among them (so how "mutual" the cluster is shows), and the least and greatest similarity. A symbol never pairs with a symbol whose span contains it (a closure and its function).
- **Type**: `clone_type` is 2 when the two normalized trees are equal and 3 otherwise. Type 1 is reported as 2 until `NormTree` carries a raw-token digest; see Consequences.
- **Identity**: a finding is about its first member in (file, line, column) order; its site hash folds the other members' structural hashes, so `FindingId::compute(check, kind, first.hash, site)` is stable under reformatting and moves and changes when membership changes. The repro selects the first member's hash; the check compares every symbol it is given and reports a group when any member is in scope, so the repro reproduces the finding.
- **Score** is the number of similar pairs (unit `pairs`), summed across families; `n` is the number of symbols compared. Metrics declared for gates: `pairs`, `clusters`, `max_jaccard`.
- **Similarity in evidence** is rounded to three decimals.

## Consequences

- One local edit costs a bounded number of fingerprints, so type-3 clones of ordinary functions (around 90 nodes and up) clear 0.82; very small functions need near-identical trees.
- A tree with many repeated statements scores by multiset overlap; two functions that are each a long run of the same statement compare as similar, which is what a reader would call a clone.
- Lowering the threshold lowers `r`, so the candidate count grows: the cost follows the requested recall. The 10k-symbol performance test runs at the default.
- `clone_type` never reads 1. Adding a token digest to `NormTree` (version 2) would let it, at the price of a fact change in every language; recorded as a P2.4 follow-up in the plan notes.
- A large component can chain symbols that are not pairwise similar; its `pairs` count against n·(n−1)/2 says so.
