// ABOUTME: `QueryTestFrames`: test cases from tests.scm with their assertions (kind and strength from
// ABOUTME: `[tests.assertions]`), mocks, skips, the file's setup blocks, and its framework.
//!
//! # `tests.scm`
//!
//! - `@test.case` on a test case's node and `@test.name` on the node whose text names it. A
//!   case node may be matched by several patterns (one per attribute or decorator it carries);
//!   their captures all belong to it, and its name comes from the earliest pattern.
//! - `@test.assertion` on an assertion, with either `@assertion.idiom.<key>` (any node) or
//!   `@assertion.name` (a node whose text, whitespace removed, is the key). The key is looked
//!   up in `[tests.assertions]`; unknown keys get `[tests].unknown_assertion`. When several
//!   patterns match one assertion node, the earliest wins, so idioms come first.
//! - `@assertion.helper` on the name a call is made by: when that name is a function of the
//!   same file that is not a case and holds an assertion (a helper such as
//!   `test_encode_ok(&cases)`), the call asserts with the class of the helper's first strong
//!   assertion, else its first: an assertion a pattern already captured at the call takes that
//!   class, else one is added. When the file defines no such function, a call that is already
//!   an assertion takes the class of an asserting function of that name in another file of the
//!   same directory in scope (a Go package's shared `_test.go` helpers). One level: a helper's
//!   own calls of helpers are not followed.
//! - `@assertion.macro` on the name of a macro the file defines (the definition is the name's
//!   parent node) and `@assertion.expansion` on the name a macro is invoked by: when the
//!   invoked macro is one the file defines outside every case and its definition holds an
//!   assertion (serde_json's `test_parse_err!`), the invocation asserts with the class of the
//!   definition's first strong assertion, else its first, as a helper call does. An invocation
//!   in a helper is one of the helper's assertions, so a case calling that helper asserts too.
//! - `@test.mock` on a test double's creation, with optional `@mock.target` naming what it
//!   replaces (else the mock node's text).
//! - `@test.skip` on a skip marker, with optional `@skip.reason` (the reason's text, without
//!   quotes).
//! - `@test.setup` on a setup or fixture block; reported per file.
//! - `@framework.<name>` on any node: the file's framework is the name from the earliest
//!   pattern that matched, else `[tests].framework`.
//! - `@property.<framework>` on any node: the case is a property test whose inputs `<framework>`
//!   generates (ADR-095); the earliest pattern wins per case.
//! - `@test.doc` on a comment or docstring, with `@doc.target` on the node it documents
//!   (ADR-134): when that node is a case, the doc is the case's. A doc pattern may match any
//!   function; it documents only cases other patterns found. A case's doc is its doc nodes' texts
//!   in document order, one per line.
//!
//! An assertion, mock, skip, or property captured in the same match as a `@test.case` belongs to that
//! case (attributes and decorators sit outside the function they mark); otherwise it belongs
//! to the innermost case containing it. A mock outside every case (a module-level mock)
//! belongs to every case in the file. Only files with at least one case are reported.
use std::collections::{BTreeMap, HashSet};

use cupel_core::FactKind;
use cupel_core::syntax::{
    Assertion, AssertionKind, AssertionStrength, Mock, SkipMarker, Span, TestCase, TestFile,
    TestFrames,
};
use tree_sitter::Node;

use crate::contract::capture;
use crate::language::Language;
use crate::parse::Parsed;
use crate::producer::{QueryProducer, Source};
use crate::query::{Match, matches, span};
use crate::spec::QueryFile;
use crate::symbols::definitions;
use crate::tables::TestRules;

/// Everything gathered for one case node.
struct Case<'t> {
    node: Node<'t>,
    name: Option<(usize, String)>,
    assertions: Vec<Assertion>,
    mocks: Vec<Mock>,
    skip: Option<SkipMarker>,
    /// The property framework, with the pattern that named it.
    property: Option<(usize, String)>,
    /// The doc comment or docstring nodes, by node ID.
    docs: BTreeMap<usize, Node<'t>>,
}

/// Cases by node ID.
type Cases<'t> = BTreeMap<usize, Case<'t>>;

