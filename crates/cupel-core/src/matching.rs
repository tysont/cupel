// ABOUTME: Symbol matching across refs (ADR-131): symbols one side removed paired with symbols the other added by
// ABOUTME: subtree-fingerprint similarity, one-to-one, one-to-many, and many-to-one; names and signatures break ties.
use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::finding::Symbol;
use crate::hash::SymbolHash;
use crate::similarity::{Fingerprints, jaccard};
use crate::syntax::{NormNode, NormTree, SymbolDef, SymbolKey, Symbols};

/// The least share of the smaller tree's fingerprints the larger must hold for two symbols to
/// be candidates: a part of a split, or a source of a merge, is contained in the whole.
pub const CONTAINMENT: f64 = 0.8;

/// The least Jaccard similarity of a match's two sides, each side taken as one tree.
pub const SET_SIMILARITY: f64 = 0.5;

/// The fewest nodes a tree needs to be matched. A smaller tree is mostly its signature and one
/// statement, so one holds most of the shapes of another that does something else.
pub const MIN_NODES: u32 = 30;

/// A symbol offered for matching: its definition, structural hash, and normalized tree.
#[derive(Debug, Clone, Copy)]
pub struct Candidate<'a> {
    pub def: &'a SymbolDef,
    pub hash: &'a SymbolHash,
    pub root: &'a NormNode,
}

/// Every symbol of `symbols` that `trees` normalized, in `symbols` order.
pub fn candidates<'a>(symbols: &'a Symbols, trees: &'a NormTree) -> Vec<Candidate<'a>> {
    let by_key: BTreeMap<&SymbolKey, (&SymbolHash, &NormNode)> = trees
        .symbols
        .iter()
        .map(|tree| (&tree.symbol, (&tree.hash, &tree.root)))
        .collect();
    symbols
        .symbols
        .iter()
        .filter_map(|def| {
            let (hash, root) = by_key.get(&def.key())?;
            Some(Candidate { def, hash, root })
        })
        .collect()
}

/// One member of a match.
#[derive(Debug, Clone, PartialEq)]
pub struct Matched {
    /// Where it is within its own side's run.
    pub key: SymbolKey,
    pub symbol: Symbol,
    /// The nodes of its normalized tree: its weight in a weighted mean.
    pub nodes: u32,
}

/// Symbols of the base side matched to symbols of the head side: one to one for a rename, a
/// move, or an edit; one to many for a split; many to one for a merge.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolMatch {
    /// In (file, line, column) order.
    pub base: Vec<Matched>,
    /// In (file, line, column) order.
    pub head: Vec<Matched>,
    /// The Jaccard similarity of the two sides' fingerprints, each side taken as one tree, to
    /// three decimals.
    pub similarity: f64,
}

/// The matches between the base side's symbols and the head side's whose structural hash the
/// other side lacks: a symbol whose hash both sides have is the same symbol (ADR-025).
pub fn across(base: &[Candidate<'_>], head: &[Candidate<'_>]) -> Vec<SymbolMatch> {
    let only = |side: &[Candidate<'_>], other: &[Candidate<'_>]| -> Vec<usize> {
        (0..side.len())
            .filter(|index| !other.iter().any(|o| o.hash == side[*index].hash))
            .collect()
    };
    let removed: Vec<Candidate<'_>> = only(base, head).into_iter().map(|i| base[i]).collect();
    let added: Vec<Candidate<'_>> = only(head, base).into_iter().map(|i| head[i]).collect();
    match_symbols(&removed, &added)
}

