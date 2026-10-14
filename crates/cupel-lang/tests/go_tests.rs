// ABOUTME: The Go TestFrames and TestConventions producers: testing.T cases, `if … { t.Errorf }` idioms,
// ABOUTME: testify and snapshot assertions, skips, mocks, setup, frameworks, and `_test.go` pairing.
mod support;

use cupel_core::syntax::{AssertionKind, AssertionStrength, TestConventions, TestFrames};
use support::Skip::{Because, Runs, Skipped};
use support::{assert_framework, assert_test_file, case, fixture, produce, rows, tree};

use AssertionKind::{Custom, Equality, NotNull, Snapshot, Throws, Truthiness};
use AssertionStrength::{Strong, Weak};

fn frames() -> TestFrames {
    produce("go", &fixture("go"))
}

#[test]
fn standard_library_tests_classify_the_condition_guarding_each_failure() {
    let frames = frames();
    let ledger = &assert_framework(&frames, "ledger_test.go", "go-test");
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
                Runs,
                &[],
                13
            ),
            case(
                "TestMembersGetADiscount",
                &[(Truthiness, Weak)],
                Runs,
                &[],
                22
            ),
            case(
                "TestNewAccountsAreRegistered",
                &[(NotNull, Weak)],
                Runs,
                &[],
                28
            ),
            case("TestParsingGarbageFails", &[(Throws, Weak)], Runs, &[], 35),
            case(
                "TestNewAccountsHaveDistinctIDs",
                &[(Equality, Weak)],
                Runs,
                &[],
                41
            ),
            case("TestClassify", &[(Equality, Strong)], Runs, &[], 48),
            case(
                "TestStatementIsMailed",
                &[(Equality, Weak)],
                Because("talks to the network".to_owned()),
                &[],
                66,
            ),
            case(
                "TestParseCentsTrimsSpaces",
                &[(NotNull, Weak), (Equality, Strong)],
                Runs,
                &[],
                75
            ),
            case("TestOpeningAnAccountDoesNotPanic", &[], Runs, &[], 85),
        ]
    );
}

#[test]
fn testify_files_have_testify_assertions_snapshots_and_mocks() {
    let pricing = &assert_test_file("go", "pricing_test.go", "testify", &[]);
    assert_eq!(
        rows(pricing),
        [
            case(
                "TestTallyReportsOverdraft",
                &[(Equality, Strong), (Truthiness, Weak)],
                Runs,
                &[],
                21
            ),
            case(
                "TestParseCentsAcceptsDigits",
                &[(NotNull, Weak), (Equality, Strong)],
                Runs,
                &[],
                27
            ),
            case(
                "TestParseCentsRejectsBlanks",
                &[(Throws, Strong)],
                Runs,
                &[],
                33
            ),
            case("TestDiscountSnapshot", &[(Snapshot, Strong)], Runs, &[], 38),
            case(
                "TestMailNotifiesTheOwner",
                &[(NotNull, Weak), (Custom, Strong)],
                Runs,
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
    assert_eq!(
        files,
        [
            "ledger_quality_test.go",
            "ledger_test.go",
            "pricing_test.go"
        ]
    );
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
    let store = &assert_framework(&frames, "store_test.go", "go-test");
    assert_eq!(
        rows(store),
        [
            case(
                "TestGet",
                &[(Truthiness, Weak)],
                Skipped,
                &["NewMockStore"],
                15
            ),
            case("Test", &[(Equality, Strong)], Runs, &[], 26),
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
