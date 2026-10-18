// ABOUTME: A second catalog entry submitted for a kind makes the catalog panic (ADR-123). It is its own
// ABOUTME: test binary because the submission joins the catalog of every test in this binary.
use cupel_core::{FactEntry, Footprint};

inventory::submit!(FactEntry::execution::<Footprint>("footprint"));

#[test]
#[should_panic(expected = "fact kinds have one catalog entry each")]
fn a_second_entry_for_a_kind_panics() {
    FactEntry::catalog();
}