/// The matches between `removed` and `added` (ADR-131).
///
/// Two symbols are candidates when one holds at least `CONTAINMENT` of the other's fingerprints.
/// Each symbol keeps its best candidate on the other side, by containment, then Jaccard
/// similarity, then name similarity, then the same parameter count, then the same file; the
/// kept pairs' connected components are the matches. A member whose removal makes its match
/// more similar is dropped, and a match whose sides are less than `SET_SIMILARITY` similar is
/// no match. Trees under `MIN_NODES` nodes take no part.
pub fn match_symbols(removed: &[Candidate<'_>], added: &[Candidate<'_>]) -> Vec<SymbolMatch> {
    let removed = Printed::all(removed);
    let added = Printed::all(added);
    let mut components = Components::new(removed.len() + added.len());
    for (i, j) in kept_pairs(&removed, &added) {
        components.join(i, removed.len() + j);
    }
    let mut matches: Vec<SymbolMatch> = components
        .groups()
        .into_iter()
        .filter_map(|group| {
            let (base, head): (Vec<usize>, Vec<usize>) =
                group.into_iter().partition(|index| *index < removed.len());
            let base: Vec<&Printed<'_>> = base.into_iter().map(|i| &removed[i]).collect();
            let head: Vec<&Printed<'_>> = head
                .into_iter()
                .map(|i| &added[i - removed.len()])
                .collect();
            settle(base, head)
        })
        .collect();
    matches.sort_by(|a, b| a.base[0].key.cmp(&b.base[0].key));
    matches
}

/// A candidate with its fingerprints.
struct Printed<'a> {
    candidate: Candidate<'a>,
    prints: Fingerprints,
}

impl<'a> Printed<'a> {
    /// Every candidate with at least `MIN_NODES` nodes, fingerprinted.
    fn all(candidates: &[Candidate<'a>]) -> Vec<Self> {
        candidates
            .iter()
            .map(|candidate| Self {
                candidate: *candidate,
                prints: Fingerprints::of(candidate.root),
            })
            .filter(|printed| printed.prints.nodes >= MIN_NODES)
            .collect()
    }

    fn matched(&self) -> Matched {
        let def = self.candidate.def;
        Matched {
            key: def.key(),
            symbol: Symbol {
                display: def.display.clone(),
                hash: self.candidate.hash.clone(),
                lang: def.lang.clone(),
            },
            nodes: self.prints.nodes,
        }
    }
}

/// How well two candidates match, compared field by field in order.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
struct Score {
    containment: f64,
    jaccard: f64,
    name: f64,
    parameters: bool,
    file: bool,
}

impl Score {
    /// The score of `a` and `b`, when they are candidates.
    fn of(a: &Printed<'_>, b: &Printed<'_>) -> Option<Self> {
        let (x, y) = (&a.prints.set, &b.prints.set);
        let smaller = x.len().min(y.len());
        if smaller == 0 {
            return None;
        }
        let containment = ratio(shared(x, y), smaller);
        let (left, right) = (a.candidate.def, b.candidate.def);
        (containment >= CONTAINMENT).then(|| Self {
            containment,
            jaccard: jaccard(x, y),
            name: name_similarity(&left.name, &right.name),
            parameters: left.parameters == right.parameters,
            file: left.file == right.file,
        })
    }
}

/// The index of the best score, the first of equals; `None` when there is no score.
fn best(scores: impl Iterator<Item = Option<Score>>) -> Option<usize> {
    let mut best: Option<(usize, Score)> = None;
    for (index, score) in scores.enumerate() {
        let Some(score) = score else { continue };
        if best.is_none_or(|(_, held)| score.partial_cmp(&held) == Some(Ordering::Greater)) {
            best = Some((index, score));
        }
    }
    best.map(|(index, _)| index)
}

/// The pairs `(removed, added)` kept: each symbol's best candidate on the other side.
fn kept_pairs(removed: &[Printed<'_>], added: &[Printed<'_>]) -> Vec<(usize, usize)> {
    let scores: Vec<Vec<Option<Score>>> = removed
        .iter()
        .map(|a| added.iter().map(|b| Score::of(a, b)).collect())
        .collect();
    let mut pairs: Vec<(usize, usize)> = scores
        .iter()
        .enumerate()
        .filter_map(|(i, row)| Some((i, best(row.iter().copied())?)))
        .collect();
    pairs
        .extend((0..added.len()).filter_map(|j| Some((best(scores.iter().map(|row| row[j]))?, j))));
    pairs.sort_unstable();
    pairs.dedup();
    pairs
}

/// The member whose removal makes a match most similar, when one does: whether it is on the
/// base side, and where.
fn worst_member(base: &[&Printed<'_>], head: &[&Printed<'_>]) -> Option<(bool, usize)> {
    let current = set_similarity(base, head);
    let mut found: Option<(bool, usize, f64)> = None;
    for (on_base, side) in [(true, base), (false, head)] {
        if side.len() < 2 {
            continue;
        }
        for at in 0..side.len() {
            let mut fewer = side.to_vec();
            fewer.remove(at);
            let value = if on_base {
                set_similarity(&fewer, head)
            } else {
                set_similarity(base, &fewer)
            };
            if value > current && found.is_none_or(|(_, _, held)| value > held) {
                found = Some((on_base, at, value));
            }
        }
    }
    found.map(|(on_base, at, _)| (on_base, at))
}

/// The match one component makes, after dropping members that make it less similar; `None`
/// when its sides are less than `SET_SIMILARITY` similar.
fn settle(mut base: Vec<&Printed<'_>>, mut head: Vec<&Printed<'_>>) -> Option<SymbolMatch> {
    while let Some((on_base, at)) = worst_member(&base, &head) {
        if on_base {
            base.remove(at);
        } else {
            head.remove(at);
        }
    }
    let similarity = set_similarity(&base, &head);
    if similarity < SET_SIMILARITY {
        return None;
    }
    let members = |side: &[&Printed<'_>]| -> Vec<Matched> {
        let mut members: Vec<Matched> = side.iter().map(|printed| printed.matched()).collect();
        members.sort_by(|a, b| a.key.cmp(&b.key));
        members
    };
    Some(SymbolMatch {
        base: members(&base),
        head: members(&head),
        similarity: (similarity * 1000.0).round() / 1000.0,
    })
}

/// The Jaccard similarity of two sides' fingerprints, each side's trees taken as the children
/// of one root, so a shape repeated across members counts once per occurrence.
fn set_similarity(base: &[&Printed<'_>], head: &[&Printed<'_>]) -> f64 {
    let prints = |side: &[&Printed<'_>]| {
        let roots = side
            .iter()
            .map(|printed| printed.candidate.root.clone())
            .collect();
        Fingerprints::of(&NormNode::new("$set", roots)).set
    };
    jaccard(&prints(base), &prints(head))
}

/// The elements two sorted sequences share, each counted as often as both hold it.
fn shared<T: Ord>(a: &[T], b: &[T]) -> usize {
    let (mut i, mut j, mut count) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            Ordering::Less => i += 1,
            Ordering::Greater => j += 1,
            Ordering::Equal => {
                count += 1;
                i += 1;
                j += 1;
            }
        }
    }
    count
}

