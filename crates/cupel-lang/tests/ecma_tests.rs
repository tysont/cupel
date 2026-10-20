// ABOUTME: The JavaScript and TypeScript TestFrames and TestConventions producers: vitest, jest, mocha with
// ABOUTME: chai, and node:test cases, assertion kinds and strengths, skips, mocks, setup, and test pairing.
mod support;

use cupel_core::syntax::{AssertionKind, AssertionStrength, TestConventions, TestFrames};
use support::Skip::{Because, Runs, Skipped};
use support::{
    Expected, assert_framework, assert_test_file, case, expected, fixture, produce, properties,
    property, rows, tree,
};

use AssertionKind::{Custom, Equality, NotNull, Snapshot, Throws, Truthiness};
use AssertionStrength::{Strong, Weak};

fn frames(lang: &str) -> TestFrames {
    produce(lang, &fixture(lang))
}

/// The module `math.test.js` mocks.
const MODULE: &str = "../src/report.cjs";

const MATH_CASES: &[Expected] = &[
    (
        "adds two numbers",
        &[(Equality, Strong)],
        None,
        &[MODULE],
        14,
    ),
    (
        "is truthy for non-zero sums",
        &[(Truthiness, Weak)],
        None,
        &[MODULE],
        18,
    ),
    (
        "returns a defined value",
        &[(NotNull, Weak)],
        None,
        &[MODULE],
        22,
    ),
    (
        "does not return zero",
        &[(Equality, Weak)],
        None,
        &[MODULE],
        26,
    ),
    (
        "handles huge numbers",
        &[(Equality, Strong)],
        Some(None),
        &[MODULE],
        30,
    ),
    ("can be called without checking", &[], None, &[MODULE], 34),
    (
        "rejects empty amounts",
        &[(Throws, Strong)],
        None,
        &[MODULE],
        40,
    ),
    ("throws on nothing", &[(Throws, Weak)], None, &[MODULE], 44),
    (
        "matches the range snapshot",
        &[(Snapshot, Strong)],
        None,
        &[MODULE],
        48,
    ),
    (
        "reports what it parsed",
        &[(Equality, Strong)],
        None,
        &[MODULE, "vi.fn"],
        53,
    ),
];

#[test]
fn vitest_cases_classify_expect_matchers_and_share_module_mocks() {
    let math = &assert_test_file("javascript", "test/math.test.js", "vitest", &[10]);
    assert_eq!(rows(math), expected(MATH_CASES));
}

#[test]
fn node_test_cases_classify_assert_calls_and_skip_with_a_reason() {
    let decide = &assert_test_file("javascript", "test/decide.test.js", "node-test", &[]);
    assert_eq!(
        rows(decide),
        [
            case(
                "classifies overdrawn balances",
                &[(Equality, Strong)],
                Runs,
                &[],
                5
            ),
            case(
                "does not call an empty balance rich",
                &[(Equality, Weak)],
                Runs,
                &[],
                9
            ),
            case(
                "labels through a stub",
                &[(Truthiness, Weak)],
                Runs,
                &["mock.fn"],
                13
            ),
            case(
                "waits for the ledger server",
                &[(Equality, Strong)],
                Because("needs a ledger server".to_owned()),
                &[],
                18
            ),
        ]
    );
}

#[test]
fn jest_globals_mocha_with_chai_and_in_source_tests_are_recognized() {
    let frames = frames("javascript");
    let report = &assert_framework(&frames, "test/report.test.js", "jest");
    let module = "../src/format.cjs";
    assert_eq!(
        rows(report),
        [
            case(
                "joins one line per entry",
                &[(Equality, Strong)],
                Runs,
                &[module],
                5
            ),
            case(
                "formats currency symbols",
                &[(Snapshot, Strong)],
                Skipped,
                &[module],
                9
            ),
        ]
    );
    let format = &assert_framework(&frames, "test/format.spec.js", "mocha");
    assert_eq!(
        rows(format),
        [
            case("shows two decimals", &[(Equality, Strong)], Runs, &[], 6),
            case(
                "is not called by accident",
                &[(Truthiness, Weak)],
                Runs,
                &["sinon.spy"],
                10
            ),
        ]
    );
    let inline = &assert_framework(&frames, "src/inline.js", "vitest");
    assert_eq!(
        rows(inline),
        [case("doubles", &[(Equality, Strong)], Runs, &[], 8)]
    );
}

#[test]
fn typescript_vitest_cases_match_their_javascript_twins() {
    let math = &assert_test_file("typescript", "src/math.test.ts", "vitest", &[13]);
    let module = "./legacy";
    assert_eq!(
        rows(math),
        [
            case(
                "adds two numbers",
                &[(Equality, Strong)],
                Runs,
                &[module],
                7
            ),
            case(
                "is truthy for non-zero sums",
                &[(Truthiness, Weak)],
                Runs,
                &[module],
                17
            ),
            case(
                "returns a defined value",
                &[(NotNull, Weak)],
                Runs,
                &[module],
                21
            ),
            case(
                "does not return zero",
                &[(Equality, Weak)],
                Runs,
                &[module],
                25
            ),
            case(
                "handles huge numbers",
                &[(Equality, Strong)],
                Skipped,
                &[module],
                29
            ),
            case("can be called without checking", &[], Runs, &[module], 33),
            case(
                "matches its snapshot",
                &[(Snapshot, Strong)],
                Runs,
                &[module],
                39
            ),
            case(
                "is consumed by a spy",
                &[(Equality, Strong)],
                Runs,
                &[module, "vi.fn"],
                44
            ),
        ]
    );
}

