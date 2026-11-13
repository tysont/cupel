// ABOUTME: The JavaScript and TypeScript TestFrames and TestConventions producers: vitest, jest, mocha with
// ABOUTME: chai, and node:test cases, assertion kinds and strengths, skips, mocks, setup, and test pairing.
mod support;

use cupel_core::syntax::{AssertionKind, AssertionStrength, TestConventions, TestFile, TestFrames};
use support::Skip::{Because, Runs, Skipped};
use support::{
    Expected, assert_framework, assert_source_file, assert_test_file, case, expected, fixture,
    produce, properties, property, rows, tree,
};

use AssertionKind::{Custom, Equality, NotNull, Snapshot, Throws, Truthiness};
use AssertionStrength::{Strong, Weak};

fn frames(lang: &str) -> TestFrames {
    produce(lang, &fixture(lang))
}

/// The module `math.test.js` mocks, at module level: the file's mock, not any one case's (ADR-164).
const MODULE: &str = "../src/report.cjs";

const MATH_CASES: &[Expected] = &[
    ("adds two numbers", &[(Equality, Strong)], None, &[], 14),
    (
        "is truthy for non-zero sums",
        &[(Truthiness, Weak)],
        None,
        &[],
        18,
    ),
    ("returns a defined value", &[(NotNull, Weak)], None, &[], 22),
    ("does not return zero", &[(Equality, Weak)], None, &[], 26),
    (
        "handles huge numbers",
        &[(Equality, Strong)],
        Some(None),
        &[],
        30,
    ),
    ("can be called without checking", &[], None, &[], 34),
    ("rejects empty amounts", &[(Throws, Strong)], None, &[], 40),
    ("throws on nothing", &[(Throws, Weak)], None, &[], 44),
    (
        "matches the range snapshot",
        &[(Snapshot, Strong)],
        None,
        &[],
        48,
    ),
    (
        "reports what it parsed",
        &[(Equality, Strong)],
        None,
        &["vi.fn"],
        53,
    ),
];

/// What the mocks outside every case of `file` replace, as written.
fn file_mocks(file: &TestFile) -> Vec<&str> {
    file.mocks.iter().map(|mock| mock.target.as_str()).collect()
}

#[test]
fn vitest_cases_classify_expect_matchers_and_a_module_mock_is_the_files() {
    let math = &assert_test_file("javascript", "test/math.test.js", "vitest", &[10]);
    assert_eq!(rows(math), expected(MATH_CASES));
    assert_eq!(file_mocks(math), [MODULE]);
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
                &[],
                5
            ),
            case(
                "formats currency symbols",
                &[(Snapshot, Strong)],
                Skipped,
                &[],
                9
            ),
        ]
    );
    assert_eq!(file_mocks(report), [module]);
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
            case("adds two numbers", &[(Equality, Strong)], Runs, &[], 7),
            case(
                "is truthy for non-zero sums",
                &[(Truthiness, Weak)],
                Runs,
                &[],
                17
            ),
            case("returns a defined value", &[(NotNull, Weak)], Runs, &[], 21),
            case("does not return zero", &[(Equality, Weak)], Runs, &[], 25),
            case(
                "handles huge numbers",
                &[(Equality, Strong)],
                Skipped,
                &[],
                29
            ),
            case("can be called without checking", &[], Runs, &[], 33),
            case("matches its snapshot", &[(Snapshot, Strong)], Runs, &[], 39),
            case(
                "is consumed by a spy",
                &[(Equality, Strong)],
                Runs,
                &["vi.fn"],
                44
            ),
        ]
    );
    assert_eq!(file_mocks(math), [module]);
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

/// A type-level test of a TypeScript library (ts-pattern's idiom, P9.9 group 3): `Expect<Equal<A, B>>`
/// fails the file's type check unless the two types are equal, and `// @ts-expect-error` fails it
/// unless the next line is a type error; a runner that type-checks (ts-jest) or the project's `tsc`
/// enforces both, so each is an assertion of the case that holds it.
const TYPE_ASSERTIONS: &str = r"type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B ? 1 : 2 ? true : false;
type Expect<T extends true> = T;

