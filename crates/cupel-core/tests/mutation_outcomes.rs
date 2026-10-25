// ABOUTME: Tests for the `MutationOutcomes` payload (ADR-080, ADR-082): its fact kind and schema, which outcomes
// ABOUTME: count toward the score, the order mutants are kept in, and how each was built.
use cupel_core::syntax::{MutationOperator, Span, SymbolKey};
use cupel_core::{
    CostClass, FactKind, Mutant, MutantOutcome, MutationOutcomes, MutationStrategy, ProducerId,
    SiteHash, schema,
};

fn mutant(file: &str, line: u32, to: &str, outcome: MutantOutcome) -> Mutant {
    Mutant {
        symbol: SymbolKey {
            file: file.to_owned(),
            line: 1,
            col: 1,
        },
        symbol_hash: None,
        site_hash: "0000000000000001".parse::<SiteHash>().unwrap(),
        operator: MutationOperator::Relational,
        from: "<".to_owned(),
        to: to.to_owned(),
        span: Span {
            line,
            col: 5,
            end_line: line,
            end_col: 6,
        },
        outcome,
        tests: Vec::new(),
        killing_tests: Vec::new(),
        duration_ms: None,
        reused: false,
        equivalent: None,
        cache_key: None,
        strategy: None,
        fallback: None,
    }
}

#[test]
fn mutation_outcomes_is_a_fact_kind_with_a_versioned_schema() {
    assert_eq!(FactKind::MutationOutcomes.as_str(), "mutation-outcomes");
    assert!(!FactKind::syntactic().contains(&FactKind::MutationOutcomes));
    assert_eq!(schema::fact_version(FactKind::MutationOutcomes), Some(2));
}

#[test]
fn mutants_are_kept_by_file_then_position_then_replacement() {
    let outcomes = MutationOutcomes::new(
        ProducerId::from("engine.mutation"),
        None,
        CostClass::Standard,
        false,
        vec![
            mutant("b.py", 1, "<=", MutantOutcome::Killed),
            mutant("a.py", 9, ">", MutantOutcome::Killed),
            mutant("a.py", 2, "<=", MutantOutcome::Killed),
            mutant("a.py", 2, ">=", MutantOutcome::Killed),
        ],
    );
    let order: Vec<(&str, u32, &str)> = outcomes
        .mutants
        .iter()
        .map(|m| (m.symbol.file.as_str(), m.span.line, m.to.as_str()))
        .collect();
    assert_eq!(
        order,
        [
            ("a.py", 2, "<="),
            ("a.py", 2, ">="),
            ("a.py", 9, ">"),
            ("b.py", 1, "<=")
        ]
    );
}

#[test]
fn only_killed_and_survived_mutants_are_scored() {
    let scored: Vec<(MutantOutcome, Option<bool>)> = [
        MutantOutcome::Killed,
        MutantOutcome::Survived,
        MutantOutcome::Timeout,
        MutantOutcome::OutOfMemory,
        MutantOutcome::Uncovered,
        MutantOutcome::Unviable,
    ]
    .into_iter()
    .map(|outcome| (outcome, outcome.killed()))
    .collect();
    assert_eq!(
        scored,
        [
            (MutantOutcome::Killed, Some(true)),
            (MutantOutcome::Survived, Some(false)),
            (MutantOutcome::Timeout, None),
            (MutantOutcome::OutOfMemory, None),
            (MutantOutcome::Uncovered, None),
            (MutantOutcome::Unviable, None),
        ]
    );
    assert_eq!(MutantOutcome::Timeout.as_str(), "timeout");
    assert_eq!(MutantOutcome::OutOfMemory.as_str(), "out-of-memory");
}

#[test]
fn each_mutant_says_how_it_was_built_and_why_not_as_schemata() {
    let mut built = mutant("a.rs", 2, ">", MutantOutcome::Killed);
    built.strategy = Some(MutationStrategy::FileCopy);
    built.fallback = Some("inside a pattern".to_owned());
    let json = serde_json::to_value(&built).unwrap();
    assert_eq!(
        (&json["strategy"], &json["fallback"]),
        (
            &serde_json::json!("file-copy"),
            &serde_json::json!("inside a pattern")
        )
    );
    assert_eq!(MutationStrategy::Schemata.as_str(), "schemata");
}

#[test]
fn a_mutant_stored_before_strategies_still_reads() {
    let mut json = serde_json::to_value(mutant("a.py", 2, "<=", MutantOutcome::Killed)).unwrap();
    let object = json.as_object_mut().unwrap();
    object.remove("strategy");
    object.remove("fallback");
    let read: Mutant = serde_json::from_value(json).unwrap();
    assert_eq!((read.strategy, read.fallback), (None, None));
}
