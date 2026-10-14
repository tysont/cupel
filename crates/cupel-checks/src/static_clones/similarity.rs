// ABOUTME: Near-duplicate search for static.clones: per-tree subtree fingerprint sets, MinHash signatures,
// ABOUTME: LSH banding into candidate pairs, and exact Jaccard similarity (design §9.2, ADR-050).
use cupel_core::syntax::NormNode;

/// The length of a min-hash signature: the number of independent hash functions.
pub const SIGNATURE: usize = 128;

/// The largest chance of missing a pair at the recall bound that banding may accept (ADR-050).
const MISS_BOUND: f64 = 1e-6;

/// How much above the threshold a pair's similarity must be for the miss bound to hold.
pub const RECALL_MARGIN: f64 = 0.05;

/// A tree's fingerprint set: one element per non-leaf node's bounded-depth shape, repeated
/// shapes told apart by their occurrence, so set Jaccard is the multiset Jaccard of shapes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprints {
    /// Sorted and distinct.
    pub set: Vec<u64>,
    /// Every node of the tree, leaves included.
    pub nodes: u32,
}

impl Fingerprints {
    pub fn of(root: &NormNode) -> Self {
        let mut shapes = Vec::new();
        let mut nodes = 0;
        visit(root, &mut shapes, &mut nodes);
        shapes.sort_unstable();
        let mut set = Vec::with_capacity(shapes.len());
        let mut occurrence = 0;
        for (index, shape) in shapes.iter().enumerate() {
            occurrence = if index > 0 && shapes[index - 1] == *shape {
                occurrence + 1
            } else {
                0
            };
            set.push(combine(*shape, occurrence));
        }
        Self::from_elements(set, nodes)
    }

    /// A set from arbitrary elements, sorted and de-duplicated.
    pub fn from_elements(mut set: Vec<u64>, nodes: u32) -> Self {
        set.sort_unstable();
        set.dedup();
        Self { set, nodes }
    }
}

/// Visits `node`'s subtree in post-order, pushing the fingerprint of every non-leaf node: its
/// shape two levels deep (the node, its children, and their children, ADR-050). Returns the
/// node's label hash and its shape one level deep, from which its parent's shape is folded,
/// so each label is hashed once.
fn visit(node: &NormNode, shapes: &mut Vec<u64>, nodes: &mut u32) -> (u64, u64) {
    *nodes += 1;
    let label = label(&node.label);
    let count = node.children.len() as u64;
    let mut one = combine(label, count);
    let mut two = one;
    for child in &node.children {
        let (child_label, child_one) = visit(child, shapes, nodes);
        one = combine(one, child_label);
        two = combine(two, child_one);
    }
    if count > 0 {
        shapes.push(two);
    }
    (label, one)
}

/// FNV-1a over the label's bytes, then mixed: stable across platforms and runs.
fn label(text: &str) -> u64 {
    let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    });
    mix(hash)
}

/// The splitmix64 finalizer: a bijection on `u64` that spreads every input bit.
pub const fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// Folds `value` into `hash`, order-sensitively.
pub fn combine(hash: u64, value: u64) -> u64 {
    mix(hash.rotate_left(23) ^ value.wrapping_add(0x9e37_79b9_7f4a_7c15))
}

/// The min-hash signature of `set`: for each of `SIGNATURE` hash functions, the least hash of
/// any element. Two sets agree at a position with probability equal to their Jaccard
/// similarity. The functions are `x ↦ mix(x ⊕ seedᵢ)` with fixed seeds, so signatures are
/// deterministic.
pub fn signature(set: &[u64]) -> [u64; SIGNATURE] {
    let mut minimums = [u64::MAX; SIGNATURE];
    for element in set {
        for (minimum, seed) in minimums.iter_mut().zip(SEEDS.iter()) {
            *minimum = (*minimum).min(mix(element ^ seed));
        }
    }
    minimums
}

/// The seeds of the signature's hash functions: a splitmix64 sequence from a fixed start.
static SEEDS: [u64; SIGNATURE] = seeds();

