// ABOUTME: Shared helpers for cupel-lang integration tests: produce a fact for one language over a
// ABOUTME: fixture repository (or a scratch tree) through the registries, as the scheduler will.
#![allow(dead_code)] // Each test crate uses a different subset of these helpers.

use std::path::{Path, PathBuf};

use cupel_core::{
    Cancellation, Ctx, Event, Exclude, Fact, ProduceCtx, ProducerRegistry, Scope, Writer,
    syntax::{
        ApiItemKind, ApiSurface, AssertionKind, AssertionStrength, DecisionKind, Decisions,
        EdgeKind, ImportTarget, ModuleGraph, NormNode, ProjectInfo, RootReason, Roots, Site, Span,
        SymbolDef, SymbolKind, Symbols, TestCase, TestFile, TestFrames, Visibility,
    },
};
use cupel_lang::{LanguageRegistry, walk_excluding};

/// The fixture repository for `lang` in cupel-fixtures.
pub fn fixture(lang: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cupel-fixtures/repos")
        .join(lang)
}

/// Produces `F` for language `lang` over every source file under `root`, panicking on any
/// event (a parse error in a fixture is a bug in the fixture).
pub fn produce<F: Fact>(lang: &str, root: &Path) -> F {
    let (fact, events) = produce_with_events::<F>(lang, root);
    assert!(events.is_empty(), "{events:?}");
    fact
}

pub fn produce_with_events<F: Fact>(lang: &str, root: &Path) -> (F, Vec<Event>) {
    produce_in::<F>(lang, root, &Exclude::default())
}

/// Produces `F` for `lang` over `root` with `exclude` applied, panicking on any event.
pub fn produce_excluding<F: Fact>(lang: &str, root: &Path, exclude: &Exclude) -> F {
    let (fact, events) = produce_in::<F>(lang, root, exclude);
    assert!(events.is_empty(), "{events:?}");
    fact
}

fn produce_in<F: Fact>(lang: &str, root: &Path, exclude: &Exclude) -> (F, Vec<Event>) {
    let languages = LanguageRegistry::builtin();
    let mut producers = ProducerRegistry::default();
    languages.register_producers(&mut producers);
    let language = languages.get(&lang.into()).expect("registered language");
    let producer = producers
        .producers_for(F::KIND, &cupel_lang::lang(language.as_ref()))
        .next()
        .unwrap_or_else(|| panic!("no {} producer for {lang}", F::KIND));
    let writer = Writer::new(root, false).unwrap().confined();
    let ctx = ProduceCtx {
        ctx: Ctx {
            root: writer.root().to_path_buf(),
            exclude: exclude.clone(),
        },
        config: serde_json::Map::new(),
        writer,
        cancel: Cancellation::new(),
    };
    let scope = Scope::of_files(walk_excluding(
        root,
        &languages,
        &ProjectInfo::default(),
        exclude,
    ));
    let mut events = Vec::new();
    let fact = producer
        .produce(&ctx, &scope, &mut events)
        .unwrap()
        .downcast::<F>()
        .expect("the producer makes its declared fact");
    (fact, events)
}

/// Writes `files` (path, contents) under a fresh temporary directory.
pub fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, contents) in files {
        let path = dir.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }
    dir
}

/// `(start line, end line)` of a span.
pub fn lines(span: Span) -> (u32, u32) {
    (span.line, span.end_line)
}

/// Whether each loop of the `lang` symbol `display` under `root` iterates over a table of cases
/// written in place, in document order.
pub fn loop_tables(lang: &str, root: &Path, display: &str) -> Vec<bool> {
    decisions_of_kind(lang, root, display, DecisionKind::Loop)
        .iter()
        .map(|d| d.table)
        .collect()
}

/// Whether each `case` of the `lang` symbol `display` under `root` continues a match or switch
/// an earlier case began, in document order.
pub fn case_continuations(lang: &str, root: &Path, display: &str) -> Vec<bool> {
    decisions_of_kind(lang, root, display, DecisionKind::Case)
        .iter()
        .map(|d| d.continues)
        .collect()
}

/// (kind, nesting, cyclomatic, cognitive) for each decision of the `lang` symbol `display`.
pub fn decisions_of(lang: &str, root: &Path, display: &str) -> Vec<(DecisionKind, u32, u32, u32)> {
    decisions_for(lang, root, display)
        .iter()
        .map(|d| (d.kind, d.nesting, d.cyclomatic, d.cognitive))
        .collect()
}

fn decisions_of_kind(
    lang: &str,
    root: &Path,
    display: &str,
    kind: DecisionKind,
) -> Vec<cupel_core::syntax::Decision> {
    decisions_for(lang, root, display)
        .into_iter()
        .filter(|d| d.kind == kind)
        .collect()
}

/// Every decision of the `lang` symbol `display` under `root`, in document order.
fn decisions_for(lang: &str, root: &Path, display: &str) -> Vec<cupel_core::syntax::Decision> {
    let symbols: Symbols = produce(lang, root);
    let decisions: Decisions = produce(lang, root);
    let key = symbols
        .symbols
        .iter()
        .find(|s| s.display == display)
        .unwrap_or_else(|| panic!("no symbol {display}"))
        .key();
    decisions
        .symbols
        .iter()
        .find(|entry| entry.symbol == key)
        .unwrap_or_else(|| panic!("no decisions entry for {display}"))
        .decisions
        .clone()
}

