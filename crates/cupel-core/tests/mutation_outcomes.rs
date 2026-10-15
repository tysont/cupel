// ABOUTME: Tests for the `MutationOutcomes` payload (ADR-080): its fact kind and schema, which outcomes count
// ABOUTME: toward the score, and the order mutants are kept in.
use cupel_core::syntax::{MutationOperator, Span, SymbolKey};
use cupel_core::{
    CostClass, FactKind, Mutant, MutantOutcome, MutationOutcomes, ProducerId, SiteHash, schema,
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
    }
}

#[test]
fn mutation_outcomes_is_a_fact_kind_with_a_versioned_schema() {
    assert_eq!(FactKind::MutationOutcomes.as_str(), "mutation-outcomes");
    assert!(!FactKind::SYNTACTIC.contains(&FactKind::MutationOutcomes));
    assert_eq!(schema::fact_version(FactKind::MutationOutcomes), Some(1));
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
            (MutantOutcome::Uncovered, None),
            (MutantOutcome::Unviable, None),
        ]
    );
    assert_eq!(MutantOutcome::Timeout.as_str(), "timeout");
}
