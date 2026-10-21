// ABOUTME: The `References` producer: the names, members, strings, and attributes that may name a symbol, from
// ABOUTME: references.scm over every file of the language, resolved in their file or through its imports (ADR-135).
//!
//! # `references.scm`
//!
//! Each capture is one reference or one dynamic access; a node captured by several patterns
//! counts once, as the earliest pattern's capture.
//!
//! - `@reference.name` on a plain identifier: resolved to the same file's functions and closures
//!   of that name, else through an import of the file that binds the name, else unresolved. An
//!   import binds the names its statement lists (`@import.name` in imports.scm), or, where
//!   `[references] path_imports` is set, its specifier's last segment when it lists none; the
//!   name must be its edge's specifier's last segment, and the edge must lead to a file.
//! - `@reference.member` on a name reached through a value, type, module, or namespace
//!   (`x.f`, `T::f`, `m/f`): unresolved, so it matches every symbol of that name.
//! - `@reference.text` on a string literal's text: when it is a name or a path of names
//!   (`Option::is_none`, `ledger.rates`), a reference to its last segment, unresolved.
//! - `@reference.attribute` on an attribute, decorator, or annotation, with `@attribute.name`
//!   on its name: a reference to the definition it belongs to (the definition whose signature
//!   holds it, else the definition that follows it among its siblings), unless `[references]
//!   inert` lists the name.
//! - `@dynamic.members` on a computed member access, `@dynamic.names` on a lookup by a
//!   run-time name or code evaluated from text.
//!
//! A definition's own name is never a reference. A method whose name `[references] implicit`
//! lists gets an `implicit` reference to itself.
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use cupel_core::syntax::{
    DynamicAccess, DynamicReach, EdgeKind, ImportTarget, ModuleGraph, Reference, ReferenceKind,
    ReferenceTarget, References, SymbolKey, SymbolKind,
};
use cupel_core::{
    Availability, CostClass, Ctx, EventSink, FactKind, Langs, ProduceCtx, Producer, ProducerId,
    ProducerRegistry, Result, Scope,
};
use rayon::prelude::*;
use tree_sitter::Node;

use crate::contract::capture;
use crate::imports::{ImportGraph, Imports, Resolve, file_imports, unquote};
use crate::language::{Language, owns, parse_one, producer_id};
use crate::parse::{ParseCache, Parsed};
use crate::producer::parse_scope;
use crate::query::{matches, span};
use crate::spec::QueryFile;
use crate::symbols::{Definition, definitions};
use crate::walk::language_files;

/// The characters that separate the segments of a path written in a string or a specifier.
const SEPARATORS: &[char] = &['/', '.', ':', '\\', '#'];

/// The longest dynamic access text kept.
const ACCESS_TEXT: usize = 80;

/// Registers `language`'s `ModuleGraph` and `References` producers, which both call its
/// hand-written module resolution.
pub fn register_resolved(
    language: Arc<dyn Language>,
    cache: &Arc<ParseCache>,
    resolve: Resolve,
    reg: &mut ProducerRegistry,
) {
    reg.register::<ModuleGraph, _>(ImportGraph::new(
        Arc::clone(&language),
        Arc::clone(cache),
        resolve,
    ));
    reg.register::<References, _>(ReferenceGraph::new(language, Arc::clone(cache), resolve));
}

/// The `References` producer of one language.
pub struct ReferenceGraph {
    language: Arc<dyn Language>,
    cache: Arc<ParseCache>,
    resolve: Resolve,
}

impl ReferenceGraph {
    pub fn new(language: Arc<dyn Language>, cache: Arc<ParseCache>, resolve: Resolve) -> Self {
        Self {
            language,
            cache,
            resolve,
        }
    }
}

impl Producer<References> for ReferenceGraph {
    fn id(&self) -> ProducerId {
        producer_id(self.language.as_ref())
    }

    fn languages(&self) -> Langs {
        Langs::Only(vec![self.language.id()])
    }

    fn needs(&self) -> &[FactKind] {
        &[]
    }

    fn cost(&self) -> CostClass {
        CostClass::Quick
    }

    fn availability(&self, _: &Ctx) -> Availability {
        Availability::Ready
    }

