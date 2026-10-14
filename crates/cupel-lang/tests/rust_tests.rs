// ABOUTME: The Rust TestFrames and TestConventions producers: cases, assertion kinds and strengths,
// ABOUTME: skips, mocks, setup, frameworks, and which test files belong to which source files.
mod support;

use cupel_core::syntax::{
    AssertionKind, AssertionStrength, TestCase, TestConventions, TestFile, TestFrames,
};
use support::{fixture, produce, tree};

use AssertionKind::{Custom, Equality, NotNull, Snapshot, Throws, Truthiness};
use AssertionStrength::{Strong, Weak};

fn frames() -> TestFrames {
    produce("rust", &fixture("rust"))
}

fn file<'a>(frames: &'a TestFrames, path: &str) -> &'a TestFile {
    frames
        .files
        .iter()
        .find(|f| f.file == path)
        .unwrap_or_else(|| panic!("no test file {path}"))
}

/// Whether a case is skipped, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Skip {
    Runs,
    Skipped,
    Because(String),
}

/// (name, assertions as (kind, strength), skip, mock targets, start line) per case.
type CaseRow = (
    String,
    Vec<(AssertionKind, AssertionStrength)>,
    Skip,
    Vec<String>,
    u32,
);

fn rows(file: &TestFile) -> Vec<CaseRow> {
    file.cases.iter().map(row).collect()
}

fn row(case: &TestCase) -> CaseRow {
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

fn case(
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

#[test]
fn integration_tests_are_cases_with_classified_assertions() {
    let frames = frames();
    let ledger = file(&frames, "tests/ledger.rs");
    assert_eq!(ledger.framework, "rust-test");
    assert!(ledger.setup.is_empty());
    assert_eq!(
        rows(ledger),
        [
            case(
                "deposit_adds_to_the_balance",
                &[(Equality, Strong)],
                Skip::Runs,
                &[],
                5
            ),
            case(
                "members_get_a_discount",
                &[(Truthiness, Weak)],
                Skip::Runs,
                &[],
                12
            ),
            case(
                "statements_mention_the_balance",
                &[(NotNull, Weak)],
                Skip::Runs,
                &[],
                17
            ),
            case(
                "parsing_garbage_fails",
                &[(Throws, Weak)],
                Skip::Runs,
                &[],
                22
            ),
            case(
                "huge_deposits_overflow",
                &[(Throws, Strong)],
                Skip::Runs,
                &[],
                28
            ),
            case(
                "statement_is_mailed",
                &[(Equality, Weak)],
                Skip::Because("talks to the network".to_owned()),
                &[],
                36,
            ),
            case(
                "opening_an_account_does_not_panic",
                &[],
                Skip::Runs,
                &[],
                42
            ),
        ]
    );
}

#[test]
fn inline_test_modules_have_fixtures_snapshots_mocks_and_async_tests() {
    let frames = frames();
    let pricing = file(&frames, "src/pricing.rs");
    assert_eq!(pricing.framework, "rust-test");
    assert_eq!(
        pricing.setup.iter().map(|s| s.line).collect::<Vec<_>>(),
        [38]
    );
    assert_eq!(
        rows(pricing),
        [
            case(
                "tally_reports_overdraft",
                &[(Equality, Strong), (Truthiness, Weak)],
                Skip::Runs,
                &[],
                43
            ),
            case(
                "discount_snapshot",
                &[(Snapshot, Strong)],
                Skip::Runs,
                &[],
                50
            ),
            case("the_log_is_written_once", &[], Skip::Runs, &["MockLog"], 55),
        ]
    );
}

#[test]
fn files_without_test_cases_have_no_frames() {
    let frames = frames();
    assert!(frames.files.iter().all(|f| !f.cases.is_empty()));
    assert!(frames.files.iter().all(|f| f.file != "src/lib.rs"));
}

#[test]
fn a_file_of_only_tokio_tests_is_a_tokio_file_and_unknown_assertions_are_custom() {
    let dir = tree(&[(
        "tests/io.rs",
        "#[tokio::test]\n#[ignore]\nasync fn reads() {\n    assert_matches!(read().await, Ok(_));\n}\n",
    )]);
    let frames: TestFrames = produce("rust", dir.path());
    let io = file(&frames, "tests/io.rs");
    assert_eq!(io.framework, "tokio");
    assert_eq!(
        rows(io),
        [case("reads", &[(Custom, Strong)], Skip::Skipped, &[], 3)]
    );
}

#[test]
fn conventions_pair_sources_with_integration_and_inline_tests() {
    let conventions: TestConventions = produce("rust", &fixture("rust"));
    assert!(conventions.is_test("tests/ledger.rs"));
    assert!(conventions.is_test("crates/foo/tests/it.rs"));
    assert!(!conventions.is_test("src/pricing.rs"));
    assert_eq!(
        conventions.test_files_for("src/lib.rs"),
        ["tests/ledger.rs"]
    );
    assert_eq!(
        conventions.test_files_for("src/pricing.rs"),
        ["src/pricing.rs"]
    );
    assert!(conventions.test_files_for("src/decide.rs").is_empty());
    let sources: Vec<&str> = conventions
        .pairs
        .iter()
        .map(|p| p.source.as_str())
        .collect();
    assert_eq!(sources, ["src/lib.rs", "src/pricing.rs"]);
}

#[test]
fn conventions_find_a_stem_named_integration_test_below_a_crate() {
    let dir = tree(&[
        ("crates/bank/src/vault.rs", "pub fn open() {}\n"),
        ("crates/bank/tests/vault.rs", "#[test]\nfn opens() {}\n"),
        ("crates/bank/tests/other.rs", "#[test]\nfn other() {}\n"),
    ]);
    let conventions: TestConventions = produce("rust", dir.path());
    assert_eq!(
        conventions.test_files_for("crates/bank/src/vault.rs"),
        ["crates/bank/tests/vault.rs"]
    );
}
