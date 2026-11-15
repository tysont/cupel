// ABOUTME: The test that covers the plain decoder, and nothing else of the crate, so the coverage report
// ABOUTME: shows the `_`-named function and the dead one never ran (ADR-226).
use codec::{Decode, Plain};

#[test]
fn plain_decodes_to_the_length() {
    assert_eq!(Plain.decode("abc"), 3);
}
