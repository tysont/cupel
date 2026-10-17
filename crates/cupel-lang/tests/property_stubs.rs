// ABOUTME: Property stubs from signatures (ADR-096 §4): each language's `[property_stubs]` table rendered for a function,
// ABOUTME: where the stub goes, its generators by parameter type, and how stubs merge into a file once each.
use cupel_lang::{LanguageRegistry, Stub, Subject, merged_stubs, property_stub};

fn stub(
    lang: &str,
    subject: &Subject<'_>,
    nearest: Option<&str>,
    files: &[(&str, &str)],
) -> Option<Stub> {
    let languages = LanguageRegistry::builtin();
    let language = languages.get(&lang.into()).unwrap();
    let read = |path: &str| {
        files
            .iter()
            .find(|(file, _)| *file == path)
            .map(|(_, text)| (*text).to_owned())
    };
    property_stub(language.as_ref(), subject, nearest, &read)
}

fn subject<'a>(
    source: &'a str,
    name: &'a str,
    namespace: &'a str,
    signature: Option<&'a str>,
    parameters: u32,
) -> Subject<'a> {
    Subject {
        source,
        name,
        namespace,
        parameters,
        signature,
    }
}

#[test]
fn a_python_stub_joins_the_nearest_test_file_with_a_strategy_per_argument() {
    let stub = stub(
        "python",
        &subject(
            "src/units/percent.py",
            "clamp_percent",
            "units.percent",
            Some("(_)"),
            1,
        ),
        Some("tests/test_percent.py"),
        &[],
    )
    .unwrap();
    assert_eq!(stub.target, "tests/test_percent.py");
    assert_eq!(
        stub.header,
        [
            "import pytest",
            "from hypothesis import given, strategies as st",
            "import units.percent",
        ]
    );
    assert_eq!(
        stub.text,
        "\n\n@pytest.mark.skip(reason=\"state the property clamp_percent keeps, then remove this mark\")\n\
         @given(st.from_type(object))\n\
         def test_clamp_percent_property(a0):\n    units.percent.clamp_percent(a0)\n"
    );
}

#[test]
fn without_a_test_file_a_python_stub_starts_one_beside_its_source() {
    let stub = stub(
        "python",
        &subject("src/units/lone.py", "lone", "units.lone", None, 2),
        None,
        &[],
    )
    .unwrap();
    assert_eq!(stub.target, "src/units/test_lone.py");
    assert!(
        stub.text
            .contains("@given(st.from_type(object), st.from_type(object))")
    );
    assert!(stub.text.contains("def test_lone_property(a0, a1):"));
}

#[test]
fn a_rust_stub_is_an_inline_test_module_with_a_strategy_per_parameter_type() {
    let stub = stub(
        "rust",
        &subject(
            "src/percent.rs",
            "clamp_percent",
            "percent",
            Some("(_: i64, _: &str) -> i64"),
            2,
        ),
        Some("tests/percent.rs"),
        &[],
    )
    .unwrap();
    assert_eq!(stub.target, "src/percent.rs");
    assert!(stub.header.is_empty());
    assert_eq!(
        stub.text,
        "\n#[cfg(test)]\nmod clamp_percent_properties {\n    use super::*;\n    use proptest::prelude::*;\n\n    \
         proptest! {\n        #[test]\n        #[ignore = \"state the property `clamp_percent` keeps, then remove this attribute\"]\n        \
         fn clamp_percent_property(a0 in any::<i64>(), a1 in any::<String>()) {\n            \
         let _ = clamp_percent(a0, &a1);\n        }\n    }\n}\n"
    );
}

#[test]
fn a_rust_function_with_a_parameter_type_the_table_does_not_map_gets_no_stub() {
    let lacking = stub(
        "rust",
        &subject(
            "src/account.rs",
            "settle",
            "account",
            Some("(_: &mut Ledger, _: u32)"),
            2,
        ),
        None,
        &[],
    );
    assert!(lacking.is_none());
    let unknown = stub("rust", &subject("src/a.rs", "f", "a", None, 1), None, &[]);
    assert!(unknown.is_none(), "an unknown type has no generator");
}

#[test]
fn an_ecma_stub_imports_as_an_es_module_and_types_its_arbitraries() {
    let esm = stub(
        "typescript",
        &subject(
            "src/percent.ts",
            "clampPercent",
            "percent",
            Some("(_: number): number"),
            1,
        ),
        Some("src/percent.test.ts"),
        &[(
            "src/percent.test.ts",
            "import { clampPercent } from \"./percent\";\n",
        )],
    )
    .unwrap();
    assert_eq!(esm.target, "src/percent.test.ts");
    assert_eq!(
        esm.header,
        [
            "import fc from \"fast-check\";",
            "import * as percentSubject from \"./percent\";"
        ]
    );
    assert_eq!(
        esm.text,
        "\ntest.skip(\"clampPercent keeps its property\", () => {\n  fc.assert(\n    \
         fc.property(fc.double(), (a0) => {\n      percentSubject.clampPercent(a0);\n    }),\n  );\n});\n"
    );
}

#[test]
fn an_ecma_stub_requires_in_a_commonjs_file_and_starts_a_test_file_where_none_is() {
    let commonjs = stub(
        "javascript",
        &subject("percent.js", "clampPercent", "percent", Some("(_)"), 1),
        Some("percent.test.js"),
        &[(
            "percent.test.js",
            "const { clampPercent } = require(\"./percent\");\n",
        )],
    )
    .unwrap();
    assert_eq!(
        commonjs.header,
        [
            "const fc = require(\"fast-check\");",
            "const percentSubject = require(\"./percent.js\");"
        ]
    );
    assert!(
        commonjs
            .text
            .contains("fc.property(fc.anything(), (a0) => {")
    );
    let fresh = stub(
        "javascript",
        &subject("lib/digits.js", "digits", "lib.digits", Some("(_)"), 1),
        None,
        &[("lib/digits.js", "export function digits(n) {}\n")],
    )
    .unwrap();
    assert_eq!(fresh.target, "lib/digits.test.js");
    assert_eq!(
        fresh.header[1],
        "import * as digitsSubject from \"./digits.js\";"
    );
}

#[test]
fn stubs_merge_into_a_file_once_with_each_header_line_once() {
    let one = stub(
        "python",
        &subject(
            "src/units/percent.py",
            "clamp_percent",
            "units.percent",
            Some("(_)"),
            1,
        ),
        Some("tests/test_percent.py"),
        &[],
    )
    .unwrap();
    let two = stub(
        "python",
        &subject(
            "src/units/percent.py",
            "scale",
            "units.percent",
            Some("(_, _)"),
            2,
        ),
        Some("tests/test_percent.py"),
        &[],
    )
    .unwrap();
    let existing = "import pytest\n\n\ndef test_clamps():\n    assert True\n";
    let text = merged_stubs(Some(existing), &[&one, &two]).unwrap();
    assert!(text.starts_with(existing));
    assert_eq!(text.matches("import pytest").count(), 1);
    assert_eq!(text.matches("import units.percent").count(), 1);
    assert!(text.contains("def test_clamp_percent_property(a0):"));
    assert!(text.contains("def test_scale_property(a0, a1):"));
    assert_eq!(
        merged_stubs(Some(&text), &[&one, &two]),
        None,
        "nothing new to add"
    );
    let created = merged_stubs(None, &[&one]).unwrap();
    assert!(created.starts_with("import pytest\nfrom hypothesis import given, strategies as st\n"));
}
