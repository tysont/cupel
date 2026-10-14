// ABOUTME: Shared helpers for cupel-lang integration tests: produce a fact for one language over a
// ABOUTME: fixture or scratch tree as the scheduler will, and read decisions, sites, norm trees, and frames.
#![allow(dead_code)] // Each test crate uses a different subset of these helpers.

use std::path::{Path, PathBuf};

use cupel_core::{
    Cancellation, Ctx, Event, Exclude, Fact, ProduceCtx, ProducerRegistry, Scope, Writer,
    syntax::{
        ApiItemKind, ApiSurface, AssertionKind, AssertionStrength, DecisionKind, Decisions,
        EdgeKind, ImportTarget, ModuleGraph, MutationOperator, MutationSites, NormNode,
        OperatorSet, ProjectInfo, RootReason, Roots, Site, Span, SymbolDef, SymbolKey, SymbolKind,
        Symbols, TestCase, TestFile, TestFrames, Visibility,
    },
};
use cupel_lang::{LanguageRegistry, walk_excluding};

/// The fixture repository for `lang` in cupel-fixtures.
pub fn fixture(lang: &str) -> PathBuf {
    cupel_fixtures::repo_path(lang)
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
    let ctx = ProduceCtx::new(
        Ctx {
            exclude: exclude.clone(),
            ..Ctx::new(writer.root().to_path_buf())
        },
        writer,
        Cancellation::new(),
    );
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
    flags_of_kind(lang, root, display, DecisionKind::Loop, |d| d.table)
}

/// Whether each `case` of the `lang` symbol `display` under `root` continues a match or switch
/// an earlier case began, in document order.
pub fn case_continuations(lang: &str, root: &Path, display: &str) -> Vec<bool> {
    flags_of_kind(lang, root, display, DecisionKind::Case, |d| d.continues)
}

/// (kind, nesting, cyclomatic, cognitive) for each decision of the `lang` symbol `display`.
pub fn decisions_of(lang: &str, root: &Path, display: &str) -> Vec<(DecisionKind, u32, u32, u32)> {
    decisions_for(lang, root, display)
        .iter()
        .map(|d| (d.kind, d.nesting, d.cyclomatic, d.cognitive))
        .collect()
}

/// `flag` of each `kind` decision of the `lang` symbol `display` under `root`, in document order.
fn flags_of_kind(
    lang: &str,
    root: &Path,
    display: &str,
    kind: DecisionKind,
    flag: fn(&cupel_core::syntax::Decision) -> bool,
) -> Vec<bool> {
    decisions_of_kind(lang, root, display, kind)
        .iter()
        .map(flag)
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

/// Asserts that every test case of `lang`'s fixture repository is a `test` root, and that there
/// are `cases` of them.
pub fn assert_every_case_is_a_test_root(lang: &str, cases: usize) {
    let root = fixture(lang);
    let roots: Roots = produce(lang, &root);
    let symbols: Symbols = produce(lang, &root);
    let frames: TestFrames = produce(lang, &root);
    let test_roots: Vec<SymbolKey> = roots
        .roots
        .iter()
        .filter(|r| r.reason == RootReason::Test)
        .map(|r| r.symbol.clone())
        .collect();
    let all: Vec<(&str, bool)> = frames
        .files
        .iter()
        .flat_map(|file| file.cases.iter().map(move |case| (file, case)))
        .map(|(file, case)| {
            let rooted = symbols
                .symbols
                .iter()
                .find(|s| s.file == file.file && s.span == case.span)
                .is_some_and(|symbol| test_roots.contains(&symbol.key()));
            (case.name.as_str(), rooted)
        })
        .collect();
    let unrooted: Vec<&str> = all
        .iter()
        .filter(|(_, rooted)| !rooted)
        .map(|(name, _)| *name)
        .collect();
    assert_eq!(
        unrooted,
        Vec::<&str>::new(),
        "{lang}: test cases that are not test roots"
    );
    assert_eq!(all.len(), cases, "{lang}");
}

/// Asserts that `hashes` are pairwise distinct.
pub fn assert_distinct(hashes: &[String]) {
    let distinct: std::collections::BTreeSet<&String> = hashes.iter().collect();
    assert_eq!(distinct.len(), hashes.len(), "{hashes:?}");
}

/// The test file at `path` in the `lang` fixture repository, after asserting its framework and
/// the lines of its setup blocks.
pub fn assert_test_file(lang: &str, path: &str, framework: &str, setup: &[u32]) -> TestFile {
    let found = assert_framework(&produce(lang, &fixture(lang)), path, framework);
    let lines: Vec<u32> = found.setup.iter().map(|span| span.line).collect();
    assert_eq!(lines, setup, "{path}");
    found
}

/// The test file at `path` in `frames`, after asserting its framework.
pub fn assert_framework(frames: &TestFrames, path: &str, framework: &str) -> TestFile {
    let found = file(frames, path).clone();
    assert_eq!(found.framework, framework, "{path}");
    found
}

/// An expected test case: (name, assertions, skip, mock targets, start line), the skip `None` when
/// the case runs, `Some(None)` when it is skipped without a reason, and `Some(Some(reason))`.
pub type Expected = (
    &'static str,
    &'static [(AssertionKind, AssertionStrength)],
    Option<Option<&'static str>>,
    &'static [&'static str],
    u32,
);

/// `cases` as rows to compare with `rows`.
pub fn expected(cases: &[Expected]) -> Vec<CaseRow> {
    cases
        .iter()
        .map(|(name, assertions, skip, mocks, line)| {
            let skip = match skip {
                None => Skip::Runs,
                Some(None) => Skip::Skipped,
                Some(Some(reason)) => Skip::Because((*reason).to_owned()),
            };
            case(name, assertions, skip, mocks, *line)
        })
        .collect()
}