const fn seeds() -> [u64; SIGNATURE] {
    let mut seeds = [0; SIGNATURE];
    let mut state: u64 = 0x0005_eedc_105e;
    let mut index = 0;
    while index < SIGNATURE {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        seeds[index] = mix(state);
        index += 1;
    }
    seeds
}

/// How a signature is cut into bands of rows for locality-sensitive hashing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Banding {
    pub bands: usize,
    pub rows: usize,
}

impl Banding {
    /// The most rows per band (the fewest false candidates) for which a pair whose Jaccard
    /// similarity is `threshold + RECALL_MARGIN` shares no band with probability at most
    /// `MISS_BOUND`. Two sets share a band with probability `1 − (1 − Jʳ)ᵇ`.
    pub fn for_threshold(threshold: f64) -> Self {
        let target = (threshold + RECALL_MARGIN).min(1.0);
        (1..=SIGNATURE)
            .rev()
            .map(|rows| Self {
                bands: SIGNATURE / rows,
                rows,
            })
            .find(|banding| banding.miss(target) <= MISS_BOUND)
            .unwrap_or(Self {
                bands: SIGNATURE,
                rows: 1,
            })
    }

    /// The probability that two sets of Jaccard similarity `jaccard` share no band.
    pub fn miss(self, jaccard: f64) -> f64 {
        let rows = i32::try_from(self.rows).unwrap_or(i32::MAX);
        let bands = i32::try_from(self.bands).unwrap_or(i32::MAX);
        (1.0 - jaccard.powi(rows)).powi(bands)
    }

    /// Every pair `(i, j)`, `i < j`, of signatures that agree on all rows of some band,
    /// sorted and distinct.
    pub fn candidates(self, signatures: &[[u64; SIGNATURE]]) -> Vec<(usize, usize)> {
        let mut pairs = Vec::new();
        let mut keyed: Vec<(u64, usize)> = Vec::with_capacity(signatures.len());
        for band in 0..self.bands {
            keyed.clear();
            let rows = band * self.rows..(band + 1) * self.rows;
            keyed.extend(signatures.iter().enumerate().map(|(index, signature)| {
                let key = signature[rows.clone()]
                    .iter()
                    .fold(band as u64, |key, row| combine(key, *row));
                (key, index)
            }));
            keyed.sort_unstable();
            for bucket in keyed.chunk_by(|a, b| a.0 == b.0) {
                for (at, (_, i)) in bucket.iter().enumerate() {
                    pairs.extend(bucket[at + 1..].iter().map(|(_, j)| (*i, *j)));
                }
            }
        }
        pairs.sort_unstable();
        pairs.dedup();
        pairs
    }
}

/// |a ∩ b| / |a ∪ b| of two sorted, distinct sets; 0 for two empty sets.
pub fn jaccard(a: &[u64], b: &[u64]) -> f64 {
    let (mut i, mut j, mut shared) = (0, 0, 0_u32);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                shared += 1;
                i += 1;
                j += 1;
            }
        }
    }
    let total = u32::try_from(a.len() + b.len()).unwrap_or(u32::MAX);
    let union = total - shared;
    if union == 0 {
        0.0
    } else {
        f64::from(shared) / f64::from(union)
    }
}

/// Two sets at least as similar as the threshold, by index.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Similar {
    pub left: usize,
    pub right: usize,
    pub jaccard: f64,
}

/// The similar pairs among a collection of sets, and how much exact comparison found them.
#[derive(Debug, Clone, PartialEq)]
pub struct Search {
    /// Sorted by `(left, right)`, `left < right`.
    pub pairs: Vec<Similar>,
    /// Candidate pairs whose Jaccard similarity was computed exactly.
    pub compared: usize,
}

