// ABOUTME: Tests for metamorphic relations (ADR-097): `[lang.<id>.relations.<name>]` checked when `cupel.toml`
// ABOUTME: loads, the input-type grammar and its round trip, and the `relation-results` fact kind and schema.
use std::path::Path;

use cupel_core::config::ConfigFile;
use cupel_core::{
    Evaluated, FactKind, InputType, LangId, RelationResult, RelationResults, Violation, relations,
    schema,
};
use proptest::prelude::*;
use serde_json::json;

fn parse(text: &str) -> cupel_core::Result<ConfigFile> {
    ConfigFile::parse(text, Path::new("cupel.toml"))
}

/// The last line of the message `text` is rejected with.
fn problem(text: &str) -> String {
    let message = parse(text)
        .expect_err("the file should be rejected")
        .to_string();
    message.lines().last().unwrap_or_default().trim().to_owned()
}

const SORTED: &str = r#"
[lang.python]
runner = "pytest"

[lang.python.relations.sort-reverse]
function = "sorting:sort_list"
input = "list[int]"
transform = "lambda xs: list(reversed(xs))"

[lang.python.relations.add-commutes]
function = "arith:add"
input = "tuple[int, int]"
transform = "lambda pair: (pair[1], pair[0])"
relation = "lambda a, b: a == b"
cases = 50
"#;

#[test]
fn relations_are_read_from_a_language_table_by_name() {
    let file = parse(SORTED).unwrap();
    let table = &file.lang[&LangId::from("python")];
    let relations = relations(&LangId::from("python"), table).unwrap();

    let names: Vec<&str> = relations.keys().map(String::as_str).collect();
    assert_eq!(names, ["add-commutes", "sort-reverse"]);
    let sort = &relations["sort-reverse"];
    assert_eq!(sort.function, "sorting:sort_list");
    assert_eq!(sort.input.to_string(), "list[int]");
    assert_eq!(sort.transform, "lambda xs: list(reversed(xs))");
    assert_eq!((sort.relation.as_deref(), sort.cases), (None, None));
    let add = &relations["add-commutes"];
    assert_eq!(add.input.to_string(), "tuple[int, int]");
    assert_eq!(add.relation.as_deref(), Some("lambda a, b: a == b"));
    assert_eq!(add.cases, Some(50));
}

#[test]
fn a_language_table_without_relations_has_none() {
    let file = parse("[lang.python]\nrunner = \"pytest\"\n").unwrap();
    let python = LangId::from("python");
    assert!(relations(&python, &file.lang[&python]).unwrap().is_empty());
}

#[test]
fn malformed_relations_are_usage_errors_naming_the_table() {
    let at = "[lang.python.relations.sorted]\n";
    let whole = "function = \"m:f\"\ninput = \"int\"\ntransform = \"abs\"\n";
    let cases = [
        (
            "[lang.python]\nrelations = 3\n".to_owned(),
            "invalid `[lang.python.relations]`: expected a table of relations by name",
        ),
        (
            format!("[lang.python.relations.Sorted]\n{whole}"),
            "invalid relation name `Sorted` in `[lang.python.relations]`: must match `[a-z][a-z0-9_-]*`",
        ),
        (
            format!("{at}input = \"int\"\ntransform = \"abs\"\n"),
            "invalid relation `sorted` in `[lang.python.relations]`: needs `function`",
        ),
        (
            format!("{at}function = \"m:f\"\ninput = \"int\"\n"),
            "invalid relation `sorted` in `[lang.python.relations]`: needs `transform`",
        ),
        (
            format!("{at}{whole}expect = \"equal\"\n"),
            "invalid relation `sorted` in `[lang.python.relations]`: unknown key `expect`; \
             expected one of `function`, `input`, `transform`, `relation`, `cases`",
        ),
        (
            format!("{at}function = 3\ninput = \"int\"\ntransform = \"abs\"\n"),
            "invalid relation `sorted` in `[lang.python.relations]`: `function` must be a \
             non-empty string",
        ),
        (
            format!("{at}function = \"m:f\"\ninput = \"float\"\ntransform = \"abs\"\n"),
            "invalid relation `sorted` in `[lang.python.relations]`: invalid input type `float`: \
             expected `bool`, `int`, `str`, `list[T]`, or `tuple[T, ...]`",
        ),
        (
            format!("{at}{whole}cases = 0\n"),
            "invalid relation `sorted` in `[lang.python.relations]`: `cases` must be a positive \
             integer",
        ),
    ];
    let problems: Vec<String> = cases.iter().map(|(text, _)| problem(text)).collect();
    let expected: Vec<&str> = cases.iter().map(|(_, problem)| *problem).collect();
    assert_eq!(problems, expected);
}