inventory::submit!(crate::generic::GenericProducer {
    kind: FactKind::TestFrames,
    drives: |spec| spec.has(QueryFile::Tests),
    attach: |language, cache, reg| {
        reg.register::<TestFrames, _>(
            QueryProducer::new(language, cache, extract).finishing(resolve_helpers),
        );
    },
});

/// The `TestFrames` extractor.
///
/// # Panics
///
/// When the language's `tests.scm` does not compile; a test compiles every query.
pub fn extract(source: &Source<'_>) -> TestFrames {
    let parsed = source.parsed;
    let spec = source.language.spec();
    let rules = &spec.tables().tests;
    let query = spec
        .query(QueryFile::Tests, &parsed.grammar)
        .unwrap_or_else(|error| panic!("{} tests.scm: {error}", source.language.id()));
    let found = matches(&query, parsed);
    let mut cases = collect_cases(parsed, &found);
    if cases.is_empty() {
        return TestFrames::default();
    }
    let mut loose = attach_assertions(&mut cases, parsed, &found, rules);
    expand_macros(&mut cases, &mut loose, parsed, &found);
    let helpers = file_helpers(source, &cases, &loose);
    for (case, node) in helper_calls(&cases, &found) {
        if let (Some(class), Some(case)) = (helpers.get(parsed.text(node)), cases.get_mut(&case)) {
            assert_through(&mut case.assertions, *class, span(node));
        }
    }
    attach_mocks(&mut cases, parsed, &found);
    attach_skips(&mut cases, parsed, &found);
    attach_properties(&mut cases, &found);
    attach_docs(&mut cases, &found);

    let mut setup: Vec<(usize, Span)> = found
        .iter()
        .filter_map(|m| m.node(capture::TEST_SETUP))
        .map(|node| (node.id(), span(node)))
        .collect();
    setup.sort_by_key(|(_, s)| (s.line, s.col));
    setup.dedup_by_key(|(id, _)| *id);

    let mut ordered: Vec<Case<'_>> = cases.into_values().collect();
    ordered.sort_by_key(|case| case.node.start_byte());
    TestFrames {
        files: vec![TestFile {
            file: parsed.file.clone(),
            framework: framework(&found).unwrap_or_else(|| rules.framework.clone()),
            setup: setup.into_iter().map(|(_, s)| s).collect(),
            cases: ordered
                .into_iter()
                .map(|case| TestCase {
                    name: case.name.map(|(_, name)| name).unwrap_or_default(),
                    span: span(case.node),
                    assertions: case.assertions,
                    mocks: case.mocks,
                    skip: case.skip,
                    property: case.property.map(|(_, framework)| framework),
                    doc: doc(parsed, &case.docs),
                })
                .collect(),
        }],
    }
}

/// Every `@test.case` node, named by the earliest pattern that names it.
fn collect_cases<'t>(parsed: &'t Parsed, found: &[Match<'_, 't>]) -> Cases<'t> {
    let mut cases = Cases::new();
    for m in found {
        let Some(node) = m.node(capture::TEST_CASE) else {
            continue;
        };
        let case = cases.entry(node.id()).or_insert_with(|| Case {
            node,
            name: None,
            assertions: Vec::new(),
            mocks: Vec::new(),
            skip: None,
            property: None,
            docs: BTreeMap::new(),
        });
        if let Some(name) = m.node(capture::TEST_NAME)
            && case
                .name
                .as_ref()
                .is_none_or(|(pattern, _)| m.pattern < *pattern)
        {
            case.name = Some((m.pattern, unquote(parsed.text(name)).to_owned()));
        }
    }
    cases
}

/// The case a capture in `m` belongs to: the match's own case, else the innermost containing.
fn owner(cases: &Cases<'_>, m: &Match<'_, '_>, node: Node<'_>) -> Option<usize> {
    m.node(capture::TEST_CASE)
        .map(|case| case.id())
        .or_else(|| {
            cases
                .values()
                .filter(|case| contains(case.node, node))
                .min_by_key(|case| case.node.end_byte() - case.node.start_byte())
                .map(|case| case.node.id())
        })
}