#[test]
fn typescript_node_test_jest_and_tsx_test_files_are_recognized() {
    let frames = frames("typescript");
    let decide = &assert_framework(&frames, "test/decide.test.ts", "node-test");
    assert_eq!(
        rows(decide),
        [
            case(
                "classifies overdrawn balances",
                &[(Equality, Strong)],
                Runs,
                &[],
                5
            ),
            case("rejects empty amounts", &[(Throws, Strong)], Runs, &[], 9),
            case(
                "waits for the ledger server",
                &[(Truthiness, Weak)],
                Because("needs a ledger server".to_owned()),
                &[],
                13
            ),
        ]
    );
    let pricing = &assert_framework(&frames, "src/__tests__/pricing.test.ts", "jest");
    assert_eq!(
        rows(pricing),
        [
            case("members pay less", &[(Equality, Strong)], Runs, &[], 5),
            case(
                "tallies stay within bounds",
                &[(Truthiness, Weak), (Custom, Weak)],
                Runs,
                &["log"],
                9
            ),
        ]
    );
    let widget = &assert_framework(&frames, "src/widget.test.tsx", "vitest");
    assert_eq!(
        rows(widget),
        [case("renders a count", &[(Equality, Strong)], Runs, &[], 4)]
    );
}

const JAVASCRIPT_TEST_FILES: [&str; 7] = [
    "src/inline.js",
    "test/decide.test.js",
    "test/format.spec.js",
    "test/math.quality.test.js",
    "test/math.test.js",
    "test/properties.test.js",
    "test/report.test.js",
];

const TYPESCRIPT_TEST_FILES: [&str; 7] = [
    "src/__tests__/pricing.test.ts",
    "src/inline.ts",
    "src/math.quality.test.ts",
    "src/math.test.ts",
    "src/widget.test.tsx",
    "test/decide.test.ts",
    "test/properties.test.ts",
];

#[test]
fn files_without_test_cases_have_no_frames() {
    let files = ["javascript", "typescript"].map(|lang| {
        frames(lang)
            .files
            .iter()
            .map(|f| (f.file.clone(), f.cases.is_empty()))
            .collect::<Vec<_>>()
    });
    let expected = [JAVASCRIPT_TEST_FILES, TYPESCRIPT_TEST_FILES].map(|files| {
        files
            .iter()
            .map(|file| ((*file).to_owned(), false))
            .collect::<Vec<_>>()
    });
    assert_eq!(
        files, expected,
        "only files with test cases; pricing.js has none"
    );
}

#[test]
fn a_file_without_framework_imports_is_jest_and_unknown_matchers_are_custom() {
    let dir = tree(&[(
        "spec/cart.spec.js",
        "describe(\"cart\", () => {\n  fit(\"totals\", () => {\n    expect(total()).toSatisfy(isEven);\n  });\n  xit(\"empties\", () => {});\n});\n",
    )]);
    let frames: TestFrames = produce("javascript", dir.path());
    let cart = &assert_framework(&frames, "spec/cart.spec.js", "jest");
    assert_eq!(
        rows(cart),
        [
            case("totals", &[(Custom, Strong)], Runs, &[], 2),
            case("empties", &[], Skipped, &[], 5),
        ]
    );
}

#[test]
fn conventions_pair_sources_with_sibling_test_directory_and_in_source_tests() {
    let javascript: TestConventions = produce("javascript", &fixture("javascript"));
    for path in [
        "test/math.test.js",
        "test/format.spec.js",
        "src/__tests__/a.js",
        "packages/x/test/helpers.js",
    ] {
        assert!(javascript.is_test(path), "{path}");
    }
    assert!(!javascript.is_test("src/math.js"));
    assert!(!javascript.is_test("src/inline.js"));
    assert_eq!(
        javascript.test_files_for("src/math.js"),
        ["test/math.test.js"]
    );
    assert_eq!(
        javascript.test_files_for("src/decide.js"),
        ["test/decide.test.js"]
    );
    assert_eq!(
        javascript.test_files_for("src/inline.js"),
        ["src/inline.js"]
    );
    assert!(javascript.test_files_for("src/pricing.js").is_empty());

    let typescript: TestConventions = produce("typescript", &fixture("typescript"));
    assert_eq!(
        typescript.test_files_for("src/math.ts"),
        ["src/math.test.ts"]
    );
    assert_eq!(
        typescript.test_files_for("src/decide.ts"),
        ["test/decide.test.ts"]
    );
    assert_eq!(
        typescript.test_files_for("src/pricing.ts"),
        ["src/__tests__/pricing.test.ts"]
    );
    assert_eq!(
        typescript.test_files_for("src/widget.tsx"),
        ["src/widget.test.tsx"]
    );
    assert_eq!(
        typescript.test_files_for("src/inline.ts"),
        ["src/inline.ts"]
    );
}

#[test]
fn fast_check_assertions_make_property_cases_in_both_languages() {
    let source = r#"import fc from "fast-check";

test("digits", () => {
  fc.assert(
    fc.property(fc.integer(), (n) => {
      expect(digits(n)).toBe(String(n).length);
    }),
  );
});

it("checks", () => {
  expect(fc.check(fc.property(fc.nat(), (n) => n >= 0)).failed).toBe(false);
});

test("example", () => {
  expect(digits(7)).toBe(1);
});
"#;
    for (lang, path) in [
        ("javascript", "props.test.js"),
        ("typescript", "props.test.ts"),
    ] {
        let dir = tree(&[(path, source)]);
        assert_eq!(
            properties(lang, dir.path(), path),
            [
                property(
                    "digits",
                    Some("fast-check"),
                    &[(Custom, Strong), (Equality, Strong)]
                ),
                property("checks", Some("fast-check"), &[(Equality, Strong)]),
                property("example", None, &[(Equality, Strong)]),
            ],
            "{lang}"
        );
    }
}
