// ABOUTME: The Rust TestFrames and TestConventions producers: cases, assertion kinds and strengths,
// ABOUTME: skips, mocks, setup, frameworks, and which test files belong to which source files.
mod support;

use cupel_core::syntax::{AssertionKind, AssertionStrength, TestConventions, TestFrames};
use support::{Skip, case, file, fixture, produce, rows, tree};

use AssertionKind::{Custom, Equality, NotNull, Snapshot, Throws, Truthiness};
use AssertionStrength::{Strong, Weak};

fn frames() -> TestFrames {
    produce("rust", &fixture("rust"))
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
        ["tests/ledger.rs", "tests/quality_signals.rs"]
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

/// The assertions of the only case in a Rust test file holding `body` as one `#[test]`.
fn assertions_in(body: &str) -> Vec<(AssertionKind, AssertionStrength)> {
    let dir = tree(&[("tests/t.rs", &format!("#[test]\nfn t() {{\n{body}\n}}\n"))]);
    let frames: TestFrames = produce("rust", dir.path());
    rows(file(&frames, "tests/t.rs"))[0].1.clone()
}

#[test]
fn comparisons_and_predicates_true_of_one_value_pin_that_value() {
    assert_eq!(assertions_in("assert!(total == 3);"), [(Equality, Strong)]);
    assert_eq!(
        assertions_in("assert!(found.is_none());"),
        [(Equality, Strong)]
    );
    assert_eq!(
        assertions_in("assert!(findings.is_empty(), \"{findings:?}\");"),
        [(Equality, Strong)]
    );
    assert_eq!(assertions_in("assert!(a != b);"), [(Equality, Weak)]);
    assert_eq!(assertions_in("assert!(x.is_some());"), [(NotNull, Weak)]);
    assert_eq!(assertions_in("assert!(ready(x));"), [(Truthiness, Weak)]);
    assert_eq!(
        assertions_in("assert!(items.iter().all(|i| i.len() == 2));"),
        [(Truthiness, Weak)],
        "a comparison inside a closure does not pin the asserted value"
    );
    assert_eq!(
        assertions_in("assert!(!list.is_empty());"),
        [(Truthiness, Weak)],
        "a negated predicate rules one value out"
    );
}

#[test]
fn assertion_helpers_and_process_assertions_are_assertions() {
    assert_eq!(
        assertions_in("assert_fixture(\"cargo\", &expected);"),
        [(Custom, Strong)]
    );
    assert_eq!(
        assertions_in("support::assert_usage(&outcome, \"--paths\");"),
        [(Custom, Strong)]
    );
    assert_eq!(
        assertions_in("cupel().arg(\"--version\").assert().success().stdout(\"x\");"),
        [(Custom, Strong)]
    );
    assert_eq!(assertions_in("helper(&outcome);"), []);
}