/// Classifies every assertion and attaches it to its case; the earliest pattern wins per
/// assertion node. The assertions outside every case are returned: a helper's.
fn attach_assertions(
    cases: &mut Cases<'_>,
    parsed: &Parsed,
    found: &[Match<'_, '_>],
    rules: &TestRules,
) -> Vec<Assertion> {
    let mut assertions: BTreeMap<usize, (usize, Option<usize>, Assertion)> = BTreeMap::new();
    for m in found {
        let Some(node) = m.node(capture::TEST_ASSERTION) else {
            continue;
        };
        let key = m
            .prefixed(capture::ASSERTION_IDIOM_PREFIX)
            .map(|(key, _)| key.to_owned())
            .or_else(|| {
                m.node(capture::ASSERTION_NAME)
                    .map(|n| parsed.text(n).split_whitespace().collect())
            })
            .unwrap_or_default();
        let class = rules.classify(&key);
        let entry = (
            m.pattern,
            owner(cases, m, node),
            Assertion {
                kind: class.kind,
                strength: class.strength,
                span: span(node),
            },
        );
        let slot = assertions.entry(node.id()).or_insert_with(|| entry.clone());
        if m.pattern < slot.0 {
            *slot = entry;
        }
    }
    let mut sorted: Vec<(usize, Option<usize>, Assertion)> = assertions.into_values().collect();
    sorted.sort_by_key(|(_, _, a)| (a.span.line, a.span.col));
    let mut loose = Vec::new();
    for (_, owner, assertion) in sorted {
        match owner.and_then(|id| cases.get_mut(&id)) {
            Some(case) => case.assertions.push(assertion),
            None => loose.push(assertion),
        }
    }
    loose
}

/// How an assertion is classified, as a helper passes it on.
#[derive(Debug, Clone, Copy)]
struct Class {
    kind: AssertionKind,
    strength: AssertionStrength,
}

/// The asserting helpers of `source`: each function that is not a case and holds one of the
/// `loose` assertions, with the class of its first strong assertion, else its first.
fn file_helpers(
    source: &Source<'_>,
    cases: &Cases<'_>,
    loose: &[Assertion],
) -> BTreeMap<String, Class> {
    if loose.is_empty() {
        return BTreeMap::new();
    }
    definitions(source.parsed, source.language)
        .iter()
        .filter(|definition| !cases.contains_key(&definition.node.id()))
        .filter_map(|definition| {
            let within = |assertion: &&Assertion| {
                let (outer, inner) = (definition.symbol.span, assertion.span);
                (outer.line, outer.col) <= (inner.line, inner.col)
                    && (inner.end_line, inner.end_col) <= (outer.end_line, outer.end_col)
            };
            let held: Vec<&Assertion> = loose.iter().filter(within).collect();
            let first = held
                .iter()
                .find(|assertion| assertion.strength == AssertionStrength::Strong)
                .or_else(|| held.first())?;
            Some((
                definition.symbol.name.clone(),
                Class {
                    kind: first.kind,
                    strength: first.strength,
                },
            ))
        })
        .collect()
}

/// Makes each invocation of an asserting macro of the file an assertion: of its case when it is
/// in one, else one of the `loose` assertions, which [`file_helpers`] gives the helper holding it.
fn expand_macros(
    cases: &mut Cases<'_>,
    loose: &mut Vec<Assertion>,
    parsed: &Parsed,
    found: &[Match<'_, '_>],
) {
    let macros = file_macros(parsed, found, loose);
    if macros.is_empty() {
        return;
    }
    for m in found {
        let Some(node) = m.node(capture::ASSERTION_EXPANSION) else {
            continue;
        };
        let Some(class) = macros.get(parsed.text(node)) else {
            continue;
        };
        match owner(cases, m, node).and_then(|id| cases.get_mut(&id)) {
            Some(case) => assert_through(&mut case.assertions, *class, span(node)),
            None => assert_through(loose, *class, span(node)),
        }
    }
}

