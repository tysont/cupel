// ABOUTME: The Go TestFrames and TestConventions producers: testing.T cases, `if … { t.Errorf }` idioms,
// ABOUTME: testify and snapshot assertions, skips, mocks, setup, frameworks, and `_test.go` pairing.
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
            case("TestOpeningAnAccountDoesNotPanic", &[], Runs, &[], 86),
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
            "ledger_helpers_test.go",
            "ledger_property_test.go",
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

#[test]
fn testing_quick_checks_make_property_cases() {
    let dir = tree(&[(
        "digits_test.go",
        r#"package digits

import (
	"testing"
	"testing/quick"
)

func TestDigits(t *testing.T) {
	if err := quick.Check(func(n uint32) bool { return Digits(n) >= 1 }, nil); err != nil {
		t.Error(err)
	}
}

func TestExample(t *testing.T) {
	if Digits(7) != 1 {
		t.Errorf("want 1")
	}
}
"#,
    )]);
    assert_eq!(
        properties("go", dir.path(), "digits_test.go"),
        [
            property(
                "TestDigits",
                Some("testing-quick"),
                &[(Custom, Strong), (NotNull, Weak)]
            ),
            property("TestExample", None, &[(Equality, Strong)]),
        ]
    );
}

#[test]
fn a_helper_handed_the_test_and_a_subtest_run_from_a_named_function_are_assertions() {
    let dir = tree(&[(
        "cli_test.go",
        r#"package cli

import (
	"testing"

	"go.uber.org/mock/gomock"
)

func expectSuccess(output string, err error, t *testing.T) {
	t.Helper()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestRuns(t *testing.T) {
	output, err := Run()
	expectSuccess(output, err, t)
}

func TestTable(t *testing.T) {
	for name, tc := range cases {
		t.Run(name, tc.test)
	}
}

func TestInlineSubtestAssertsNothing(t *testing.T) {
	t.Run("runs", func(t *testing.T) {
		Run()
	})
}

func TestSetupIsNoAssertion(t *testing.T) {
	ctrl := gomock.NewController(t)
	server := newServer(t)
	Run(ctrl, server)
}
"#,
    )]);
    let frames: TestFrames = produce("go", dir.path());
    let cli = &assert_framework(&frames, "cli_test.go", "go-test");
    assert_eq!(
        rows(cli),
        [
            case("TestRuns", &[(NotNull, Weak)], Runs, &[], 16),
            case("TestTable", &[(Custom, Strong)], Runs, &[], 21),
            case("TestInlineSubtestAssertsNothing", &[], Runs, &[], 27),
            case("TestSetupIsNoAssertion", &[], Runs, &[], 33),
        ]
    );
}

#[test]
fn standard_library_equality_calls_compound_conditions_and_switch_cases_pin_values() {
    let dir = tree(&[(
        "flags_test.go",
        r#"package flags

import (
	"bytes"
	"errors"
	"os"
	"reflect"
	"testing"
)

func TestDeepEqual(t *testing.T) {
	if got := Strip(args); !reflect.DeepEqual(got, want) {
		t.Errorf("got %v", got)
	}
}

func TestBytes(t *testing.T) {
	if !bytes.Equal(Render(), []byte("out")) {
		t.Fatal("render")
	}
}

func TestErrorIs(t *testing.T) {
	if err := Write(); !errors.Is(err, os.ErrPermission) {
		t.Errorf("got %v", err)
	}
}

func TestCompound(t *testing.T) {
	if c.Name() == "" || c.Name() != "renamed" {
		t.Error("not renamed")
	}
}

func TestSwitch(t *testing.T) {
	output, err := Run()
	switch {
	case err == nil && output != want:
		t.Errorf("got %q", output)
	case err != nil:
		t.Errorf("unexpected %v", err)
	}
}
"#,
    )]);
    let frames: TestFrames = produce("go", dir.path());
    let flags = &assert_framework(&frames, "flags_test.go", "go-test");
    assert_eq!(
        rows(flags),
        [
            case("TestDeepEqual", &[(Equality, Strong)], Runs, &[], 11),
            case("TestBytes", &[(Equality, Strong)], Runs, &[], 17),
            case("TestErrorIs", &[(Throws, Strong)], Runs, &[], 23),
            case("TestCompound", &[(Equality, Strong)], Runs, &[], 29),
            case(
                "TestSwitch",
                &[(Equality, Strong), (NotNull, Weak)],
                Runs,
                &[],
                35
            ),
        ]
    );
}

#[test]
fn a_helper_of_another_test_file_of_the_package_asserts_as_it_does() {
    let dir = tree(&[
        (
            "helpers_test.go",
            r#"package cli

import (
	"strings"
	"testing"
)

func check(t *testing.T, found, expected string) {
	t.Helper()
	if !strings.Contains(found, expected) {
		t.Errorf("expecting to contain %q", expected)
	}
}

func checkExact(t *testing.T, got, want string) {
	if got != want {
		t.Errorf("got %q, want %q", got, want)
	}
}
"#,
        ),
        (
            "cli_test.go",
            r#"package cli

import "testing"

func TestOutputMentionsTheName(t *testing.T) {
	check(t, Run(), "name")
}

func TestOutputIsExact(t *testing.T) {
	checkExact(t, Run(), "name\n")
}

func TestUnknownHelper(t *testing.T) {
	verify(t, Run())
}
"#,
        ),
        (
            "other/other_test.go",
            "package other\n\nimport \"testing\"\n\nfunc TestElsewhere(t *testing.T) {\n\tcheck(t, \"a\", \"a\")\n}\n",
        ),
    ]);
    let frames: TestFrames = produce("go", dir.path());
    assert_eq!(
        rows(file(&frames, "cli_test.go")),
        [
            case(
                "TestOutputMentionsTheName",
                &[(Truthiness, Weak)],
                Runs,
                &[],
                5
            ),
            case("TestOutputIsExact", &[(Equality, Strong)], Runs, &[], 9),
            case("TestUnknownHelper", &[(Custom, Strong)], Runs, &[], 13),
        ],
        "a helper of another package's directory is not this one's"
    );
    assert_eq!(
        rows(file(&frames, "other/other_test.go")),
        [case("TestElsewhere", &[(Custom, Strong)], Runs, &[], 5)]
    );
}