    fn produce(
        &self,
        ctx: &ProduceCtx,
        scope: &Scope,
        sink: &mut dyn EventSink,
    ) -> Result<References> {
        let language = self.language.as_ref();
        parse_scope(&self.cache, ctx, scope, language, sink)?;
        let root = &ctx.ctx.root;
        let mut files = language_files(root, language, &ctx.ctx.exclude);
        files.extend(
            scope
                .files
                .iter()
                .filter(|file| owns(language, file))
                .cloned(),
        );
        files.sort();
        files.dedup();
        ctx.cancel.checkpoint()?;
        let extractor = extract as fn(&Parsed, &dyn Language) -> FileReferences as usize;
        let parts: Vec<FileReferences> = files
            .par_iter()
            .filter_map(|file| parse_one(&self.cache, root, file, language, &ctx.cancel).ok())
            .map(|parsed| parsed.derived(extractor, || extract(&parsed, language)))
            .collect();
        ctx.cancel.checkpoint()?;
        let graph = (self.resolve)(&Imports::new(root, language, &self.cache, files));
        ctx.cancel.checkpoint()?;
        Ok(resolve(parts, &graph))
    }
}

/// One file's references before imports are resolved.
#[derive(Debug, Clone)]
struct FileReferences {
    file: String,
    /// The file's functions and closures, which a plain name may call, by name.
    callable: BTreeMap<String, Vec<SymbolKey>>,
    /// The names each import statement lists, by the statement's line.
    bound: BTreeSet<(u32, String)>,
    /// The lines of import statements that list no names but bind their paths' last segments.
    paths: BTreeSet<u32>,
    references: Vec<Reference>,
    dynamic: Vec<DynamicAccess>,
}

/// Every file's references, each plain name its file does not define resolved through the
/// file's imports.
fn resolve(parts: Vec<FileReferences>, graph: &ModuleGraph) -> References {
    let callable: HashMap<&str, &BTreeMap<String, Vec<SymbolKey>>> = parts
        .iter()
        .map(|part| (part.file.as_str(), &part.callable))
        .collect();
    let statements: HashMap<&str, &FileReferences> = parts
        .iter()
        .map(|part| (part.file.as_str(), part))
        .collect();
    let mut bindings: HashMap<&str, Vec<(&str, &str)>> = HashMap::new();
    for edge in graph.roots.iter().flat_map(|root| &root.edges) {
        let ImportTarget::File { path } = &edge.target else {
            continue;
        };
        let name = edge.specifier.rsplit(SEPARATORS).next().unwrap_or_default();
        let binds = edge.edge_kind != EdgeKind::Declaration
            && statements.get(edge.from.as_str()).is_some_and(|part| {
                part.paths.contains(&edge.line)
                    || part.bound.contains(&(edge.line, name.to_owned()))
            });
        if binds {
            bindings
                .entry(edge.from.as_str())
                .or_default()
                .push((name, path.as_str()));
        }
    }
    let imported = |file: &str, name: &str| -> Option<ReferenceTarget> {
        bindings.get(file)?.iter().find_map(|(bound, target)| {
            let symbols = callable.get(target)?.get(name)?;
            (*bound == name).then(|| ReferenceTarget::Imported {
                file: (*target).to_owned(),
                symbols: symbols.clone(),
            })
        })
    };
    let mut fact = References::default();
    for part in &parts {
        for reference in &part.references {
            let mut reference = reference.clone();
            if reference.kind == ReferenceKind::Name
                && reference.target == ReferenceTarget::Unresolved
                && let Some(target) = imported(&part.file, &reference.name)
            {
                reference.target = target;
            }
            fact.references.push(reference);
        }
        fact.dynamic.extend(part.dynamic.iter().cloned());
    }
    fact
}

/// What one capture of `references.scm` is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Captured {
    Reference(ReferenceKind),
    Dynamic(DynamicReach),
}