/// The asserting macros of the file: each `@assertion.macro` whose definition holds one of the
/// `loose` assertions (a definition inside a case holds none), with the class of its first strong
/// assertion, else its first.
fn file_macros(
    parsed: &Parsed,
    found: &[Match<'_, '_>],
    loose: &[Assertion],
) -> BTreeMap<String, Class> {
    found
        .iter()
        .filter_map(|m| m.node(capture::ASSERTION_MACRO))
        .filter_map(|name| {
            let outer = span(name.parent()?);
            let held: Vec<&Assertion> = loose
                .iter()
                .filter(|assertion| {
                    let inner = assertion.span;
                    (outer.line, outer.col) <= (inner.line, inner.col)
                        && (inner.end_line, inner.end_col) <= (outer.end_line, outer.end_col)
                })
                .collect();
            let first = held
                .iter()
                .find(|assertion| assertion.strength == AssertionStrength::Strong)
                .or_else(|| held.first())?;
            Some((
                parsed.text(name).to_owned(),
                Class {
                    kind: first.kind,
                    strength: first.strength,
                },
            ))
        })
        .collect()
}

/// Each `@assertion.helper` node inside a case, with its case.
fn helper_calls<'t>(cases: &Cases<'t>, found: &[Match<'_, 't>]) -> Vec<(usize, Node<'t>)> {
    found
        .iter()
        .filter_map(|m| {
            let node = m.node(capture::ASSERTION_HELPER)?;
            Some((owner(cases, m, node)?, node))
        })
        .collect()
}

/// Makes the call at `at` an assertion of `class`: the assertion a convention already captured
/// there takes the class, else one is added.
fn assert_through(assertions: &mut Vec<Assertion>, class: Class, at: Span) {
    let starts =
        |assertion: &Assertion| (assertion.span.line, assertion.span.col) == (at.line, at.col);
    if let Some(assertion) = assertions.iter_mut().find(|assertion| starts(assertion)) {
        assertion.kind = class.kind;
        assertion.strength = class.strength;
        return;
    }
    assertions.push(Assertion {
        kind: class.kind,
        strength: class.strength,
        span: at,
    });
    assertions.sort_by_key(|assertion| (assertion.span.line, assertion.span.col));
}

/// Gives each helper call that names no helper of its own file the class of an asserting helper
/// of that name in another file of the same directory, when the language's tests share helpers
/// by directory (a Go package's `_test.go` helpers, a Python test package's harness).
fn resolve_helpers(frames: &mut TestFrames, parsed: &[Parsed], language: &dyn Language) {
    let spec = language.spec();
    if !spec.has(QueryFile::Tests) || !spec.tables().tests.directory_helpers {
        return;
    }
    let rules = &spec.tables().tests;
    let mut shared: BTreeMap<(String, String), Class> = BTreeMap::new();
    let mut unresolved: Vec<(String, Span, String)> = Vec::new();
    for parsed in parsed {
        let source = Source { parsed, language };
        let query = spec
            .query(QueryFile::Tests, &parsed.grammar)
            .unwrap_or_else(|error| panic!("{} tests.scm: {error}", language.id()));
        let found = matches(&query, parsed);
        let mut cases = collect_cases(parsed, &found);
        let mut loose = attach_assertions(&mut cases, parsed, &found, rules);
        expand_macros(&mut cases, &mut loose, parsed, &found);
        let helpers = file_helpers(&source, &cases, &loose);
        let dir = directory(&parsed.file);
        for (_, node) in helper_calls(&cases, &found) {
            let name = parsed.text(node);
            if !helpers.contains_key(name) {
                unresolved.push((parsed.file.clone(), span(node), name.to_owned()));
            }
        }
        for (name, class) in helpers {
            shared.entry((dir.to_owned(), name)).or_insert(class);
        }
    }
    for (file, at, name) in unresolved {
        let Some(class) = shared.get(&(directory(&file).to_owned(), name)) else {
            continue;
        };
        for test_file in frames
            .files
            .iter_mut()
            .filter(|test_file| test_file.file == file)
        {
            for case in &mut test_file.cases {
                if case.span.line <= at.line && at.line <= case.span.end_line {
                    assert_through(&mut case.assertions, *class, at);
                }
            }
        }
    }
}