/// `part / whole` for counts.
fn ratio(part: usize, whole: usize) -> f64 {
    let count = |n: usize| f64::from(u32::try_from(n).unwrap_or(u32::MAX));
    count(part) / count(whole)
}

/// Disjoint sets over `0..n`, joined pair by pair.
struct Components {
    parent: Vec<usize>,
}

impl Components {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
        }
    }

    fn root(&mut self, mut index: usize) -> usize {
        while self.parent[index] != index {
            self.parent[index] = self.parent[self.parent[index]];
            index = self.parent[index];
        }
        index
    }

    fn join(&mut self, a: usize, b: usize) {
        let (a, b) = (self.root(a), self.root(b));
        if a != b {
            self.parent[a.max(b)] = a.min(b);
        }
    }

    /// Every set with at least two members, each in index order.
    fn groups(mut self) -> Vec<Vec<usize>> {
        let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for index in 0..self.parent.len() {
            let root = self.root(index);
            groups.entry(root).or_default().push(index);
        }
        groups
            .into_values()
            .filter(|group| group.len() > 1)
            .collect()
    }
}

/// The Dice coefficient of two names' letter pairs, ignoring case: twice the pairs they share
/// over all their pairs. Names too short to have a pair score 1 when equal and 0 otherwise.
fn name_similarity(a: &str, b: &str) -> f64 {
    let pairs = |name: &str| -> Vec<(char, char)> {
        let letters: Vec<char> = name.to_lowercase().chars().collect();
        let mut pairs: Vec<(char, char)> = letters.windows(2).map(|w| (w[0], w[1])).collect();
        pairs.sort_unstable();
        pairs
    };
    let (x, y) = (pairs(a), pairs(b));
    if x.is_empty() || y.is_empty() {
        return if a.to_lowercase() == b.to_lowercase() {
            1.0
        } else {
            0.0
        };
    }
    2.0 * ratio(shared(&x, &y), x.len() + y.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::LangId;
    use crate::similarity::mix;
    use crate::syntax::{Span, SymbolKind, Visibility};

    /// splitmix64, for reproducible synthetic trees.
    struct Rng(u64);

    impl Rng {
        fn below(&mut self, n: usize) -> usize {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            usize::try_from(mix(self.0) % n as u64).unwrap()
        }
    }

    const LABELS: [&str; 16] = [
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
        "let",
        "tuple",
        "field",
    ];

    /// A random statement-like subtree `depth` levels deep, each node with a leaf.
    fn statement(rng: &mut Rng, depth: u32) -> NormNode {
        let label = LABELS[rng.below(LABELS.len())];
        let width = if depth == 0 { 0 } else { 1 + rng.below(3) };
        let mut children: Vec<NormNode> = (0..width).map(|_| statement(rng, depth - 1)).collect();
        children.push(NormNode::leaf(["$id", "$lit", "+", "<"][rng.below(4)]));
        NormNode::new(label, children)
    }

    /// `count` random statements from `seed`.
    fn statements(seed: u64, count: usize) -> Vec<NormNode> {
        let mut rng = Rng(seed);
        (0..count).map(|_| statement(&mut rng, 3)).collect()
    }

    /// A function whose body is `body`.
    fn function(body: Vec<NormNode>) -> NormNode {
        NormNode::new(
            "function",
            vec![
                NormNode::new("parameters", vec![NormNode::leaf("$id")]),
                NormNode::new("block", body),
            ],
        )
    }

    /// One side's symbols: (name, file, tree) each, laid out one per ten lines of its file.
    struct Side {
        defs: Vec<SymbolDef>,
        hashes: Vec<SymbolHash>,
        roots: Vec<NormNode>,
    }

    impl Side {
        fn of(symbols: Vec<(&str, &str, NormNode)>) -> Self {
            let mut side = Self {
                defs: Vec::new(),
                hashes: Vec::new(),
                roots: Vec::new(),
            };
            for (index, (name, file, root)) in symbols.into_iter().enumerate() {
                let line = u32::try_from(index).unwrap() * 10 + 1;
                side.defs.push(SymbolDef {
                    name: name.to_owned(),
                    namespace: String::new(),
                    display: name.to_owned(),
                    kind: SymbolKind::Function,
                    visibility: Visibility::Public,
                    lang: LangId::from("rust"),
                    file: file.to_owned(),
                    span: Span {
                        line,
                        col: 1,
                        end_line: line + 8,
                        end_col: 2,
                    },
                    body: None,
                    parameters: 1,
                });
                let json = serde_json::to_string(&root).unwrap();
                let digest = blake3::hash(json.as_bytes()).to_hex();
                side.hashes
                    .push(format!("sh_{}", &digest[..16]).parse().unwrap());
                side.roots.push(root);
            }
            side
        }

        fn candidates(&self) -> Vec<Candidate<'_>> {
            (0..self.defs.len())
                .map(|index| Candidate {
                    def: &self.defs[index],
                    hash: &self.hashes[index],
                    root: &self.roots[index],
                })
                .collect()
        }
    }

    /// Each match as (base names, head names).
    fn names(matches: &[SymbolMatch]) -> Vec<(Vec<&str>, Vec<&str>)> {
        let display = |members: &[Matched]| -> Vec<String> {
            members
                .iter()
                .map(|member| member.symbol.display.clone())
                .collect()
        };
        matches
            .iter()
            .map(|found| (leak(display(&found.base)), leak(display(&found.head))))
            .collect()
    }

    fn leak(names: Vec<String>) -> Vec<&'static str> {
        names
            .into_iter()
            .map(|name| &*Box::leak(name.into_boxed_str()))
            .collect()
    }

    fn matched(base: &Side, head: &Side) -> Vec<SymbolMatch> {
        across(&base.candidates(), &head.candidates())
    }

    #[test]
    fn a_renamed_and_edited_function_matches_its_predecessor() {
        let body = statements(1, 8);
        let mut edited = body.clone();
        edited[3] = statements(99, 1).remove(0);
        let base = Side::of(vec![("settle", "src/a.rs", function(body))]);
        let head = Side::of(vec![("settle_invoices", "src/a.rs", function(edited))]);
        let found = matched(&base, &head);
        assert_eq!(names(&found), [(vec!["settle"], vec!["settle_invoices"])]);
        assert!(found[0].similarity >= 0.5 && found[0].similarity < 1.0);
    }

    #[test]
    fn a_function_moved_to_another_file_and_edited_matches() {
        let body = statements(2, 8);
        let mut edited = body.clone();
        edited.push(statements(98, 1).remove(0));
        let base = Side::of(vec![("price", "src/a.rs", function(body))]);
        let head = Side::of(vec![("price", "src/b.rs", function(edited))]);
        let found = matched(&base, &head);
        assert_eq!(names(&found), [(vec!["price"], vec!["price"])]);
        assert_eq!(found[0].head[0].key.file, "src/b.rs");
    }

    #[test]
    fn a_split_matches_one_base_symbol_to_both_parts() {
        let body = statements(3, 10);
        let base = Side::of(vec![("reconcile", "src/a.rs", function(body.clone()))]);
        let head = Side::of(vec![
            ("reconcile", "src/a.rs", function(body[..6].to_vec())),
            ("match_payment", "src/a.rs", function(body[6..].to_vec())),
        ]);
        let found = matched(&base, &head);
        assert_eq!(
            names(&found),
            [(vec!["reconcile"], vec!["reconcile", "match_payment"])]
        );
        assert!(found[0].head.iter().all(|member| member.nodes > 0));
    }

    #[test]
    fn a_merge_matches_both_base_symbols_to_one() {
        let body = statements(4, 10);
        let base = Side::of(vec![
            ("load", "src/a.rs", function(body[..5].to_vec())),
            ("parse", "src/a.rs", function(body[5..].to_vec())),
        ]);
        let head = Side::of(vec![("load_and_parse", "src/a.rs", function(body))]);
        assert_eq!(
            names(&matched(&base, &head)),
            [(vec!["load", "parse"], vec!["load_and_parse"])]
        );
    }

    #[test]
    fn unrelated_symbols_and_unchanged_ones_are_not_matched() {
        let kept = function(statements(5, 6));
        let base = Side::of(vec![
            ("kept", "src/a.rs", kept.clone()),
            ("gone", "src/a.rs", function(statements(6, 8))),
        ]);
        let head = Side::of(vec![
            ("kept_renamed", "src/a.rs", kept),
            ("fresh", "src/a.rs", function(statements(7, 8))),
        ]);
        assert_eq!(names(&matched(&base, &head)), Vec::new());
    }

    #[test]
    fn trees_too_small_to_tell_apart_are_not_matched() {
        // Two one-assertion Go tests, `assert.Equal(t, 10, 5+5)` deleted and
        // `assert.Equal(t, 7, 7)` added: their trees (22 and 19 nodes) are mostly the test
        // signature, so one holds most of the other's shapes though they test different things.
        let test = |arguments: Vec<NormNode>| {
            let leaf = NormNode::leaf;
            let node = NormNode::new;
            let signature = node(
                "parameter_list",
                vec![node(
                    "parameter_declaration",
                    vec![
                        leaf("$id"),
                        node(
                            "pointer_type",
                            vec![
                                leaf("*"),
                                node("qualified_type", vec![leaf("$id"), leaf("$id")]),
                            ],
                        ),
                    ],
                )],
            );
            let call = node(
                "call_expression",
                vec![
                    node("selector_expression", vec![leaf("$id"), leaf("Equal")]),
                    node("argument_list", arguments),
                ],
            );
            let body = node(
                "block",
                vec![node(
                    "statement_list",
                    vec![node("expression_statement", vec![call])],
                )],
            );
            node("function_declaration", vec![signature, body])
        };
        let (id, lit) = (NormNode::leaf("$id"), NormNode::leaf("$lit"));
        let sum = NormNode::new(
            "binary_expression",
            vec![lit.clone(), NormNode::leaf("+"), lit.clone()],
        );
        let base = Side::of(vec![(
            "TestGoesAway",
            "weak_test.go",
            test(vec![id.clone(), lit.clone(), sum]),
        )]);
        let head = Side::of(vec![(
            "TestAddsACheck",
            "weak_test.go",
            test(vec![id, lit.clone(), lit]),
        )]);
        assert_eq!(names(&matched(&base, &head)), Vec::new());
    }

    #[test]
    fn identical_twins_pair_by_name() {
        let body = statements(8, 8);
        let mut edited = body.clone();
        edited.push(statements(97, 1).remove(0));
        let base = Side::of(vec![
            ("parse_header", "src/a.rs", function(body.clone())),
            ("parse_footer", "src/a.rs", function(body)),
        ]);
        let head = Side::of(vec![
            ("read_footer", "src/a.rs", function(edited.clone())),
            ("read_header", "src/a.rs", function(edited)),
        ]);
        assert_eq!(
            names(&matched(&base, &head)),
            [
                (vec!["parse_header"], vec!["read_header"]),
                (vec!["parse_footer"], vec!["read_footer"]),
            ]
        );
    }

    #[test]
    fn a_deleted_function_does_not_match_a_small_one_built_from_its_parts() {
        let body = statements(9, 12);
        let base = Side::of(vec![("everything", "src/a.rs", function(body.clone()))]);
        let head = Side::of(vec![("piece", "src/a.rs", function(body[..2].to_vec()))]);
        assert_eq!(names(&matched(&base, &head)), Vec::new());
    }

    #[test]
    fn a_member_that_only_repeats_what_the_true_successor_holds_is_left_out() {
        let body = statements(10, 10);
        let mut edited = body.clone();
        edited[0] = statements(96, 1).remove(0);
        let base = Side::of(vec![("total", "src/a.rs", function(body.clone()))]);
        let head = Side::of(vec![
            ("total", "src/a.rs", function(edited)),
            ("copy_of_a_part", "src/b.rs", function(body[4..7].to_vec())),
        ]);
        assert_eq!(
            names(&matched(&base, &head)),
            [(vec!["total"], vec!["total"])]
        );
    }

    #[test]
    fn candidates_join_symbols_with_their_trees_by_key() {
        let side = Side::of(vec![("a", "src/a.rs", function(statements(11, 3)))]);
        let symbols = Symbols {
            symbols: side.defs.clone(),
        };
        let trees = NormTree {
            symbols: vec![crate::syntax::NormalizedSymbol {
                symbol: side.defs[0].key(),
                hash: side.hashes[0].clone(),
                root: side.roots[0].clone(),
            }],
        };
        let joined = candidates(&symbols, &trees);
        assert_eq!(joined.len(), 1);
        assert_eq!(joined[0].hash, &side.hashes[0]);
    }

    #[test]
    fn name_similarity_is_the_share_of_shared_letter_pairs() {
        let shares = [
            name_similarity("parse_header", "parse_header"),
            name_similarity("ab", "cd"),
            name_similarity("night", "nacht"),
        ];
        assert_eq!(
            shares.map(|share| format!("{share:.2}")),
            ["1.00", "0.00", "0.25"]
        );
    }
}
