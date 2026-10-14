// ABOUTME: The Python TestFrames and TestConventions producers: pytest and unittest cases, assertion idioms,
// ABOUTME: skips, mocks, fixtures and setUp, frameworks, and which test files belong to which modules.
mod support;

use cupel_core::syntax::{AssertionKind, AssertionStrength, TestConventions, TestFrames};
use support::{Skip, case, file, fixture, produce, rows, tree};

use AssertionKind::{Custom, Equality, NotNull, Snapshot, Throws, Truthiness};
use AssertionStrength::{Strong, Weak};

fn frames() -> TestFrames {
    produce("python", &fixture("python"))
}

fn because(reason: &str) -> Skip {
    Skip::Because(reason.to_owned())
}

#[test]
fn pytest_functions_are_cases_with_classified_assert_statements() {
    let frames = frames();
    let ledger = file(&frames, "tests/test_ledger.py");
    assert_eq!(ledger.framework, "pytest");
    assert!(ledger.setup.is_empty());
    assert_eq!(
        rows(ledger),
        [
            case(
                "test_deposit_adds_to_the_balance",
                &[(Equality, Strong)],
                Skip::Runs,
                &[],
                10
            ),
            case(
                "test_statements_mention_the_owner",
                &[(Truthiness, Weak)],
                Skip::Runs,
                &[],
                15
            ),
            case(
                "test_parsing_garbage_gives_nothing",
                &[(Equality, Strong)],
                Skip::Runs,
                &[],
                19
            ),
            case(
                "test_an_opening_account_exists",
                &[(NotNull, Weak)],
                Skip::Runs,
                &[],
                23
            ),
            case(
                "test_negative_deposits_are_rejected",
                &[(Throws, Strong)],
                Skip::Runs,
                &[],
                27
            ),
            case(
                "test_rounding_is_close",
                &[(Equality, Strong)],
                Skip::Runs,
                &[],
                32
            ),
            case(
                "test_statement_is_published",
                &[(Equality, Weak)],
                because("talks to the network"),
                &[],
                37
            ),
            case(
                "test_report_matches_snapshot",
                &[(Snapshot, Strong)],
                Skip::Runs,
                &[],
                41
            ),
            case(
                "test_publish_posts_once",
                &[(Custom, Strong)],
                Skip::Runs,
                &["ledger.report.format.requests"],
                46
            ),
            case(
                "test_rendering_can_be_replaced",
                &[(Equality, Strong)],
                Skip::Runs,
                &["ledger.report.format.render"],
                51
            ),
            case(
                "test_opening_an_account_does_not_raise",
                &[],
                Skip::Runs,
                &[],
                56
            ),
        ]
    );
}

