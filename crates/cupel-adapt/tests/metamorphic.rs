// ABOUTME: The metamorphic engine (ADR-097): inputs generated from the seed, minimization candidates, availability, and
// ABOUTME: relations evaluated by the real Python harness on modules written for each test (skipped without python3).
mod support;

use std::path::Path;

use cupel_adapt::metamorphic::{DEFAULT_CASES, candidates, generate, relation_seed};
use cupel_core::{
    Availability, Cancellation, Ctx, Error, Evaluated, FactKind, InputType, LangId, ProduceCtx,
    RegisteredProducer, RelationResult, RelationResults, Scope, Writer, find_program,
};
use proptest::prelude::*;
use serde_json::{Value, json};

const ID: &str = "engine.metamorphic";

fn input_type(text: &str) -> InputType {
    text.parse().unwrap()
}

/// Whether `value` is a value of `ty`.
fn conforms(ty: &InputType, value: &Value) -> bool {
    match (ty, value) {
        (InputType::Bool, Value::Bool(_)) | (InputType::Str, Value::String(_)) => true,
        (InputType::Int, Value::Number(number)) => number.is_i64(),
        (InputType::List(element), Value::Array(items)) => {
            items.iter().all(|item| conforms(element, item))
        }
        (InputType::Tuple(elements), Value::Array(items)) => {
            elements.len() == items.len()
                && elements
                    .iter()
                    .zip(items)
                    .all(|(ty, item)| conforms(ty, item))
        }
        _ => false,
    }
}

/// Input types as written in `cupel.toml`, nested up to three deep, parsed.
fn types() -> impl Strategy<Value = InputType> {
    let leaf = prop::sample::select(vec!["bool", "int", "str"]).prop_map(str::to_owned);
    let written = leaf.prop_recursive(3, 12, 3, |inner| {
        prop_oneof![
            inner.clone().prop_map(|element| format!("list[{element}]")),
            prop::collection::vec(inner, 1..4)
                .prop_map(|elements| format!("tuple[{}]", elements.join(", "))),
        ]
    });
    written.prop_map(|text| input_type(&text))
}

proptest! {
    #[test]
    fn generated_inputs_are_of_their_type_and_follow_the_seed_alone(
        ty in types(), seed in any::<u64>(), case in 0u64..400,
    ) {
        let value = generate(&ty, seed, case);
        prop_assert!(conforms(&ty, &value), "{value} is not a {ty}");
        prop_assert_eq!(generate(&ty, seed, case), value);
    }

    #[test]
    fn every_candidate_is_of_the_type_and_differs_from_the_input(
        ty in types(), seed in any::<u64>(), case in 0u64..100,
    ) {
        let value = generate(&ty, seed, case);
        let candidates = candidates(&ty, &value);
        for candidate in &candidates {
            prop_assert!(conforms(&ty, candidate), "{candidate} is not a {ty}");
            prop_assert!(candidate != &value, "{value} is its own candidate");
        }
        let distinct: std::collections::BTreeSet<String> =
            candidates.iter().map(Value::to_string).collect();
        prop_assert_eq!(distinct.len(), candidates.len());
    }
}

#[test]
fn early_cases_are_small_and_later_ones_grow() {
    let ty = input_type("list[int]");
    let longest = |cases: std::ops::Range<u64>| {
        cases
            .map(|case| generate(&ty, 9, case).as_array().unwrap().len())
            .max()
            .unwrap()
    };
    assert_eq!(
        longest(0..4).max(1),
        1,
        "the first four cases are of size one"
    );
    assert_eq!(longest(0..400), 8, "cases grow to size eight");
    let triples = input_type("tuple[int, int, int]");
    let seeds: std::collections::BTreeSet<String> = (0..8)
        .map(|seed| generate(&triples, seed, 50).to_string())
        .collect();
    assert_eq!(seeds.len(), 8, "each seed draws its own lists: {seeds:?}");
}