/// Every pair of non-empty sets whose Jaccard similarity is at least `threshold`, found by
/// banding min-hash signatures and comparing only the candidates that share a band and whose
/// sizes allow the threshold (`|a| / |b| ≥ threshold` when `|a| ≤ |b|`).
pub fn search(sets: &[Fingerprints], threshold: f64) -> Search {
    let present: Vec<usize> = (0..sets.len())
        .filter(|index| !sets[*index].set.is_empty())
        .collect();
    let signatures: Vec<[u64; SIGNATURE]> = present
        .iter()
        .map(|index| signature(&sets[*index].set))
        .collect();
    let mut compared = 0;
    let mut pairs = Vec::new();
    for (i, j) in Banding::for_threshold(threshold).candidates(&signatures) {
        let (left, right) = (present[i], present[j]);
        let (a, b) = (&sets[left].set, &sets[right].set);
        let size = |set: &[u64]| f64::from(u32::try_from(set.len()).unwrap_or(u32::MAX));
        if size(a).min(size(b)) < threshold * size(a).max(size(b)) {
            continue;
        }
        compared += 1;
        let jaccard = jaccard(a, b);
        if jaccard >= threshold {
            pairs.push(Similar {
                left,
                right,
                jaccard,
            });
        }
    }
    Search { pairs, compared }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn node(label: &str, children: Vec<NormNode>) -> NormNode {
        NormNode {
            label: label.to_owned(),
            children,
        }
    }

    fn leaf(label: &str) -> NormNode {
        node(label, Vec::new())
    }

    /// `statements` copies of `$id = $id + $lit` in a block under a function.
    fn function(statements: usize) -> NormNode {
        let statement = node(
            "assignment",
            vec![
                leaf("$id"),
                leaf("="),
                node("binary", vec![leaf("$id"), leaf("+"), leaf("$lit")]),
            ],
        );
        node(
            "function",
            vec![
                node("parameters", vec![leaf("$id")]),
                node("block", vec![statement; statements]),
            ],
        )
    }

    #[test]
    fn fingerprints_count_every_node_and_fingerprint_every_inner_node() {
        let prints = Fingerprints::of(&function(2));
        // function, parameters, $id, block, then 2 × (assignment, $id, =, binary, $id, +, $lit).
        assert_eq!(prints.nodes, 18);
        // function, parameters, block, and 2 × (assignment, binary).
        assert_eq!(prints.set.len(), 7);
    }

    #[test]
    fn repeated_shapes_count_once_per_occurrence() {
        let two = Fingerprints::of(&function(2));
        let four = Fingerprints::of(&function(4));
        assert_eq!(four.set.len(), two.set.len() + 4);
        // Shared: parameters and the first two assignments and binaries, of 7 + 11 − 5.
        assert!((jaccard(&two.set, &four.set) - 5.0 / 13.0).abs() < 1e-12);
    }

    #[test]
    fn equal_trees_have_equal_fingerprints_and_an_edit_changes_only_nearby_ones() {
        assert_eq!(
            Fingerprints::of(&function(6)),
            Fingerprints::of(&function(6))
        );
        let mut edited = function(6);
        edited.children[1].children[5].children[2].children[1] = leaf("-");
        let (a, b) = (Fingerprints::of(&function(6)), Fingerprints::of(&edited));
        // The binary and its assignment, which see the edited leaf within two levels, change;
        // the block, three levels up, does not.
        let shared = a.set.iter().filter(|print| b.set.contains(print)).count();
        assert_eq!(a.set.len() - shared, 2);
    }

    #[test]
    fn labels_and_order_both_matter() {
        let ab = node("call", vec![leaf("a"), leaf("b")]);
        let ba = node("call", vec![leaf("b"), leaf("a")]);
        let (ab, ba) = (Fingerprints::of(&ab), Fingerprints::of(&ba));
        // Each tree has one fingerprint, the call's, and the two have none in common.
        let shared = ab.set.iter().filter(|print| ba.set.contains(print)).count();
        assert_eq!((ab.set.len(), ba.set.len(), shared), (1, 1, 0));
    }

    #[test]
    fn jaccard_is_the_share_of_the_union_in_common() {
        let shares = [
            jaccard(&[1, 2, 3], &[2, 3, 4]),
            jaccard(&[1, 2], &[1, 2]),
            jaccard(&[1], &[2]),
            jaccard(&[], &[]),
        ];
        assert_eq!(
            shares.map(|share| share.to_string()),
            ["0.5", "1", "0", "0"]
        );
    }

    #[test]
    fn signatures_are_deterministic_and_estimate_jaccard() {
        let a = Fingerprints::from_elements((0..400).map(mix).collect(), 0);
        let b = Fingerprints::from_elements((100..500).map(mix).collect(), 0);
        assert_eq!(signature(&a.set), signature(&a.set));
        let agree = signature(&a.set)
            .iter()
            .zip(signature(&b.set).iter())
            .filter(|(x, y)| x == y)
            .count();
        // True Jaccard is 300 / 500 = 0.6; 128 positions estimate it within a few hundredths.
        let estimate = f64::from(u32::try_from(agree).unwrap()) / 128.0;
        assert!((estimate - 0.6).abs() < 0.15, "{estimate}");
    }

    #[test]
    fn banding_at_the_default_threshold_is_25_bands_of_5_rows() {
        let banding = Banding::for_threshold(0.82);
        assert_eq!(banding, Banding { bands: 25, rows: 5 });
        assert!(banding.miss(0.87) <= MISS_BOUND);
        assert!(Banding { bands: 21, rows: 6 }.miss(0.87) > MISS_BOUND);
    }

    #[test]
    fn a_lower_threshold_uses_fewer_rows_and_a_threshold_of_one_the_most() {
        assert!(Banding::for_threshold(0.5).rows < 5);
        assert_eq!(Banding::for_threshold(1.0).rows, SIGNATURE);
    }

    #[test]
    fn candidates_are_signatures_agreeing_on_a_whole_band() {
        let banding = Banding { bands: 2, rows: 2 };
        let mut a = [0; SIGNATURE];
        let mut b = [0; SIGNATURE];
        let mut c = [0; SIGNATURE];
        a[..4].copy_from_slice(&[1, 2, 3, 4]);
        b[..4].copy_from_slice(&[9, 9, 3, 4]);
        c[..4].copy_from_slice(&[1, 9, 3, 9]);
        assert_eq!(banding.candidates(&[a, b, c]), vec![(0, 1)]);
    }

    #[test]
    fn search_reports_similar_pairs_and_skips_empty_and_dissimilar_sets() {
        let base: Vec<u64> = (0..100).map(mix).collect();
        let mut near = base.clone();
        near[0] = mix(1_000);
        let sets = [
            Fingerprints::from_elements(base, 0),
            Fingerprints::from_elements(Vec::new(), 0),
            Fingerprints::from_elements((500..600).map(mix).collect(), 0),
            Fingerprints::from_elements(near, 0),
        ];
        let found = search(&sets, 0.82);
        assert_eq!(found.pairs.len(), 1);
        assert_eq!((found.pairs[0].left, found.pairs[0].right), (0, 3));
        assert!((found.pairs[0].jaccard - 99.0 / 101.0).abs() < 1e-12);
    }

    /// Two sets whose true Jaccard is at least `threshold + RECALL_MARGIN` (shared elements
    /// plus at most 13/87 as many extra ones split between them), and unrelated sets.
    fn similar_pair_and_noise() -> impl Strategy<Value = (Vec<u64>, Vec<u64>, Vec<Vec<u64>>)> {
        (20_usize..200)
            .prop_flat_map(|shared| {
                (
                    prop::collection::btree_set(any::<u64>(), shared),
                    prop::collection::vec(any::<u64>(), 0..=shared * 13 / 87),
                    any::<prop::sample::Index>(),
                    prop::collection::vec(prop::collection::vec(any::<u64>(), 20..100), 0..8),
                )
            })
            .prop_map(|(shared, extra, split, noise)| {
                let split = split.index(extra.len() + 1);
                let mut a: Vec<u64> = shared.iter().copied().collect();
                a.extend(&extra[..split]);
                let mut b: Vec<u64> = shared.into_iter().collect();
                b.extend(&extra[split..]);
                (a, b, noise)
            })
    }

    /// splitmix64, for reproducible synthetic trees.
    struct Rng(u64);

    impl Rng {
        fn below(&mut self, n: usize) -> usize {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            usize::try_from(mix(self.0) % n as u64).unwrap()
        }
    }

    const LABELS: [&str; 24] = [
        "if",
        "loop",
        "call",
        "binary",
        "assignment",
        "return",
        "block",
        "member",
        "index",
        "arguments",
        "unary",
        "match",
        "arm",
        "closure",
        "let",
        "tuple",
        "array",
        "field",
        "try",
        "await",
        "range",
        "cast",
        "struct",
        "macro",
    ];

    /// A random statement-like subtree `depth` levels deep, each node with a leaf.
    fn random_tree(rng: &mut Rng, depth: u32) -> NormNode {
        let label = LABELS[rng.below(LABELS.len())];
        let width = if depth == 0 { 0 } else { 1 + rng.below(3) };
        let mut children: Vec<NormNode> = (0..width).map(|_| random_tree(rng, depth - 1)).collect();
        children.push(leaf(["$id", "$lit", "+", "<"][rng.below(4)]));
        node(label, children)
    }

    /// Design §9.2's "approximately linear", for plan P2.4's 10k symbols: about 120 nodes
    /// each, every hundredth an edited copy of the one before. Exact comparisons stay linear
    /// in the symbol count, and the whole search (fingerprints included) takes under a second
    /// in a release build, about 0.2 s on an Apple M2. A debug build (`make test`) is about
    /// ten times slower, so it is held to a debug budget instead.
    /// `count` random function trees, every hundredth an edited copy of the one before.
    fn corpus(count: usize) -> Vec<NormNode> {
        let mut rng = Rng(7);
        let mut trees: Vec<NormNode> = Vec::with_capacity(count);
        for index in 0..count {
            let tree = if index % 100 == 99 {
                let mut copy = trees[index - 1].clone();
                copy.children.push(leaf("$id"));
                copy
            } else {
                node(
                    "function",
                    (0..4).map(|_| random_tree(&mut rng, 3)).collect(),
                )
            };
            trees.push(tree);
        }
        trees
    }

    /// Milliseconds the ten-thousand-symbol search may take: a debug build is about ten times
    /// slower than a release build.
    const SEARCH_BUDGET_MS: u128 = if cfg!(debug_assertions) {
        20_000
    } else {
        1_000
    };

    #[test]
    fn ten_thousand_symbols_are_searched_in_linear_work_and_under_a_second() {
        let trees = corpus(10_000);
        let started = std::time::Instant::now();
        let sets: Vec<Fingerprints> = trees.iter().map(Fingerprints::of).collect();
        let found = search(&sets, 0.82);
        let elapsed = started.elapsed();
        let planted: Vec<(usize, usize)> = (0..100).map(|n| (n * 100 + 98, n * 100 + 99)).collect();
        let missed: Vec<&(usize, usize)> = planted
            .iter()
            .filter(|pair| !found.pairs.iter().any(|p| (p.left, p.right) == **pair))
            .collect();
        assert_eq!(missed, Vec::<&(usize, usize)>::new());
        assert!(
            found.compared <= 2 * sets.len(),
            "{} exact comparisons",
            found.compared
        );
        assert!(elapsed.as_millis() < SEARCH_BUDGET_MS, "took {elapsed:?}");
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// The recall bound of plan P2.4: a pair at least `threshold + 0.05` similar is found,
        /// and every pair found is exactly as similar as reported and at least the threshold.
        #[test]
        fn search_never_misses_a_pair_above_the_recall_margin(
            (a, b, noise) in similar_pair_and_noise()
        ) {
            let threshold = 0.82;
            let mut sets = vec![
                Fingerprints::from_elements(a, 0),
                Fingerprints::from_elements(b, 0),
            ];
            prop_assume!(jaccard(&sets[0].set, &sets[1].set) >= threshold + RECALL_MARGIN);
            sets.extend(noise.into_iter().map(|set| Fingerprints::from_elements(set, 0)));
            let found = search(&sets, threshold);
            prop_assert!(found.pairs.iter().any(|pair| (pair.left, pair.right) == (0, 1)));
            for pair in &found.pairs {
                let exact = jaccard(&sets[pair.left].set, &sets[pair.right].set);
                prop_assert!(exact >= threshold);
                prop_assert!((pair.jaccard - exact).abs() < 1e-12);
            }
        }
    }
}