fn captured(name: &str) -> Option<Captured> {
    Some(match name {
        capture::REFERENCE_NAME => Captured::Reference(ReferenceKind::Name),
        capture::REFERENCE_MEMBER => Captured::Reference(ReferenceKind::Member),
        capture::REFERENCE_TEXT => Captured::Reference(ReferenceKind::Text),
        capture::REFERENCE_ATTRIBUTE => Captured::Reference(ReferenceKind::Attribute),
        capture::DYNAMIC_MEMBERS => Captured::Dynamic(DynamicReach::Members),
        capture::DYNAMIC_NAMES => Captured::Dynamic(DynamicReach::Names),
        _ => return None,
    })
}

/// One file's references, resolved within the file.
///
/// # Panics
///
/// When the language's `references.scm` does not compile; a test compiles every query.
fn extract(parsed: &Parsed, language: &dyn Language) -> FileReferences {
    let spec = language.spec();
    let defined = definitions(parsed, language);
    let mut callable: BTreeMap<String, Vec<SymbolKey>> = BTreeMap::new();
    for definition in &defined {
        if definition.symbol.kind != SymbolKind::Method {
            callable
                .entry(definition.symbol.name.clone())
                .or_default()
                .push(definition.symbol.key());
        }
    }
    let rules = &spec.tables().references;
    let mut bound = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for import in file_imports(parsed, language).imports {
        if import.names.is_empty() {
            if rules.path_imports {
                paths.insert(import.line);
            }
        } else {
            bound.extend(import.names.into_iter().map(|name| (import.line, name)));
        }
    }
    let mut part = FileReferences {
        file: parsed.file.clone(),
        callable,
        bound,
        paths,
        references: Vec::new(),
        dynamic: Vec::new(),
    };
    for definition in &defined {
        let symbol = &definition.symbol;
        if symbol.kind == SymbolKind::Method && rules.implicit.contains(&symbol.name) {
            part.references.push(Reference {
                file: parsed.file.clone(),
                span: span(definition.name),
                name: symbol.name.clone(),
                kind: ReferenceKind::Implicit,
                from: None,
                target: ReferenceTarget::Local {
                    symbols: vec![symbol.key()],
                },
            });
        }
    }
    if !spec.has(QueryFile::References) {
        return part;
    }
    let query = spec
        .query(QueryFile::References, &parsed.grammar)
        .unwrap_or_else(|error| panic!("{} references.scm: {error}", language.id()));
    // The earliest pattern's capture of each node, with its match's attribute name.
    let mut chosen: BTreeMap<(usize, usize), (usize, Captured, Node<'_>, Option<Node<'_>>)> =
        BTreeMap::new();
    for m in matches(&query, parsed) {
        let attribute_name = m.node(capture::ATTRIBUTE_NAME);
        for (name, node) in &m.captures {
            let Some(kind) = captured(name) else {
                continue;
            };
            let dynamic = matches!(kind, Captured::Dynamic(_));
            let key = (node.start_byte(), node.id() * 2 + usize::from(dynamic));
            let entry = chosen
                .entry(key)
                .or_insert((m.pattern, kind, *node, attribute_name));
            if m.pattern < entry.0 {
                *entry = (m.pattern, kind, *node, attribute_name);
            }
        }
    }
    let names: Vec<(usize, usize)> = defined
        .iter()
        .map(|definition| definition.name.byte_range())
        .map(|range| (range.start, range.end))
        .collect();
    let within_name = |node: Node<'_>| {
        names
            .iter()
            .any(|(start, end)| *start <= node.start_byte() && node.end_byte() <= *end)
    };
    for (_, kind, node, attribute_name) in chosen.into_values() {
        let from = innermost(&defined, node).map(|definition| definition.symbol.key());
        match kind {
            Captured::Dynamic(reach) => part.dynamic.push(DynamicAccess {
                file: parsed.file.clone(),
                span: span(node),
                from,
                reach,
                text: access_text(parsed.text(node)),
            }),
            Captured::Reference(ReferenceKind::Attribute) => {
                let Some(name) = attribute_name.map(|n| last_segment(parsed.text(n))) else {
                    continue;
                };
                if rules.inert.iter().any(|inert| inert == name) {
                    continue;
                }
                let Some(owner) = attributed(&defined, node) else {
                    continue;
                };
                part.references.push(Reference {
                    file: parsed.file.clone(),
                    span: span(node),
                    name: name.to_owned(),
                    kind: ReferenceKind::Attribute,
                    from,
                    target: ReferenceTarget::Local {
                        symbols: vec![owner.symbol.key()],
                    },
                });
            }
            Captured::Reference(kind) => {
                if within_name(node) {
                    continue;
                }
                let text = parsed.text(node);
                let name = if kind == ReferenceKind::Text {
                    match path_name(unquote(text)) {
                        Some(name) => name,
                        None => continue,
                    }
                } else {
                    text
                };
                let target = match (kind, part.callable.get(name)) {
                    (ReferenceKind::Name, Some(symbols)) => ReferenceTarget::Local {
                        symbols: symbols.clone(),
                    },
                    _ => ReferenceTarget::Unresolved,
                };
                part.references.push(Reference {
                    file: parsed.file.clone(),
                    span: span(node),
                    name: name.to_owned(),
                    kind,
                    from,
                    target,
                });
            }
        }
    }
    part
}

