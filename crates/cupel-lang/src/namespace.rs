// ABOUTME: Namespaces: a file's module path (from `[namespace]` rules or `@namespace.name`) plus the
// ABOUTME: names of the scopes (`@scope` + `@scope.name`) and symbols that lexically enclose a node.
use tree_sitter::Node;

use crate::contract::capture;
use crate::parse::Parsed;
use crate::query::{Match, encloses};
use crate::tables::NamespaceRules;

/// A named node whose name becomes a namespace segment for everything inside it.
#[derive(Debug, Clone)]
pub struct Scope<'t> {
    pub node: Node<'t>,
    /// One segment, or several already joined by the separator.
    pub name: String,
}

/// The namespace context of one file.
pub struct Namespaces<'t> {
    separator: String,
    file: Vec<String>,
    scopes: Vec<Scope<'t>>,
}

impl<'t> Namespaces<'t> {
    /// Collects the file namespace and every `@scope` in `matches` (from symbols.scm or api.scm).
    pub fn new(rules: &NamespaceRules, parsed: &'t Parsed, matches: &[Match<'_, 't>]) -> Self {
        let declared = matches
            .iter()
            .find_map(|m| m.node(capture::NAMESPACE_NAME))
            .map(|node| vec![parsed.text(node).to_owned()]);
        let mut scopes: Vec<Scope<'t>> = matches
            .iter()
            .filter_map(|m| {
                Some(Scope {
                    node: m.node(capture::SCOPE)?,
                    name: parsed.text(m.node(capture::SCOPE_NAME)?).to_owned(),
                })
            })
            .collect();
        scopes.sort_by_key(|scope| {
            (
                scope.node.start_byte(),
                std::cmp::Reverse(scope.node.end_byte()),
            )
        });
        scopes.dedup_by_key(|scope| scope.node.id());
        Self {
            separator: rules.separator.clone(),
            file: declared.unwrap_or_else(|| file_namespace(rules, &parsed.file)),
            scopes,
        }
    }

    /// The namespace of `node`: the file's segments, then every scope and every symbol in
    /// `enclosing` that strictly contains `node`, outermost first, then `extra` (a scope name
    /// captured in the node's own match, such as a Go method's receiver type).
    pub fn of(&self, node: Node<'_>, enclosing: &[Scope<'_>], extra: Option<&str>) -> String {
        let mut around: Vec<(usize, usize, &str)> = self
            .scopes
            .iter()
            .chain(enclosing)
            .map(|scope| (scope.node, scope.name.as_str()))
            .filter(|(outer, _)| encloses(*outer, node))
            .map(|(outer, name)| (outer.start_byte(), usize::MAX - outer.end_byte(), name))
            .collect();
        around.sort_unstable();
        let segments = self
            .file
            .iter()
            .map(String::as_str)
            .chain(around.into_iter().map(|(_, _, name)| name))
            .chain(extra)
            .filter(|segment| !segment.is_empty());
        segments.collect::<Vec<_>>().join(&self.separator)
    }

    /// `first` and `second` joined by the separator, or whichever is not empty.
    pub fn join(&self, first: &str, second: &str) -> String {
        match (first.is_empty(), second.is_empty()) {
            (true, _) => second.to_owned(),
            (_, true) => first.to_owned(),
            _ => format!("{first}{}{second}", self.separator),
        }
    }

    /// `namespace` and `name` joined, or `name` alone at the top level.
    pub fn display(&self, namespace: &str, name: &str) -> String {
        if namespace.is_empty() {
            name.to_owned()
        } else {
            format!("{namespace}{}{name}", self.separator)
        }
    }
}

/// The namespace segments `[namespace]` derives from a file's path.
pub fn file_namespace(rules: &NamespaceRules, file: &str) -> Vec<String> {
    let mut parts: Vec<&str> = file.split('/').collect();
    let Some(last) = parts.pop() else {
        return Vec::new();
    };
    if let Some(root) = parts
        .iter()
        .rposition(|dir| rules.source_roots.iter().any(|r| r == dir))
    {
        parts.drain(..=root);
    }
    let stem = last.split_once('.').map_or(last, |(stem, _)| stem);
    if !rules.index_files.iter().any(|index| index == stem) {
        parts.push(stem);
    }
    parts.into_iter().map(str::to_owned).collect()
}

/// `path` split at its last source root directory: the part before it (with a trailing `/`, or
/// empty when no source root is on the path) and the part below it without the extension.
pub fn file_namespace_root(path: &str, source_roots: &[String]) -> (String, String) {
    let parts: Vec<&str> = path.split('/').collect();
    let root = parts[..parts.len().saturating_sub(1)]
        .iter()
        .rposition(|dir| source_roots.iter().any(|r| r == dir));
    let without_ext = |text: &str| {
        text.rsplit_once('.')
            .map_or(text, |(stem, _)| stem)
            .to_owned()
    };
    match root {
        Some(index) => (
            parts[..index].iter().fold(String::new(), |mut base, dir| {
                base.push_str(dir);
                base.push('/');
                base
            }),
            without_ext(&parts[index + 1..].join("/")),
        ),
        None => (String::new(), without_ext(path)),
    }
}