describe('narrow', () => {
  it('infers the input', () => {
    const value = parse('1');
    type t = Expect<Equal<typeof value, number>>;
  });

  it('infers every case', () => {
    const handler = (x: number | string) => {
      type cases = [Expect<Equal<typeof x, number | string>>, Expect<IsUnion<typeof x>>];
      return x;
    };
  });

  it('rejects a string', () => {
    // @ts-expect-error
    parse(1);
  });

  it('runs exhaustively', () => {
    parse('2');
  });
});
";

#[test]
fn type_level_assertions_assert_in_typescript_tests() {
    let dir = tree(&[("tests/narrow.test.ts", TYPE_ASSERTIONS)]);
    let frames: TestFrames = produce("typescript", dir.path());
    let narrow = &assert_framework(&frames, "tests/narrow.test.ts", "jest");
    assert_eq!(
        rows(narrow),
        [
            case("infers the input", &[(Equality, Strong)], Runs, &[], 5),
            case(
                "infers every case",
                &[(Equality, Strong), (Truthiness, Weak)],
                Runs,
                &[],
                10
            ),
            case("rejects a string", &[(Throws, Weak)], Runs, &[], 17),
            case("runs exhaustively", &[], Runs, &[], 22),
        ]
    );
}

#[test]
fn a_case_that_calls_an_asserting_helper_of_its_file_asserts_through_it() {
    let source = r#"import { expect, it } from "vitest";

const before = (text, first, second) => {
  expect(text.indexOf(first)).toBeLessThan(text.indexOf(second));
};

function same(actual, expected) {
  expect(actual).toEqual(expected);
}

it("checks before encrypting", () => {
  before("check then encrypt", "check", "encrypt");
});

it("round-trips", () => {
  same(1, 1);
});

it("asserts nothing", () => {
  const x = 1;
});
"#;
    let file = assert_source_file("javascript", "a.test.js", source, "vitest");
    assert_eq!(
        rows(&file),
        [
            case("checks before encrypting", &[(Custom, Weak)], Runs, &[], 11),
            case("round-trips", &[(Equality, Strong)], Runs, &[], 15),
            case("asserts nothing", &[], Runs, &[], 19),
        ]
    );
}

#[test]
fn membership_pattern_and_partial_object_matchers_are_strong() {
    let source = r#"import { expect, it } from "vitest";

it("lists the entry", () => {
  expect(["a", "b"]).toContain("a");
});

it("names the version", () => {
  expect("v1.2").toMatch(/v1/);
});

it("keeps the id", () => {
  expect({ id: 1, name: "a" }).toMatchObject({ id: 1 });
});

it("holds an equal entry", () => {
  expect([{ id: 1 }]).toContainEqual({ id: 1 });
});
"#;
    let file = assert_source_file("javascript", "a.test.js", source, "vitest");
    assert_eq!(
        rows(&file),
        [
            case("lists the entry", &[(Custom, Strong)], Runs, &[], 3),
            case("names the version", &[(Custom, Strong)], Runs, &[], 7),
            case("keeps the id", &[(Equality, Strong)], Runs, &[], 11),
            case("holds an equal entry", &[(Custom, Weak)], Runs, &[], 15),
        ]
    );
}

#[test]
fn a_playwright_suite_step_and_hook_are_not_cases() {
    let source = r#"import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.goto("/");
});

test.describe("settings page", () => {
  test("loads its form", async ({ page }) => {
    await test.step("opens", async () => {
      await expect(page).toHaveTitle("Settings");
    });
    await expect(page.locator("form")).toBeVisible();
  });
});
"#;
    let file = assert_source_file("typescript", "e2e/settings.spec.ts", source, "jest");
    let names: Vec<&str> = file.cases.iter().map(|case| case.name.as_str()).collect();
    assert_eq!(names, ["loads its form"]);
    assert_eq!(file.cases[0].assertions.len(), 2);
}
