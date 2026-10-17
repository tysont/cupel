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
//! - `@test.mock` on a test double's creation, with optional `@mock.target` naming what it
//!   replaces (else the mock node's text).
//! - `@test.skip` on a skip marker, with optional `@skip.reason` (the reason's text, without
//!   quotes).
//! - `@test.setup` on a setup or fixture block; reported per file.
//! - `@framework.<name>` on any node: the file's framework is the name from the earliest
//!   pattern that matched, else `[tests].framework`.
//! - `@property.<framework>` on any node: the case is a property test whose inputs `<framework>`
//!   generates (ADR-095); the earliest pattern wins per case.
//!
//! An assertion, mock, skip, or property captured in the same match as a `@test.case` belongs to that
//! case (attributes and decorators sit outside the function they mark); otherwise it belongs
//! to the innermost case containing it. A mock outside every case (a module-level mock)
//! belongs to every case in the file. Only files with at least one case are reported.
use std::collections::{BTreeMap, HashSet};

use cupel_core::FactKind;
use cupel_core::syntax::{Assertion, Mock, SkipMarker, Span, TestCase, TestFile, TestFrames};
use tree_sitter::Node;

use crate::contract::capture;
use crate::parse::Parsed;
use crate::producer::{QueryProducer, Source};
use crate::query::{Match, matches, span};
use crate::spec::QueryFile;
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
}

/// Cases by node ID.
type Cases<'t> = BTreeMap<usize, Case<'t>>;

inventory::submit!(crate::generic::GenericProducer {
    kind: FactKind::TestFrames,
    drives: |spec| spec.has(QueryFile::Tests),
    attach: |language, cache, reg| {
        reg.register::<TestFrames, _>(QueryProducer::new(language, cache, extract));
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
    attach_assertions(&mut cases, parsed, &found, rules);
    attach_mocks(&mut cases, parsed, &found);
    attach_skips(&mut cases, parsed, &found);
    attach_properties(&mut cases, &found);

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

/// Classifies every assertion; the earliest pattern wins per assertion node.
fn attach_assertions(
    cases: &mut Cases<'_>,
    parsed: &Parsed,
    found: &[Match<'_, '_>],
    rules: &TestRules,
) {
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
    for (_, owner, assertion) in sorted {
        if let Some(case) = owner.and_then(|id| cases.get_mut(&id)) {
            case.assertions.push(assertion);
        }
    }
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