#[test]
fn candidates_shrink_toward_the_simplest_value_of_the_type() {
    let shrunk = |ty: &str, value: Value| candidates(&input_type(ty), &value);
    assert_eq!(shrunk("int", json!(0)), Vec::<Value>::new());
    assert_eq!(shrunk("int", json!(9)), [json!(0), json!(4), json!(8)]);
    assert_eq!(
        shrunk("int", json!(-9)),
        [json!(0), json!(9), json!(-4), json!(-8)]
    );
    assert_eq!(shrunk("bool", json!(true)), [json!(false)]);
    assert_eq!(shrunk("bool", json!(false)), Vec::<Value>::new());
    assert_eq!(shrunk("str", json!("")), Vec::<Value>::new());
    assert_eq!(
        shrunk("str", json!("ba")),
        [json!(""), json!("a"), json!("b"), json!("aa")]
    );
    assert_eq!(shrunk("list[int]", json!([])), Vec::<Value>::new());
    assert_eq!(
        shrunk("list[int]", json!([3, 0])),
        [
            json!([]),
            json!([0]),
            json!([3]),
            json!([0, 0]),
            json!([1, 0]),
            json!([2, 0])
        ]
    );
    assert_eq!(
        shrunk("tuple[int, bool]", json!([2, true])),
        [json!([0, true]), json!([1, true]), json!([2, false])]
    );
}

#[test]
fn each_relation_draws_its_own_inputs_from_the_runs_seed() {
    assert_eq!(relation_seed(5, "a"), relation_seed(5, "a"));
    assert_ne!(relation_seed(5, "a"), relation_seed(5, "b"));
    assert_ne!(relation_seed(5, "a"), relation_seed(6, "a"));
}

// ---------------------------------------------------------------- the engine

fn engine() -> &'static dyn RegisteredProducer {
    static REGISTRY: std::sync::OnceLock<cupel_core::ProducerRegistry> = std::sync::OnceLock::new();
    let registry = REGISTRY.get_or_init(support::registry);
    support::producer(registry, FactKind::RelationResults, ID, "python")
}

fn python_installed() -> bool {
    let installed = find_program("python3").is_some();
    if !installed {
        eprintln!("SKIPPED: python3 not installed; the metamorphic harness was not run");
    }
    installed
}

/// The context of producing in `root` with `seed` and `relations` configured for Python.
fn produce_ctx(root: &Path, seed: u64, relations: Value) -> ProduceCtx {
    let table = serde_json::Map::from_iter([("relations".to_owned(), relations)]);
    let mut ctx = ProduceCtx::new(
        Ctx {
            lang: [(LangId::from("python"), table.clone())].into(),
            ..Ctx::new(root.to_path_buf())
        },
        Writer::new(root, false).unwrap().confined(),
        Cancellation::new(),
    );
    ctx.config = table;
    ctx.seed = seed;
    ctx
}

fn produce(ctx: &ProduceCtx) -> cupel_core::Result<RelationResults> {
    engine()
        .produce(ctx, &Scope::default(), &mut Vec::new())
        .map(|fact| fact.downcast::<RelationResults>().unwrap())
}

/// A repository holding `sorting.py`: a sort that makes only one bubble pass, and an `add`.
fn repository() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("sorting.py"),
        "def sort_list(xs):\n\
         \x20   result = list(xs)\n\
         \x20   for i in range(len(result) - 1):\n\
         \x20       if result[i] > result[i + 1]:\n\
         \x20           result[i], result[i + 1] = result[i + 1], result[i]\n\
         \x20   return result\n\
         \n\
         \n\
         def add(a, b):\n\
         \x20   print('adding', a, b)\n\
         \x20   return a + b\n\
         \n\
         \n\
         def inverse(x):\n\
         \x20   return 1 // x\n",
    )
    .unwrap();
    dir
}

fn sort_reverse() -> Value {
    json!({
        "function": "sorting:sort_list",
        "input": "list[int]",
        "transform": "lambda xs: list(reversed(xs))",
    })
}

/// The result of relation `name`; relations are evaluated in name order.
fn named<'a>(results: &'a RelationResults, name: &str) -> &'a RelationResult {
    let names: Vec<&str> = results.relations.iter().map(|r| r.name.as_str()).collect();
    let index = names
        .binary_search(&name)
        .expect("the relation was evaluated");
    &results.relations[index]
}

#[test]
fn the_engine_is_unconfigured_without_relations_and_ready_with_them() {
    let dir = repository();
    let bare = Ctx::new(dir.path().to_path_buf());
    assert_eq!(
        engine().availability(&bare),
        Availability::Unconfigured {
            key: "lang.python.relations".to_owned()
        }
    );
    let ctx = produce_ctx(dir.path(), 1, json!({ "sort-reverse": sort_reverse() }));
    if python_installed() {
        assert_eq!(engine().availability(&ctx.ctx), Availability::Ready);
    }
    assert_eq!(engine().needs(), &[] as &[FactKind]);
    assert_eq!(engine().cost(), cupel_core::CostClass::Deep);
}