/// The directory of `file`, `""` at the root.
fn directory(file: &str) -> &str {
    file.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// Attaches mocks to their case, or to every case when outside all of them.
fn attach_mocks(cases: &mut Cases<'_>, parsed: &Parsed, found: &[Match<'_, '_>]) {
    let mut seen = HashSet::new();
    let mut mocks = Vec::new();
    for m in found {
        let Some(node) = m.node(capture::TEST_MOCK) else {
            continue;
        };
        if !seen.insert(node.id()) {
            continue;
        }
        let target = m.node(capture::MOCK_TARGET).unwrap_or(node);
        let mock = Mock {
            target: parsed.text(target).to_owned(),
            span: span(node),
        };
        mocks.push((owner(cases, m, node), mock));
    }
    for (owner, mock) in mocks {
        match owner {
            Some(id) => {
                if let Some(case) = cases.get_mut(&id) {
                    case.mocks.push(mock);
                }
            }
            None => {
                for case in cases.values_mut() {
                    case.mocks.push(mock.clone());
                }
            }
        }
    }
}

/// Attaches each skip marker to its case; a case keeps its first.
fn attach_skips(cases: &mut Cases<'_>, parsed: &Parsed, found: &[Match<'_, '_>]) {
    let mut skips = Vec::new();
    for m in found {
        let Some(node) = m.node(capture::TEST_SKIP) else {
            continue;
        };
        let skip = SkipMarker {
            reason: m
                .node(capture::SKIP_REASON)
                .map(|r| unquote(parsed.text(r)).to_owned()),
            span: span(node),
        };
        skips.push((owner(cases, m, node), skip));
    }
    for (owner, skip) in skips {
        if let Some(case) = owner.and_then(|id| cases.get_mut(&id)) {
            case.skip.get_or_insert(skip);
        }
    }
}

/// Names each property case's framework: the earliest pattern's `@property.<framework>`.
fn attach_properties(cases: &mut Cases<'_>, found: &[Match<'_, '_>]) {
    for m in found {
        let Some((framework, node)) = m.prefixed(capture::PROPERTY_PREFIX) else {
            continue;
        };
        let Some(case) = owner(cases, m, node).and_then(|id| cases.get_mut(&id)) else {
            continue;
        };
        if case
            .property
            .as_ref()
            .is_none_or(|(pattern, _)| m.pattern < *pattern)
        {
            case.property = Some((m.pattern, framework.to_owned()));
        }
    }
}

/// Attaches each `@test.doc` node to the case its match's `@doc.target` is, if any.
fn attach_docs<'t>(cases: &mut Cases<'t>, found: &[Match<'_, 't>]) {
    for m in found {
        let Some(case) = m
            .node(capture::DOC_TARGET)
            .and_then(|node| cases.get_mut(&node.id()))
        else {
            continue;
        };
        for doc in m.nodes(capture::TEST_DOC) {
            case.docs.insert(doc.id(), doc);
        }
    }
}

/// The texts of `docs` in document order, one per line, each without trailing whitespace.
fn doc(parsed: &Parsed, docs: &BTreeMap<usize, Node<'_>>) -> Option<String> {
    let mut nodes: Vec<&Node<'_>> = docs.values().collect();
    nodes.sort_by_key(|node| node.start_byte());
    let lines: Vec<&str> = nodes
        .into_iter()
        .map(|node| parsed.text(*node).trim_end())
        .collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// The framework named by the earliest pattern with a `@framework.<name>` capture.
fn framework(found: &[Match<'_, '_>]) -> Option<String> {
    found
        .iter()
        .filter_map(|m| {
            m.prefixed(capture::FRAMEWORK_PREFIX)
                .map(|(name, _)| (m.pattern, name.to_owned()))
        })
        .min_by_key(|(pattern, _)| *pattern)
        .map(|(_, name)| name)
}

fn contains(outer: Node<'_>, inner: Node<'_>) -> bool {
    outer.start_byte() <= inner.start_byte() && inner.end_byte() <= outer.end_byte()
}

/// `text` without one pair of surrounding quotes or backticks.
fn unquote(text: &str) -> &str {
    for quote in ['"', '\'', '`'] {
        if let Some(inner) = text
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner;
        }
    }
    text
}
