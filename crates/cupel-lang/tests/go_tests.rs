// ABOUTME: The Go TestFrames and TestConventions producers: testing.T cases, `if … { t.Errorf }` idioms,
// ABOUTME: testify and snapshot assertions, skips, mocks, setup, frameworks, and `_test.go` pairing.
mod support;

use cupel_core::syntax::{
    AssertionKind, AssertionStrength, TestCase, TestConventions, TestFile, TestFrames,
};
use support::{fixture, produce, tree};

use AssertionKind::{Custom, Equality, NotNull, Snapshot, Throws, Truthiness};
use AssertionStrength::{Strong, Weak};

fn frames() -> TestFrames {
    produce("go", &fixture("go"))
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
fn standard_library_tests_classify_the_condition_guarding_each_failure() {
    let frames = frames();
    let ledger = file(&frames, "ledger_test.go");
    assert_eq!(ledger.framework, "go-test");
    assert_eq!(
        ledger.setup.iter().map(|s| s.line).collect::<Vec<_>>(),
        [8, 15]
    );
    assert_eq!(
        rows(ledger),
        [
            case(
                "TestDepositAddsToTheBalance",
                &[(Equality, Strong)],
                Skip::Runs,
                &[],
                13
            ),
            case(
                "TestMembersGetADiscount",
                &[(Truthiness, Weak)],
                Skip::Runs,
                &[],
                22
            ),
            case(
                "TestNewAccountsAreRegistered",
                &[(NotNull, Weak)],
                Skip::Runs,
                &[],
                28
            ),
            case(
                "TestParsingGarbageFails",
                &[(Throws, Weak)],
                Skip::Runs,
                &[],
                35
            ),
            case(
                "TestNewAccountsHaveDistinctIDs",
                &[(Equality, Weak)],
                Skip::Runs,
                &[],
                41
            ),
            case("TestClassify", &[(Equality, Strong)], Skip::Runs, &[], 48),
            case(
                "TestStatementIsMailed",
                &[(Equality, Weak)],
                Skip::Because("talks to the network".to_owned()),
                &[],
                66,
            ),
            case(
                "TestParseCentsTrimsSpaces",
                &[(NotNull, Weak), (Equality, Strong)],
                Skip::Runs,
                &[],
                75
            ),
            case("TestOpeningAnAccountDoesNotPanic", &[], Skip::Runs, &[], 85),
        ]
    );
}

#[test]
fn testify_files_have_testify_assertions_snapshots_and_mocks() {
    let frames = frames();
    let pricing = file(&frames, "pricing_test.go");
    assert_eq!(pricing.framework, "testify");
    assert!(pricing.setup.is_empty());
    assert_eq!(
        rows(pricing),
        [
            case(
                "TestTallyReportsOverdraft",
                &[(Equality, Strong), (Truthiness, Weak)],
                Skip::Runs,
                &[],
                21
            ),
            case(
                "TestParseCentsAcceptsDigits",
                &[(NotNull, Weak), (Equality, Strong)],
                Skip::Runs,
                &[],
                27
            ),
            case(
                "TestParseCentsRejectsBlanks",
                &[(Throws, Strong)],
                Skip::Runs,
                &[],
                33
            ),
            case(
                "TestDiscountSnapshot",
                &[(Snapshot, Strong)],
                Skip::Runs,
                &[],
                38
            ),
            case(
                "TestMailNotifiesTheOwner",
                &[(NotNull, Weak), (Custom, Strong)],
                Skip::Runs,
                &["MockNotifier"],
                42
            ),
        ]
    );
}

#[test]
fn files_without_test_cases_have_no_frames() {
    let frames = frames();
    let files: Vec<&str> = frames.files.iter().map(|f| f.file.as_str()).collect();
    assert_eq!(files, ["ledger_test.go", "pricing_test.go"]);
}

#[test]
fn only_test_functions_taking_testing_t_are_cases_and_gomock_doubles_are_mocks() {
    let dir = tree(&[(
        "store_test.go",
        "package store\n\nimport (\n\t\"testing\"\n\n\t\"go.uber.org/mock/gomock\"\n)\n\n\
         func Testify(t *testing.T) {}\n\n\
         func BenchmarkGet(b *testing.B) {}\n\n\
         func helper(t *testing.T) {}\n\n\
         func TestGet(t *testing.T) {\n\tctrl := gomock.NewController(t)\n\tstore := NewMockStore(ctrl)\n\t\
         if store == nil {\n\t\tt.SkipNow()\n\t}\n\tif !store.Get() {\n\t\tt.Fail()\n\t}\n}\n\n\
         func Test(t *testing.T) {\n\tif err.Error() != \"x\" {\n\t\tt.Fatalf(\"got %v\", err)\n\t}\n}\n",
    )]);
    let frames: TestFrames = produce("go", dir.path());
    let store = file(&frames, "store_test.go");
    assert_eq!(store.framework, "go-test");
    assert_eq!(
        rows(store),
        [
            case(
                "TestGet",
                &[(Truthiness, Weak)],
                Skip::Skipped,
                &["NewMockStore"],
                15
            ),
            case("Test", &[(Equality, Strong)], Skip::Runs, &[], 26),
        ]
    );
}

#[test]
fn conventions_pair_sources_with_test_files_beside_them() {
    let conventions: TestConventions = produce("go", &fixture("go"));
    assert!(conventions.is_test("ledger_test.go"));
    assert!(conventions.is_test("report/format_test.go"));
    assert!(conventions.is_test("report/testdata/golden.go"));
    assert!(!conventions.is_test("ledger.go"));
    assert_eq!(conventions.test_files_for("ledger.go"), ["ledger_test.go"]);
    assert_eq!(
        conventions.test_files_for("pricing.go"),
        ["pricing_test.go"]
    );
    assert!(conventions.test_files_for("decide.go").is_empty());
    let sources: Vec<&str> = conventions
        .pairs
        .iter()
        .map(|p| p.source.as_str())
        .collect();
    assert_eq!(sources, ["ledger.go", "pricing.go"]);
}

#[test]
fn conventions_find_test_files_in_nested_packages() {
    let dir = tree(&[
        ("internal/vault/vault.go", "package vault\n"),
        ("internal/vault/vault_test.go", "package vault\n"),
        ("internal/vault/other_test.go", "package vault\n"),
    ]);
    let conventions: TestConventions = produce("go", dir.path());
    assert_eq!(
        conventions.test_files_for("internal/vault/vault.go"),
        ["internal/vault/vault_test.go"]
    );
}
