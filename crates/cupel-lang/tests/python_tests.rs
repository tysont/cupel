// ABOUTME: The Python TestFrames and TestConventions producers: pytest and unittest cases, assertion idioms,
// ABOUTME: skips, mocks, fixtures and setUp, frameworks, and which test files belong to which modules.
mod support;

use cupel_core::syntax::{AssertionKind, AssertionStrength, TestConventions, TestFrames};
use support::Skip::{Because, Runs};
use support::{
    Expected, assert_framework, assert_source_file, assert_test_file, case, expected, fixture,
    produce, properties, property, rows, tree,
};

use AssertionKind::{Custom, Equality, NotNull, Snapshot, Throws, Truthiness};
use AssertionStrength::{Strong, Weak};

fn frames() -> TestFrames {
    produce("python", &fixture("python"))
}

fn because(reason: &str) -> support::Skip {
    Because(reason.to_owned())
}

const LEDGER_CASES: &[Expected] = &[
    (
        "test_deposit_adds_to_the_balance",
        &[(Equality, Strong)],
        None,
        &[],
        10,
    ),
    (
        "test_statements_mention_the_owner",
        &[(Truthiness, Weak)],
        None,
        &[],
        15,
    ),
    (
        "test_parsing_garbage_gives_nothing",
        &[(Equality, Strong)],
        None,
        &[],
        19,
    ),
    (
        "test_an_opening_account_exists",
        &[(NotNull, Weak)],
        None,
        &[],
        23,
    ),
    (
        "test_negative_deposits_are_rejected",
        &[(Throws, Strong)],
        None,
        &[],
        27,
    ),
    (
        "test_rounding_is_close",
        &[(Equality, Strong)],
        None,
        &[],
        32,
    ),
    (
        "test_statement_is_published",
        &[(Equality, Weak)],
        Some(Some("talks to the network")),
        &[],
        37,
    ),
    (
        "test_report_matches_snapshot",
        &[(Snapshot, Strong)],
        None,
        &[],
        41,
    ),
    (
        "test_publish_posts_once",
        &[(Custom, Strong)],
        None,
        &["ledger.report.format.requests"],
        46,
    ),
    (
        "test_rendering_can_be_replaced",
        &[(Equality, Strong)],
        None,
        &["ledger.report.format.render"],
        51,
    ),
    ("test_opening_an_account_does_not_raise", &[], None, &[], 56),
];

#[test]
fn pytest_functions_are_cases_with_classified_assert_statements() {
    let ledger = &assert_test_file("python", "tests/test_ledger.py", "pytest", &[]);
    assert_eq!(rows(ledger), expected(LEDGER_CASES));
}

#[test]
fn unittest_methods_have_set_up_skips_mocks_and_assert_methods() {
    let frames = frames();
    let pricing = &assert_framework(&frames, "tests/test_pricing.py", "unittest");
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
                Runs,
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
                Runs,
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
            "tests/test_ledger_case.py",
            "tests/test_pricing.py",
            "tests/test_properties.py",
            "tests/test_quality_signals.py"
        ]
    );
}

const MISC_CASES: &[Expected] = &[
    (
        "test_paths",
        &[(Custom, Weak)],
        Some(Some("posix only")),
        &[],
        9,
    ),
    ("test_rounding", &[(Custom, Weak)], Some(None), &[], 13),
    (
        "test_skipped_inside",
        &[(Truthiness, Weak)],
        Some(Some("not today")),
        &[],
        16,
    ),
    (
        "test_doubles",
        &[(Equality, Weak)],
        None,
        &["ledger.send", "HOME", "MagicMock"],
        20,
    ),
    ("test_raises", &[(Throws, Strong)], None, &[], 33),
];

#[test]
fn skip_markers_mocks_fixtures_and_more_assertion_idioms() {
    let misc = &assert_source_file(
        "python",
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
        "pytest",
    );
    assert_eq!(
        misc.setup.iter().map(|s| s.line).collect::<Vec<_>>(),
        [5, 30]
    );
    assert_eq!(rows(misc), expected(MISC_CASES));
}

#[test]
fn unittest_assert_methods_map_to_kinds_and_strengths() {
    let methods = &assert_source_file(
        "python",
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
        "unittest",
    );
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

#[test]
fn hypothesis_tests_are_property_cases() {
    let dir = tree(&[(
        "tests/test_props.py",
        r"import hypothesis
from hypothesis import given, strategies as st


@given(st.integers())
def test_digits(n):
    assert digits(n) >= 1


@hypothesis.given(st.text())
def test_text(s):
    assert s == s


class TestThing:
    @given(st.integers())
    def test_method(self, n):
        assert n == n


def test_example():
    assert 1 == 1
",
    )]);
    assert_eq!(
        properties("python", dir.path(), "tests/test_props.py"),
        [
            property("test_digits", Some("hypothesis"), &[(Custom, Weak)]),
            property("test_text", Some("hypothesis"), &[(Equality, Strong)]),
            property("test_method", Some("hypothesis"), &[(Equality, Strong)]),
            property("test_example", None, &[(Equality, Strong)]),
        ]
    );
}

/// A test package whose harness module defines an asserting `TestCase` method, a test module
/// beside it with its own helpers, and a test module in another directory.
fn harness_package() -> tempfile::TempDir {
    tree(&[
        (
            "pkg/test/harness.py",
            "import unittest\n\
             \n\
             class TestCase(unittest.TestCase):\n    \
             def flakes(self, source, *expected):\n        \
             self.assertEqual(check(source), list(expected))\n\
             \n    \
             def make_file(self, text):\n        \
             return write(text)\n",
        ),
        (
            "pkg/test/test_builtin.py",
            "from pkg.test.harness import TestCase\n\
             \n\
             def verify(found):\n    \
             assert found == []\n\
             \n\
             class TestBuiltins(TestCase):\n    \
             def has_errors(self, path, errors):\n        \
             self.assertIn(errors, path)\n\
             \n    \
             def test_unbound_local(self):\n        \
             self.flakes('print(range)', UndefinedLocal)\n\
             \n    \
             def test_errors(self):\n        \
             self.has_errors('a.py', [])\n\
             \n    \
             def test_verified(self):\n        \
             verify(check('x = 1'))\n\
             \n    \
             def test_file_only(self):\n        \
             self.make_file('x = 1')\n",
        ),
        (
            "pkg/other/test_elsewhere.py",
            "import unittest\n\
             \n\
             class TestElsewhere(unittest.TestCase):\n    \
             def test_no_helper_beside_it(self):\n        \
             self.flakes('x')\n",
        ),
    ])
}

#[test]
fn a_call_of_an_asserting_helper_or_of_a_base_class_method_beside_it_asserts() {
    let dir = harness_package();
    let frames: TestFrames = produce("python", dir.path());
    assert_eq!(
        rows(&assert_framework(
            &frames,
            "pkg/test/test_builtin.py",
            "unittest"
        )),
        [
            case("test_unbound_local", &[(Equality, Strong)], Runs, &[], 10),
            case("test_errors", &[(Custom, Weak)], Runs, &[], 13),
            case("test_verified", &[(Equality, Strong)], Runs, &[], 16),
            case("test_file_only", &[], Runs, &[], 19),
        ]
    );
    assert_eq!(
        rows(&assert_framework(
            &frames,
            "pkg/other/test_elsewhere.py",
            "unittest"
        )),
        [case("test_no_helper_beside_it", &[], Runs, &[], 4)]
    );
}