#[test]
fn a_violated_relation_is_minimized_and_a_held_one_runs_every_case() {
    if !python_installed() {
        return;
    }
    let dir = repository();
    let relations = json!({
        "sort-reverse": sort_reverse(),
        "add-commutes": {
            "function": "sorting:add",
            "input": "tuple[int, int]",
            "transform": "lambda pair: (pair[1], pair[0])",
            "cases": 40,
        },
    });
    let results = produce(&produce_ctx(dir.path(), 3, relations)).unwrap();

    assert_eq!(results.seed, 3);
    let held = named(&results, "add-commutes");
    assert_eq!((held.cases, held.violation.is_none()), (40, true));
    assert_eq!(held.input, "tuple[int, int]");
    let sort = named(&results, "sort-reverse");
    let violation = sort
        .violation
        .as_ref()
        .expect("one bubble pass does not sort");
    assert!(sort.cases <= DEFAULT_CASES && sort.cases == violation.case + 1);
    let input = violation.input.as_array().unwrap();
    assert_eq!(input.len(), 3, "a minimal unsortable list: {input:?}");
    assert!(violation.shrinks > 0 || violation.original == violation.input);
    let Evaluated::Value(transformed) = &violation.transformed else {
        panic!("{:?}", violation.transformed)
    };
    let reversed: Vec<Value> = input.iter().rev().cloned().collect();
    assert_eq!(transformed, &Value::Array(reversed));
    assert_ne!(Some(&violation.source), violation.followup.as_ref());
    let location = sort.location.as_ref().unwrap();
    assert_eq!(
        (location.file.as_str(), location.line, location.end_line),
        ("sorting.py", 1, 6)
    );
}

#[test]
fn the_same_seed_repeats_every_input_and_the_case_budget_caps_them() {
    if !python_installed() {
        return;
    }
    let dir = repository();
    let relations = json!({ "sort-reverse": sort_reverse() });
    let first = produce(&produce_ctx(dir.path(), 11, relations.clone())).unwrap();
    let again = produce(&produce_ctx(dir.path(), 11, relations.clone())).unwrap();
    assert_eq!(first, again);

    let mut capped = produce_ctx(dir.path(), 11, relations);
    capped.focus.cases = Some(2);
    let results = produce(&capped).unwrap();
    let sort = named(&results, "sort-reverse");
    assert_eq!((sort.cases, sort.violation.is_none()), (2, true));
}

#[test]
fn an_exception_is_a_violation_with_what_it_raised() {
    if !python_installed() {
        return;
    }
    let dir = repository();
    let relations = json!({
        "inverse-negates": {
            "function": "sorting:inverse",
            "input": "int",
            "transform": "lambda x: -x",
            "relation": "lambda a, b: a == -b",
        },
    });
    let results = produce(&produce_ctx(dir.path(), 2, relations)).unwrap();
    let violation = named(&results, "inverse-negates")
        .violation
        .clone()
        .unwrap();
    assert_eq!(
        (violation.input.clone(), violation.source.clone()),
        (
            json!(0),
            Evaluated::Raised("ZeroDivisionError: integer division or modulo by zero".to_owned())
        ),
        "{violation:?}"
    );
}

#[test]
fn a_relation_the_harness_cannot_set_up_is_an_error_naming_it() {
    if !python_installed() {
        return;
    }
    let dir = repository();
    let cases = [
        (
            json!("nowhere:sort_list"),
            "lambda xs: xs",
            "cannot import `nowhere`",
        ),
        (
            json!("sorting:missing"),
            "lambda xs: xs",
            "`sorting` has no `missing`",
        ),
        (
            json!("sorting:sort_list"),
            "lambda xs:",
            "`transform` does not evaluate",
        ),
    ];
    for (function, transform, expected) in cases {
        let relations = json!({
            "broken": { "function": function, "input": "int", "transform": transform },
        });
        let error = produce(&produce_ctx(dir.path(), 1, relations)).unwrap_err();
        let Error::Process { program, message } = &error else {
            panic!("{error:?}")
        };
        assert_eq!(program, "python3");
        assert!(
            message.starts_with("relation `broken`: ") && message.contains(expected),
            "{message}"
        );
    }
}

#[test]
fn the_harness_writes_nothing_into_the_repository() {
    if !python_installed() {
        return;
    }
    let dir = repository();
    produce(&produce_ctx(
        dir.path(),
        1,
        json!({ "sort-reverse": sort_reverse() }),
    ))
    .unwrap();
    let names: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["sorting.py"]);
}
