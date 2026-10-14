// ABOUTME: `TableNormalize`: each symbol's normalized syntax tree under `[normalize]` (locals, fields,
// ABOUTME: and literals become markers; operators, call names, and shape stay) and its structural hash.
use cupel_core::syntax::{NormNode, NormTree, NormalizedSymbol};
use cupel_core::{FactKind, LangId, SymbolHash};
use tree_sitter::Node;

use crate::parse::Parsed;
use crate::producer::{QueryProducer, Source};
use crate::spec::QueryFile;
use crate::symbols::definitions;
use crate::tables::NormalizeRules;

/// Characters an anonymous token may consist of to be kept as an operator.
const OPERATOR_CHARS: &str = "+-*/%=!<>&|^~?";

inventory::submit!(crate::generic::GenericProducer {
    kind: FactKind::NormTree,
    drives: |spec| spec.has(QueryFile::Symbols) && spec.tables().normalize.is_some(),
    attach: |language, cache, reg| {
        reg.register::<NormTree, _>(QueryProducer::new(language, cache, extract));
    },
});

/// The `NormTree` extractor. Registered only for languages with a `[normalize]` table.
pub fn extract(source: &Source<'_>) -> NormTree {
    let Some(rules) = source.language.spec().tables().normalize.as_ref() else {
        return NormTree::default();
    };
    let lang = source.language.id();
    NormTree {
        symbols: definitions(source.parsed, source.language)
            .iter()
            .map(|definition| {
                let normalizer = Normalizer {
                    rules,
                    parsed: source.parsed,
                    skip: definition.name.id(),
                };
                let root = normalizer.root(definition.node);
                NormalizedSymbol {
                    symbol: definition.symbol.key(),
                    hash: structural_hash(&lang, &root),
                    root,
                }
            })
            .collect(),
    }
}

/// `sh_` plus the first 64 bits of `blake3(lang ∥ 0x00 ∥ json(root))`, as hex (ADR-025).
///
/// # Panics
///
/// Never: sixteen lowercase hex digits after `sh_` are always a valid `SymbolHash`.
pub fn structural_hash(lang: &LangId, root: &NormNode) -> SymbolHash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(lang.as_str().as_bytes());
    hasher.update(&[0]);
    hasher.update(&serde_json::to_vec(root).unwrap_or_default());
    format!("sh_{}", hex16(hasher.finalize().as_bytes()))
        .parse()
        .expect("sixteen hex digits make a valid symbol hash")
}

/// The first eight bytes of a digest as sixteen lowercase hex digits.
pub fn hex16(digest: &[u8; 32]) -> String {
    use std::fmt::Write as _;
    let mut hex = String::with_capacity(16);
    for byte in &digest[..8] {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Whether identifiers keep their text: inside a callee, outside its receivers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Plain,
    Callee,
}

struct Normalizer<'a> {
    rules: &'a NormalizeRules,
    parsed: &'a Parsed,
    /// The symbol's own name node, which is dropped so renaming the symbol keeps its hash.
    skip: usize,
}

impl Normalizer<'_> {
    fn root(&self, node: Node<'_>) -> NormNode {
        let mut nodes = self.node(node, Mode::Plain);
        if nodes.len() == 1 {
            nodes.remove(0)
        } else {
            NormNode::new(node.kind(), nodes)
        }
    }

    /// The normalized form of `node`: none (erased), one node, or several (flattened).
    fn node(&self, node: Node<'_>, mode: Mode) -> Vec<NormNode> {
        if let Some(terminal) = self.terminal(node, mode) {
            return terminal;
        }
        let kind = node.kind();
        let callee = self
            .rules
            .calls
            .iter()
            .find(|call| call.kind == kind)
            .map(|call| call.callee.as_str());
        let mut children = Vec::new();
        let mut cursor = node.walk();
        for (index, child) in node.children(&mut cursor).enumerate() {
            let field = u32::try_from(index)
                .ok()
                .and_then(|index| node.field_name_for_child(index));
            children.extend(self.node(child, self.child_mode(mode, callee, field)));
        }
        if self.rules.flatten.iter().any(|k| k == kind) {
            return children;
        }
        vec![NormNode::new(kind, children)]
    }
}

impl Normalizer<'_> {
    /// What `node` normalizes to when its children do not matter: nothing for an erased,
    /// skipped, or extra node or a non-operator token, a leaf for an operator, identifier, or
    /// literal; `None` for a node whose children decide.
    fn terminal(&self, node: Node<'_>, mode: Mode) -> Option<Vec<NormNode>> {
        let kind = node.kind();
        let has = |list: &[String]| list.iter().any(|k| k == kind);
        if node.id() == self.skip || node.is_extra() || has(&self.rules.erase) {
            return Some(Vec::new());
        }
        if !node.is_named() {
            let text = self.parsed.text(node);
            let operator = !text.is_empty() && text.chars().all(|c| OPERATOR_CHARS.contains(c));
            let kept = operator || self.rules.operators.iter().any(|o| o == text);
            return Some(if kept {
                vec![NormNode::leaf(text)]
            } else {
                Vec::new()
            });
        }
        if has(&self.rules.identifiers) {
            return Some(vec![match mode {
                Mode::Callee => NormNode::leaf(self.parsed.text(node)),
                Mode::Plain => NormNode::leaf("$id"),
            }]);
        }
        has(&self.rules.literals).then(|| vec![NormNode::leaf("$lit")])
    }

    /// How the child in `field` of a node normalized in `mode` normalizes, where `callee` is the
    /// field naming the called function when the node is a call.
    fn child_mode(&self, mode: Mode, callee: Option<&str>, field: Option<&str>) -> Mode {
        match (mode, field) {
            (_, Some(field)) if callee == Some(field) => Mode::Callee,
            (Mode::Callee, Some(field)) if self.rules.receivers.iter().any(|r| r == field) => {
                Mode::Plain
            }
            // Arguments and the other parts of a call normalize as usual.
            (_, _) if callee.is_some() => Mode::Plain,
            (mode, _) => mode,
        }
    }
}
