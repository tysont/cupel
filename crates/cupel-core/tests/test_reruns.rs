// ABOUTME: Tests for the `TestReruns` payload (ADR-078): its fact kind and schema, and the order key that
// ABOUTME: says whether two reruns ran their tests in the same order.
use cupel_core::{FactKind, RunOrder, TestRerun, schema};

fn rerun(order: RunOrder, seed: u64, executed: Option<&[&str]>) -> TestRerun {
    TestRerun {
        order,
        seed,
        parallelism: Some(1),
        executed: executed.map(|ids| ids.iter().map(|id| (*id).to_owned()).collect()),
        invocations: Vec::new(),
        tests: Vec::new(),
    }
}

#[test]
fn test_reruns_is_a_fact_kind_with_a_versioned_schema() {
    assert_eq!(FactKind::TestReruns.as_str(), "test-reruns");
    assert!(FactKind::ALL.contains(&FactKind::TestReruns));
    assert!(!FactKind::SYNTACTIC.contains(&FactKind::TestReruns));
    assert_eq!(schema::fact_version(FactKind::TestReruns), Some(1));
}

#[test]
fn reruns_that_report_their_order_compare_by_what_ran() {
    let declared = rerun(RunOrder::Declared, 1, Some(&["a", "b"]));
    let shuffled_same = rerun(RunOrder::Shuffled, 9, Some(&["a", "b"]));
    let shuffled_other = rerun(RunOrder::Shuffled, 9, Some(&["b", "a"]));
    assert_eq!(declared.order_key(), shuffled_same.order_key());
    assert_ne!(declared.order_key(), shuffled_other.order_key());
}

#[test]
fn reruns_that_do_not_report_their_order_compare_by_what_was_asked() {
    let first = rerun(RunOrder::Declared, 1, None);
    let second = rerun(RunOrder::Declared, 2, None);
    assert_eq!(first.order_key(), second.order_key());
    assert_ne!(
        rerun(RunOrder::Shuffled, 1, None).order_key(),
        rerun(RunOrder::Shuffled, 2, None).order_key()
    );
    assert_ne!(
        first.order_key(),
        rerun(RunOrder::Reversed, 1, None).order_key()
    );
}