#[test]
fn input_types_parse_with_any_spacing_and_print_canonically() {
    let spelled = [
        ("bool", "bool"),
        ("int", "int"),
        (" str ", "str"),
        ("list[ int ]", "list[int]"),
        ("tuple[int,list[str]]", "tuple[int, list[str]]"),
        ("list[tuple[bool, int, str]]", "list[tuple[bool, int, str]]"),
    ];
    for (text, canonical) in spelled {
        assert_eq!(
            text.parse::<InputType>().map(|t| t.to_string()),
            Ok(canonical.to_owned()),
            "{text}"
        );
    }
    for bad in ["", "list", "list[]", "list[int", "tuple[]", "int]", "list[int]x", "dict"] {
        assert!(bad.parse::<InputType>().is_err(), "{bad} should not parse");
    }
}

#[test]
fn only_a_tuple_spreads_into_arguments() {
    let spreads = |text: &str| text.parse::<InputType>().unwrap().spreads();
    assert!(spreads("tuple[int, int]"));
    assert!(!spreads("list[int]"));
    assert!(!spreads("int"));
}

fn input_type() -> impl Strategy<Value = InputType> {
    let leaf = prop_oneof![
        Just(InputType::Bool),
        Just(InputType::Int),
        Just(InputType::Str)
    ];
    leaf.prop_recursive(3, 12, 3, |inner| {
        prop_oneof![
            inner.clone().prop_map(|t| InputType::List(Box::new(t))),
            prop::collection::vec(inner, 1..4).prop_map(InputType::Tuple),
        ]
    })
}

proptest! {
    #[test]
    fn every_input_type_parses_back_from_its_printed_form(input in input_type()) {
        prop_assert_eq!(input.to_string().parse::<InputType>(), Ok(input));
    }
}

#[test]
fn relation_results_is_a_fact_kind_with_a_versioned_schema() {
    assert_eq!(FactKind::RelationResults.as_str(), "relation-results");
    assert!(FactKind::ALL.contains(&FactKind::RelationResults));
    assert!(!FactKind::SYNTACTIC.contains(&FactKind::RelationResults));
    assert_eq!(schema::fact_version(FactKind::RelationResults), Some(1));
}

#[test]
fn a_violation_serializes_each_evaluation_by_how_it_ended() {
    let results = RelationResults {
        seed: 7,
        relations: vec![RelationResult {
            name: "sort-reverse".to_owned(),
            function: "sorting:sort_list".to_owned(),
            input: "list[int]".to_owned(),
            cases: 3,
            location: None,
            violation: Some(Violation {
                case: 2,
                input: json!([1, 0]),
                original: json!([5, -3, 0]),
                shrinks: 4,
                transformed: Evaluated::Value(json!([0, 1])),
                source: Evaluated::Value(json!([1, 0])),
                followup: Some(Evaluated::Raised("ValueError: unsorted".to_owned())),
                relation_raised: None,
            }),
        }],
    };
    let value = serde_json::to_value(&results).unwrap();
    assert_eq!(
        value["relations"][0]["violation"],
        json!({
            "case": 2,
            "input": [1, 0],
            "original": [5, -3, 0],
            "shrinks": 4,
            "transformed": {"value": [0, 1]},
            "source": {"value": [1, 0]},
            "followup": {"raised": "ValueError: unsorted"},
            "relation_raised": null,
        })
    );
    let back: RelationResults = serde_json::from_value(value).unwrap();
    assert_eq!(back, results);
    let repr: Evaluated = serde_json::from_value(json!({"repr": "<object>"})).unwrap();
    assert_eq!(repr, Evaluated::Repr("<object>".to_owned()));
}
