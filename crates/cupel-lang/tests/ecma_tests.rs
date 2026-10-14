// ABOUTME: The JavaScript and TypeScript TestFrames and TestConventions producers: vitest, jest, mocha with
// ABOUTME: chai, and node:test cases, assertion kinds and strengths, skips, mocks, setup, and test pairing.
mod support;

use cupel_core::syntax::{AssertionKind, AssertionStrength, TestConventions, TestFile, TestFrames};
use support::{Skip, case, file, fixture, produce, rows, tree};

use AssertionKind::{Custom, Equality, NotNull, Snapshot, Throws, Truthiness};
use AssertionStrength::{Strong, Weak};

fn frames(lang: &str) -> TestFrames {
    produce(lang, &fixture(lang))
}

fn setup_lines(file: &TestFile) -> Vec<u32> {
    file.setup.iter().map(|s| s.line).collect()
}

#[test]
fn vitest_cases_classify_expect_matchers_and_share_module_mocks() {
    let frames = frames("javascript");
    let math = file(&frames, "test/math.test.js");
    assert_eq!(math.framework, "vitest");
    assert_eq!(setup_lines(math), [10]);
    let module = "../src/report.cjs";
    assert_eq!(
        rows(math),
        [
            case(
                "adds two numbers",
                &[(Equality, Strong)],
                Skip::Runs,
                &[module],
                14
            ),
            case(
                "is truthy for non-zero sums",
                &[(Truthiness, Weak)],
                Skip::Runs,
                &[module],
                18
            ),
            case(
                "returns a defined value",
                &[(NotNull, Weak)],
                Skip::Runs,
                &[module],
                22
            ),
            case(
                "does not return zero",
                &[(Equality, Weak)],
                Skip::Runs,
                &[module],
                26
            ),
            case(
                "handles huge numbers",
                &[(Equality, Strong)],
                Skip::Skipped,
                &[module],
                30
            ),
            case(
                "can be called without checking",
                &[],
                Skip::Runs,
                &[module],
                34
            ),
            case(
                "rejects empty amounts",
                &[(Throws, Strong)],
                Skip::Runs,
                &[module],
                40
            ),
            case(
                "throws on nothing",
                &[(Throws, Weak)],
                Skip::Runs,
                &[module],
                44
            ),
            case(
                "matches the range snapshot",
                &[(Snapshot, Strong)],
                Skip::Runs,
                &[module],
                48
            ),
            case(
                "reports what it parsed",
                &[(Equality, Strong)],
                Skip::Runs,
                &[module, "vi.fn"],
                52
            ),
        ]
    );
}

#[test]
fn node_test_cases_classify_assert_calls_and_skip_with_a_reason() {
    let frames = frames("javascript");
    let decide = file(&frames, "test/decide.test.js");
    assert_eq!(decide.framework, "node-test");
    assert!(decide.setup.is_empty());
    assert_eq!(
        rows(decide),
        [
            case(
                "classifies overdrawn balances",
                &[(Equality, Strong)],
                Skip::Runs,
                &[],
                5
            ),
            case(
                "does not call an empty balance rich",
                &[(Equality, Weak)],
                Skip::Runs,
                &[],
                9
            ),
            case(
                "labels through a stub",
                &[(Truthiness, Weak)],
                Skip::Runs,
                &["mock.fn"],
                13
            ),
            case(
                "waits for the ledger server",
                &[(Equality, Strong)],
                Skip::Because("needs a ledger server".to_owned()),
                &[],
                18
            ),
        ]
    );
}

#[test]
fn jest_globals_mocha_with_chai_and_in_source_tests_are_recognized() {
    let frames = frames("javascript");
    let report = file(&frames, "test/report.test.js");
    assert_eq!(report.framework, "jest");
    let module = "../src/format.cjs";
    assert_eq!(
        rows(report),
        [
            case(
                "joins one line per entry",
                &[(Equality, Strong)],
                Skip::Runs,
                &[module],
                5
            ),
            case(
                "formats currency symbols",
                &[(Snapshot, Strong)],
                Skip::Skipped,
                &[module],
                9
            ),
        ]
    );
    let format = file(&frames, "test/format.spec.js");
    assert_eq!(format.framework, "mocha");
    assert_eq!(
        rows(format),
        [
            case(
                "shows two decimals",
                &[(Equality, Strong)],
                Skip::Runs,
                &[],
                6
            ),
            case(
                "is not called by accident",
                &[(Truthiness, Weak)],
                Skip::Runs,
                &["sinon.spy"],
                10
            ),
        ]
    );
    let inline = file(&frames, "src/inline.js");
    assert_eq!(inline.framework, "vitest");
    assert_eq!(
        rows(inline),
        [case("doubles", &[(Equality, Strong)], Skip::Runs, &[], 8)]
    );
}