/// One expected symbol: its display name, kind, visibility, and first and last lines.
pub type SymbolRow = (&'static str, SymbolKind, Visibility, (u32, u32));

/// Asserts that `file`'s symbols are exactly `expected`, in order.
pub fn assert_symbols(symbols: &Symbols, file: &str, expected: &[SymbolRow]) {
    let actual: Vec<(String, SymbolKind, Visibility, (u32, u32))> = symbols
        .symbols
        .iter()
        .filter(|symbol| symbol.file == file)
        .map(|s| (s.display.clone(), s.kind, s.visibility, lines(s.span)))
        .collect();
    let expected: Vec<(String, SymbolKind, Visibility, (u32, u32))> = expected
        .iter()
        .map(|(d, k, v, l)| ((*d).to_owned(), *k, *v, *l))
        .collect();
    assert_eq!(actual, expected, "{file}");
}

/// An import edge as a test compares it: (root, from, line, specifier, target, relative), the
/// target a file path, `external:<package>`, or `unresolved`.
pub type EdgeRow = (String, String, u32, String, String, bool);

/// An expected edge within a root: (from, line, specifier, target, relative).
pub type Edge<'a> = (&'a str, u32, &'a str, &'a str, bool);

/// Every edge of `graph`, root by root.
pub fn edge_rows(graph: &ModuleGraph) -> Vec<EdgeRow> {
    graph
        .roots
        .iter()
        .flat_map(|root| {
            root.edges.iter().map(|edge| {
                (
                    root.root.clone(),
                    edge.from.clone(),
                    edge.line,
                    edge.specifier.clone(),
                    match &edge.target {
                        ImportTarget::File { path } => path.clone(),
                        ImportTarget::External { package } => format!("external:{package}"),
                        ImportTarget::Unresolved => "unresolved".to_owned(),
                    },
                    edge.relative,
                )
            })
        })
        .collect()
}

/// The expected `edge` in the root `root`.
pub fn row(root: &str, edge: Edge<'_>) -> EdgeRow {
    let (from, line, specifier, target, relative) = edge;
    (
        root.to_owned(),
        from.to_owned(),
        line,
        specifier.to_owned(),
        target.to_owned(),
        relative,
    )
}

/// The expected `edges`, all in the root `root`.
pub fn under(root: &str, edges: &[Edge<'_>]) -> Vec<EdgeRow> {
    edges.iter().map(|edge| row(root, *edge)).collect()
}

pub fn items(api: &ApiSurface, file: &str) -> Vec<(String, ApiItemKind, String)> {
    api.items
        .iter()
        .filter(|item| item.file == file)
        .map(|item| (item.path.clone(), item.kind, item.signature.clone()))
        .collect()
}

pub fn reasons(roots: &Roots, symbols: &Symbols, display: &str) -> Vec<RootReason> {
    let symbol: &SymbolDef = symbols
        .symbols
        .iter()
        .find(|s| s.display == display)
        .unwrap_or_else(|| panic!("no symbol {display}"));
    roots
        .roots
        .iter()
        .filter(|root| root.symbol == symbol.key())
        .map(|root| root.reason)
        .collect()
}

/// (from, specifier, kind) for every edge, in graph order.
pub fn kinds(graph: &ModuleGraph) -> Vec<(String, String, EdgeKind)> {
    graph
        .roots
        .iter()
        .flat_map(|root| &root.edges)
        .map(|edge| (edge.from.clone(), edge.specifier.clone(), edge.edge_kind))
        .collect()
}

pub fn kind(from: &str, specifier: &str, kind: EdgeKind) -> (String, String, EdgeKind) {
    (from.to_owned(), specifier.to_owned(), kind)
}

/// Whether a case is skipped, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Skip {
    Runs,
    Skipped,
    Because(String),
}

/// (name, assertions as (kind, strength), skip, mock targets, start line) per case.
pub type CaseRow = (
    String,
    Vec<(AssertionKind, AssertionStrength)>,
    Skip,
    Vec<String>,
    u32,
);

pub fn file<'a>(frames: &'a TestFrames, path: &str) -> &'a TestFile {
    frames
        .files
        .iter()
        .find(|f| f.file == path)
        .unwrap_or_else(|| panic!("no test file {path}"))
}

pub fn case_row(case: &TestCase) -> CaseRow {
    (
        case.name.clone(),
        case.assertions
            .iter()
            .map(|a| (a.kind, a.strength))
            .collect(),
        match case.skip.as_ref().map(|s| s.reason.clone()) {
            None => Skip::Runs,
            Some(None) => Skip::Skipped,
            Some(Some(reason)) => Skip::Because(reason),
        },
        case.mocks.iter().map(|m| m.target.clone()).collect(),
        case.span.line,
    )
}

pub fn case(
    name: &str,
    assertions: &[(AssertionKind, AssertionStrength)],
    skip: Skip,
    mocks: &[&str],
    line: u32,
) -> CaseRow {
    (
        name.to_owned(),
        assertions.to_vec(),
        skip,
        mocks.iter().map(|m| (*m).to_owned()).collect(),
        line,
    )
}

/// The byte range of a site from its 1-based line and byte columns.
pub fn byte_range(source: &str, site: &Site) -> (usize, usize) {
    let offset = |line: u32, col: u32| {
        let start: usize = source
            .split_inclusive('\n')
            .take(line as usize - 1)
            .map(str::len)
            .sum();
        start + col as usize - 1
    };
    (
        offset(site.span.line, site.span.col),
        offset(site.span.end_line, site.span.end_col),
    )
}

/// `(label child …)`, or `label` for a leaf.
pub fn sexp(node: &NormNode) -> String {
    if node.children.is_empty() {
        node.label.clone()
    } else {
        let children: Vec<String> = node.children.iter().map(sexp).collect();
        format!("({} {})", node.label, children.join(" "))
    }
}

pub fn rows(file: &TestFile) -> Vec<CaseRow> {
    file.cases.iter().map(case_row).collect()
}
