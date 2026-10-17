// ABOUTME: The Rust TestFrames and TestConventions producers: cases, assertion kinds and strengths,
// ABOUTME: skips, mocks, setup, frameworks, and which test files belong to which source files.
mod support;

use cupel_core::syntax::{AssertionKind, AssertionStrength, TestConventions, TestFrames};
use support::Skip::{Because, Runs, Skipped};
use support::{
    assert_framework, assert_test_file, case, file, fixture, produce, properties, property, rows,
    tree,
};

use AssertionKind::{Custom, Equality, NotNull, Snapshot, Throws, Truthiness};
use AssertionStrength::{Strong, Weak};

fn frames() -> TestFrames {
    produce("rust", &fixture("rust"))
}

#[test]
fn integration_tests_are_cases_with_classified_assertions() {
    let ledger = &assert_test_file("rust", "tests/ledger.rs", "rust-test", &[]);
    assert_eq!(
        rows(ledger),
        [
            case(
                "deposit_adds_to_the_balance",
                &[(Equality, Strong)],
                Runs,
                &[],
                5
            ),
            case(
                "members_get_a_discount",
                &[(Truthiness, Weak)],
                Runs,
                &[],
                12
            ),
            case(
                "statements_mention_the_balance",
                &[(NotNull, Weak)],
                Runs,
                &[],
                17
            ),
            case("parsing_garbage_fails", &[(Throws, Weak)], Runs, &[], 22),
            case("huge_deposits_overflow", &[(Throws, Strong)], Runs, &[], 28),
            case(
                "statement_is_mailed",
                &[(Equality, Weak)],
                Because("talks to the network".to_owned()),
                &[],
                36,
            ),
            case("opening_an_account_does_not_panic", &[], Runs, &[], 42),
        ]
    );
}

#[test]
fn inline_test_modules_have_fixtures_snapshots_mocks_and_async_tests() {
    let frames = frames();
    let pricing = &assert_framework(&frames, "src/pricing.rs", "rust-test");
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
                Runs,
                &[],
                43
            ),
            case("discount_snapshot", &[(Snapshot, Strong)], Runs, &[], 50),
            case("the_log_is_written_once", &[], Runs, &["MockLog"], 55),
        ]
    );
}

#[test]
fn files_without_test_cases_have_no_frames() {
    let frames = frames();
    let files: Vec<(&str, bool)> = frames
        .files
        .iter()
        .map(|f| (f.file.as_str(), f.cases.is_empty()))
        .collect();
    assert_eq!(
        files,
        [
            ("src/pricing.rs", false),
            ("tests/ledger.rs", false),
            ("tests/properties.rs", false),
            ("tests/quality_signals.rs", false),
        ],
        "only files with test cases; src/lib.rs has none"
    );
}

#[test]
fn a_file_of_only_tokio_tests_is_a_tokio_file_and_unknown_assertions_are_custom() {
    let dir = tree(&[(
        "tests/io.rs",
        "#[tokio::test]\n#[ignore]\nasync fn reads() {\n    assert_matches!(read().await, Ok(_));\n}\n",
    )]);
    let frames: TestFrames = produce("rust", dir.path());
    let io = &assert_framework(&frames, "tests/io.rs", "tokio");
    assert_eq!(
        rows(io),
        [case("reads", &[(Custom, Strong)], Skipped, &[], 3)]
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
        [
            "tests/ledger.rs",
            "tests/properties.rs",
            "tests/quality_signals.rs"
        ]
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

/// Assertion bodies and how each is classified (ADR-070): comparisons and predicates true of one
/// value pin it; inequality, presence, and plain truthiness do not; a comparison inside a
/// closure and a negated predicate stay weak; `assert_…` helpers and `assert_cmd` are custom.
const ASSERTION_FORMS: &[(&str, &[(AssertionKind, AssertionStrength)])] = &[
    ("assert!(total == 3);", &[(Equality, Strong)]),
    ("assert!(found.is_none());", &[(Equality, Strong)]),
    (
        "assert!(findings.is_empty(), \"{findings:?}\");",
        &[(Equality, Strong)],
    ),
    ("assert!(a != b);", &[(Equality, Weak)]),
    ("assert!(x.is_some());", &[(NotNull, Weak)]),
    ("assert!(ready(x));", &[(Truthiness, Weak)]),
    (
        "assert!(items.iter().all(|i| i.len() == 2));",
        &[(Truthiness, Weak)],
    ),
    ("assert!(!list.is_empty());", &[(Truthiness, Weak)]),
    ("assert_fixture(\"cargo\", &expected);", &[(Custom, Strong)]),
    (
        "support::assert_usage(&outcome, \"--paths\");",
        &[(Custom, Strong)],
    ),
    (
        "cupel().arg(\"--version\").assert().success().stdout(\"x\");",
        &[(Custom, Strong)],
    ),
    ("helper(&outcome);", &[]),
];

#[test]
fn each_assertion_form_has_its_kind_and_strength() {
    for (body, expected) in ASSERTION_FORMS {
        assert_eq!(assertions_in(body), *expected, "{body}");
    }
}

#[test]
fn property_tests_name_their_framework_and_proptest_blocks_hold_cases() {
    let dir = tree(&[(
        "tests/props.rs",
        r"use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8))]

    #[test]
    fn digits_count(n in 0u32..110_000) {
        prop_assert_eq!(digits(n), n.to_string().len() as u32);
        for _ in 0..2 {
            assert!(n < 110_000);
        }
    }

    #[test]
    fn halves(a in any::<u8>()) {
        prop_assert!(a / 2 <= a);
    }
}

#[test]
fn closure_form() {
    proptest!(|(n in 0u32..10)| {
        prop_assert!(n < 10);
    });
}

#[quickcheck]
fn reversing_twice_is_identity(xs: Vec<u32>) -> bool {
    xs.clone() == xs
}

#[test]
fn runner_form() {
    quickcheck(reversing_twice_is_identity as fn(Vec<u32>) -> bool);
}

#[test]
fn example() {
    assert_eq!(1, 1);
}
",
    )]);
    assert_eq!(
        properties("rust", dir.path(), "tests/props.rs"),
        [
            property(
                "digits_count",
                Some("proptest"),
                &[(Equality, Strong), (Truthiness, Weak)]
            ),
            property("halves", Some("proptest"), &[(Truthiness, Weak)]),
            property("closure_form", Some("proptest"), &[(Truthiness, Weak)]),
            property(
                "reversing_twice_is_identity",
                Some("quickcheck"),
                &[(Custom, Strong)]
            ),
            property("runner_form", Some("quickcheck"), &[(Custom, Strong)]),
            property("example", None, &[(Equality, Strong)]),
        ]
    );
}