#[test]
fn typescript_vitest_cases_match_their_javascript_twins() {
    let frames = frames("typescript");
    let math = file(&frames, "src/math.test.ts");
    assert_eq!(math.framework, "vitest");
    assert_eq!(setup_lines(math), [13]);
    let module = "./legacy";
    assert_eq!(
        rows(math),
        [
            case(
                "adds two numbers",
                &[(Equality, Strong)],
                Skip::Runs,
                &[module],
                7
            ),
            case(
                "is truthy for non-zero sums",
                &[(Truthiness, Weak)],
                Skip::Runs,
                &[module],
                17
            ),
            case(
                "returns a defined value",
                &[(NotNull, Weak)],
                Skip::Runs,
                &[module],
                21
            ),
            case(
                "does not return zero",
                &[(Equality, Weak)],
                Skip::Runs,
                &[module],
                25
            ),
            case(
                "handles huge numbers",
                &[(Equality, Strong)],
                Skip::Skipped,
                &[module],
                29
            ),
            case(
                "can be called without checking",
                &[],
                Skip::Runs,
                &[module],
                33
            ),
            case(
                "matches its snapshot",
                &[(Snapshot, Strong)],
                Skip::Runs,
                &[module],
                39
            ),
            case(
                "is consumed by a spy",
                &[(Equality, Strong)],
                Skip::Runs,
                &[module, "vi.fn"],
                43
            ),
        ]
    );
}

#[test]
fn typescript_node_test_jest_and_tsx_test_files_are_recognized() {
    let frames = frames("typescript");
    let decide = file(&frames, "test/decide.test.ts");
    assert_eq!(decide.framework, "node-test");
    assert_eq!(
        rows(decide),
        [
            case(
                "classifies overdrawn balances",
                &[(Equality, Strong)],
                Skip::Runs,
                &[],
                5
            ),
            case(
                "rejects empty amounts",
                &[(Throws, Strong)],
                Skip::Runs,
                &[],
                9
            ),
            case(
                "waits for the ledger server",
                &[(Truthiness, Weak)],
                Skip::Because("needs a ledger server".to_owned()),
                &[],
                13
            ),
        ]
    );
    let pricing = file(&frames, "src/__tests__/pricing.test.ts");
    assert_eq!(pricing.framework, "jest");
    assert_eq!(
        rows(pricing),
        [
            case(
                "members pay less",
                &[(Equality, Strong)],
                Skip::Runs,
                &[],
                5
            ),
            case(
                "tallies stay within bounds",
                &[(Truthiness, Weak), (Custom, Weak)],
                Skip::Runs,
                &["log"],
                9
            ),
        ]
    );
    let widget = file(&frames, "src/widget.test.tsx");
    assert_eq!(widget.framework, "vitest");
    assert_eq!(
        rows(widget),
        [case(
            "renders a count",
            &[(Equality, Strong)],
            Skip::Runs,
            &[],
            4
        )]
    );
}

#[test]
fn files_without_test_cases_have_no_frames() {
    for lang in ["javascript", "typescript"] {
        let frames = frames(lang);
        assert!(frames.files.iter().all(|f| !f.cases.is_empty()));
        assert!(frames.files.iter().all(|f| !f.file.contains("pricing.js")));
    }
}

#[test]
fn a_file_without_framework_imports_is_jest_and_unknown_matchers_are_custom() {
    let dir = tree(&[(
        "spec/cart.spec.js",
        "describe(\"cart\", () => {\n  fit(\"totals\", () => {\n    expect(total()).toSatisfy(isEven);\n  });\n  xit(\"empties\", () => {});\n});\n",
    )]);
    let frames: TestFrames = produce("javascript", dir.path());
    let cart = file(&frames, "spec/cart.spec.js");
    assert_eq!(cart.framework, "jest");
    assert_eq!(
        rows(cart),
        [
            case("totals", &[(Custom, Strong)], Skip::Runs, &[], 2),
            case("empties", &[], Skip::Skipped, &[], 5),
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