/// The test file of a repository holding only `source` at `path`, after asserting its framework.
pub fn assert_source_file(lang: &str, path: &str, source: &str, framework: &str) -> TestFile {
    let dir = tree(&[(path, source)]);
    assert_framework(&produce(lang, dir.path()), path, framework)
}

/// The mutation sites of the `lang` symbol `display` under `root`, in document order.
pub fn sites_of(lang: &str, root: &Path, display: &str) -> Vec<Site> {
    let symbols: Symbols = produce(lang, root);
    let sites: MutationSites = produce(lang, root);
    let key = symbols
        .symbols
        .iter()
        .find(|s| s.display == display)
        .unwrap_or_else(|| panic!("no symbol {display}"))
        .key();
    sites
        .symbols
        .into_iter()
        .find(|entry| entry.symbol == key)
        .unwrap_or_else(|| panic!("no sites entry for {display}"))
        .sites
}

/// One mutation site as a test compares it: (operator, set, from, to, line).
pub type SiteRow = (MutationOperator, OperatorSet, String, Vec<String>, u32);

/// (operator, set, from, to, line) per site.
pub fn summary(sites: &[Site]) -> Vec<SiteRow> {
    sites
        .iter()
        .map(|s| (s.operator, s.set, s.from.clone(), s.to.clone(), s.span.line))
        .collect()
}

/// The expected site: its set follows from the operator (the default set or the extended one).
pub fn site_row(operator: MutationOperator, from: &str, to: &[&str], line: u32) -> SiteRow {
    use MutationOperator::{
        ArithAddSub, ArithMulDiv, BooleanLiteral, BooleanOp, Equality, Relational, ZeroOne,
    };
    let set = match operator {
        Relational | Equality | ArithAddSub | ArithMulDiv | BooleanOp | BooleanLiteral
        | ZeroOne => OperatorSet::Default,
        _ => OperatorSet::Extended,
    };
    (
        operator,
        set,
        from.to_owned(),
        to.iter().map(|t| (*t).to_owned()).collect(),
        line,
    )
}

/// Each `lang` symbol under `root` that has a sites entry, by name, with its site hashes.
pub fn site_hashes(lang: &str, root: &Path) -> Vec<(String, Vec<String>)> {
    let symbols: Symbols = produce(lang, root);
    let sites: MutationSites = produce(lang, root);
    sites
        .symbols
        .iter()
        .map(|entry| {
            let name = symbols
                .symbols
                .iter()
                .find(|s| s.key() == entry.symbol)
                .unwrap()
                .name
                .clone();
            (
                name,
                entry
                    .sites
                    .iter()
                    .map(|s| s.site_hash.to_string())
                    .collect(),
            )
        })
        .collect()
}

/// Every replacement of every site in `root`'s `lang` files leaves a file that parses cleanly
/// with the grammar `grammar` picks for the file; returns how many file extensions were
/// exercised.
pub fn assert_every_replacement_parses(
    lang: &str,
    root: &Path,
    grammar: fn(&str) -> tree_sitter::Language,
) -> usize {
    let sites: MutationSites = produce(lang, root);
    let symbols: Symbols = produce(lang, root);
    let mut parser = tree_sitter::Parser::new();
    let mut checked = 0;
    let mut extensions = std::collections::BTreeSet::new();
    for entry in &sites.symbols {
        let file = &symbols
            .symbols
            .iter()
            .find(|s| s.key() == entry.symbol)
            .unwrap()
            .file;
        parser.set_language(&grammar(file)).unwrap();
        let source = std::fs::read_to_string(root.join(file)).unwrap();
        for site in &entry.sites {
            extensions.insert(Path::new(file).extension().unwrap().to_owned());
            let (start, end) = byte_range(&source, site);
            assert_eq!(&source[start..end], site.from, "{file}");
            for to in &site.to {
                let mutated = format!("{}{to}{}", &source[..start], &source[end..]);
                let tree = parser.parse(&mutated, None).unwrap();
                assert!(
                    !tree.root_node().has_error(),
                    "{file}:{}: {} -> {to} does not parse",
                    site.span.line,
                    site.from
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 0);
    extensions.len()
}

/// The `lang` fixture's `file`, reformatted: each leading `unit` of indentation becomes `indent`
/// and `blank` follows every line break, so every line keeps its depth.
pub fn relaid(lang: &str, file: &str, unit: &str, indent: &str, blank: &str) -> String {
    std::fs::read_to_string(fixture(lang).join(file))
        .unwrap()
        .lines()
        .map(|line| {
            let mut rest = line;
            let mut depth = 0;
            while let Some(inner) = rest.strip_prefix(unit) {
                rest = inner;
                depth += 1;
            }
            format!("{}{}", indent.repeat(depth), line.trim_start())
        })
        .collect::<Vec<_>>()
        .join(&format!("\n{blank}"))
}

/// The normalized shape (as an s-expression) and structural hash of the one `lang` symbol in a
/// repository holding only `source` at `path`.
pub fn normalize_at(lang: &str, path: &str, source: &str) -> (String, String) {
    let dir = tree(&[(path, source)]);
    let norm: cupel_core::syntax::NormTree = produce(lang, dir.path());
    let symbols: Symbols = produce(lang, dir.path());
    assert_eq!(norm.symbols.len(), 1, "{source}");
    assert_eq!(norm.symbols[0].symbol, symbols.symbols[0].key());
    (
        sexp(&norm.symbols[0].root),
        norm.symbols[0].hash.to_string(),
    )
}