/// The innermost definition whose node contains `node`.
fn innermost<'d, 't>(defined: &'d [Definition<'t>], node: Node<'_>) -> Option<&'d Definition<'t>> {
    defined
        .iter()
        .filter(|definition| contains(definition.node, node))
        .min_by_key(|definition| definition.node.end_byte() - definition.node.start_byte())
}

/// The definition an attribute belongs to: the innermost one holding it outside its body, else
/// the one whose node follows it among its siblings, past other attributes and comments.
fn attributed<'d, 't>(
    defined: &'d [Definition<'t>],
    attribute: Node<'_>,
) -> Option<&'d Definition<'t>> {
    let holding = defined
        .iter()
        .filter(|definition| {
            contains(definition.node, attribute)
                && definition
                    .body
                    .is_none_or(|body| !contains(body, attribute))
        })
        .min_by_key(|definition| definition.node.end_byte() - definition.node.start_byte());
    if holding.is_some() {
        return holding;
    }
    let mut next = attribute.next_named_sibling();
    while let Some(sibling) = next {
        if sibling.kind() != attribute.kind() && !sibling.kind().contains("comment") {
            return defined
                .iter()
                .find(|definition| definition.node.byte_range() == sibling.byte_range());
        }
        next = sibling.next_named_sibling();
    }
    None
}

fn contains(outer: Node<'_>, inner: Node<'_>) -> bool {
    outer.start_byte() <= inner.start_byte() && inner.end_byte() <= outer.end_byte()
}

/// The last segment of a path such as `functools.cache` or `tokio::main`.
fn last_segment(text: &str) -> &str {
    text.rsplit(SEPARATORS)
        .find(|segment| !segment.is_empty())
        .unwrap_or(text)
}

/// The last segment of `text` when the whole string is a name or a path of names.
fn path_name(text: &str) -> Option<&str> {
    let segments: Vec<&str> = text.split(SEPARATORS).collect();
    let named = |segment: &&str| {
        segment
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
            && segment
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '$' | '-' | '?' | '!' | '*'))
    };
    let last = *segments.last()?;
    (!text.is_empty()
        && text.len() <= 200
        && named(&last)
        && segments
            .iter()
            .all(|segment| segment.is_empty() || named(segment)))
    .then_some(last)
}

/// The accessor's first line, at most `ACCESS_TEXT` characters.
fn access_text(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or_default()
        .chars()
        .take(ACCESS_TEXT)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{last_segment, path_name};

    #[test]
    fn a_string_names_a_symbol_only_when_it_is_a_name_or_a_path_of_names() {
        assert_eq!(path_name("default_port"), Some("default_port"));
        assert_eq!(path_name("Option::is_none"), Some("is_none"));
        assert_eq!(path_name("ledger.rates"), Some("rates"));
        assert_eq!(
            path_name("ledger.references/dynamic-rate"),
            Some("dynamic-rate")
        );
        assert_eq!(path_name("Return the total."), None);
        assert_eq!(path_name("1st"), None);
        assert_eq!(path_name(""), None);
        assert_eq!(path_name("a b"), None);
    }

    #[test]
    fn an_attribute_is_named_by_its_last_segment() {
        assert_eq!(last_segment("functools.cache"), "cache");
        assert_eq!(last_segment("tokio::main"), "main");
        assert_eq!(last_segment("Override"), "Override");
    }
}