#[test]
fn unittest_methods_have_set_up_skips_mocks_and_assert_methods() {
    let frames = frames();
    let pricing = file(&frames, "tests/test_pricing.py");
    assert_eq!(pricing.framework, "unittest");
    assert_eq!(
        pricing.setup.iter().map(|s| s.line).collect::<Vec<_>>(),
        [8]
    );
    assert_eq!(
        rows(pricing),
        [
            case(
                "test_tally_reports_overdraft",
                &[(Equality, Strong), (Truthiness, Weak)],
                Skip::Runs,
                &[],
                11
            ),
            case(
                "test_members_get_a_discount",
                &[(Equality, Weak)],
                because("pricing tables are being revised"),
                &[],
                17
            ),
            case(
                "test_tally_logs_once",
                &[(Custom, Strong)],
                Skip::Runs,
                &["ledger.pricing.log_total"],
                21
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
            "tests/test_ledger.py",
            "tests/test_pricing.py",
            "tests/test_quality_signals.py"
        ]
    );
}

#[test]
fn skip_markers_mocks_fixtures_and_more_assertion_idioms() {
    let dir = tree(&[(
        "tests/test_misc.py",
        "import sys\n\
         import pytest\n\
         \n\
         @pytest.fixture(scope=\"module\")\n\
         def ledger():\n    return []\n\
         \n\
         @pytest.mark.skipif(sys.platform == \"win32\", reason=\"posix only\")\n\
         def test_paths(ledger):\n    assert \"a\" in ledger\n\
         \n\
         @pytest.mark.xfail\n\
         def test_rounding():\n    assert 1 > 0\n\
         \n\
         def test_skipped_inside():\n    pytest.skip(\"not today\")\n    assert not sys\n\
         \n\
         def test_doubles(mocker, monkeypatch):\n    \
         mocker.patch(\"ledger.send\")\n    \
         monkeypatch.setenv(\"HOME\", \"/tmp\")\n    \
         client = MagicMock()\n    \
         assert client is not sys\n\
         \n\
         def helper():\n    assert True\n\
         \n\
         class TestNested:\n    \
         def setup_method(self):\n        self.x = 1\n\n    \
         def test_raises(self):\n        with pytest.raises(KeyError):\n            {}[1]\n",
    )]);
    let frames: TestFrames = produce("python", dir.path());
    let misc = file(&frames, "tests/test_misc.py");
    assert_eq!(misc.framework, "pytest");
    assert_eq!(
        misc.setup.iter().map(|s| s.line).collect::<Vec<_>>(),
        [5, 30]
    );
    assert_eq!(
        rows(misc),
        [
            case(
                "test_paths",
                &[(Custom, Weak)],
                because("posix only"),
                &[],
                9
            ),
            case("test_rounding", &[(Custom, Weak)], Skip::Skipped, &[], 13),
            case(
                "test_skipped_inside",
                &[(Truthiness, Weak)],
                because("not today"),
                &[],
                16
            ),
            case(
                "test_doubles",
                &[(Equality, Weak)],
                Skip::Runs,
                &["ledger.send", "HOME", "MagicMock"],
                20
            ),
            case("test_raises", &[(Throws, Strong)], Skip::Runs, &[], 33),
        ]
    );
}

#[test]
fn unittest_assert_methods_map_to_kinds_and_strengths() {
    let dir = tree(&[(
        "tests/test_methods.py",
        "import unittest\n\
         from unittest.mock import patch\n\
         \n\
         class Methods(unittest.TestCase):\n    \
         @unittest.skipIf(True, \"too old\")\n    \
         @patch.object(Ledger, \"send\")\n    \
         def test_all(self, send):\n        \
         self.assertIsNone(a)\n        \
         self.assertIsNotNone(a)\n        \
         self.assertIn(a, b)\n        \
         self.assertAlmostEqual(a, b)\n        \
         with self.assertRaises(ValueError):\n            pass\n        \
         with patch(\"ledger.clock\"):\n            pass\n",
    )]);
    let frames: TestFrames = produce("python", dir.path());
    let methods = file(&frames, "tests/test_methods.py");
    assert_eq!(methods.framework, "unittest");
    assert_eq!(
        rows(methods),
        [case(
            "test_all",
            &[
                (Equality, Strong),
                (NotNull, Weak),
                (Custom, Weak),
                (Equality, Strong),
                (Throws, Strong),
            ],
            because("too old"),
            &["Ledger", "ledger.clock"],
            7
        )]
    );
}

#[test]
fn conventions_pair_modules_with_test_files_by_name() {
    let conventions: TestConventions = produce("python", &fixture("python"));
    assert!(conventions.is_test("tests/test_ledger.py"));
    assert!(conventions.is_test("tests/conftest.py"));
    assert!(conventions.is_test("pkg/ledger_test.py"));
    assert!(conventions.is_test("test_top.py"));
    assert!(!conventions.is_test("src/ledger/pricing.py"));
    assert_eq!(
        conventions.test_files_for("src/ledger/pricing.py"),
        ["tests/test_pricing.py"]
    );
    assert!(
        conventions
            .test_files_for("src/ledger/decide.py")
            .is_empty()
    );
    let sources: Vec<&str> = conventions
        .pairs
        .iter()
        .map(|p| p.source.as_str())
        .collect();
    assert_eq!(sources, ["src/ledger/pricing.py"]);
}

#[test]
fn conventions_find_sibling_nested_and_per_project_test_files() {
    let dir = tree(&[
        ("app/clock.py", "def now():\n    pass\n"),
        ("app/test_clock.py", "def test_now():\n    pass\n"),
        ("app/clock_test.py", "def test_now():\n    pass\n"),
        ("tests/unit/test_clock.py", "def test_now():\n    pass\n"),
        ("services/api/src/api/routes.py", "def get():\n    pass\n"),
        (
            "services/api/tests/test_routes.py",
            "def test_get():\n    pass\n",
        ),
    ]);
    let conventions: TestConventions = produce("python", dir.path());
    assert_eq!(
        conventions.test_files_for("app/clock.py"),
        [
            "app/test_clock.py",
            "app/clock_test.py",
            "tests/unit/test_clock.py"
        ]
    );
    assert_eq!(
        conventions.test_files_for("services/api/src/api/routes.py"),
        ["services/api/tests/test_routes.py"]
    );
}
