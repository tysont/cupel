//! Sparse tests: one assertion each, a quarter of the ledger tests' density, and the last one
//! sets up a month of entries for its single assertion.
use ledger::report::{largest, statement};

#[test]
fn the_statement_names_the_owner() {
    assert_eq!(statement("ada", &[5]), "ada: 5");
}

#[test]
fn the_largest_entry_is_found() {
    assert_eq!(largest(&[3, 9, 4]), Some(9));
}

#[test]
fn a_month_of_entries_ends_in_one_statement() {
    let mut entries = Vec::new();
    entries.push(100);
    entries.push(-30);
    entries.push(100);
    entries.push(-30);
    entries.push(100);
    entries.push(-30);
    entries.push(100);
    entries.push(-30);
    entries.push(100);
    entries.push(-30);
    entries.push(100);
    entries.push(-30);
    entries.push(100);
    entries.push(-30);
    entries.push(100);
    entries.push(-30);
    entries.push(100);
    entries.push(-30);
    entries.push(100);
    entries.push(-30);
    let printed = statement("ada", &entries);
    assert_eq!(printed, "ada: 700");
}
