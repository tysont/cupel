// ABOUTME: Tests that check values through a helper of the shared module, called by path as clap's
// ABOUTME: `common::register_example::<RuntimeBuilder>(…)` is, and one that only builds inputs.
use crate::common;

#[test]
fn seven_parses() {
    common::check_parse::<&str>("7", 7);
}

#[test]
fn inputs_are_built() {
    common::inputs();
}
