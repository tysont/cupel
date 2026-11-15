// ABOUTME: A decoder trait whose object safety a `_`-named function checks at compile time, beside a
// ABOUTME: private function nothing calls and a decoder its test covers: only the second is dead or uncovered.

pub trait Decode {
    fn decode(&self, text: &str) -> usize;
}

/// Decodes text as its length.
pub struct Plain;

impl Decode for Plain {
    fn decode(&self, text: &str) -> usize {
        text.len()
    }
}

// The existence of this function makes the compiler catch if `Decode` stops being object-safe.
fn _assert_trait_object(_d: &dyn Decode) {}

fn unused_length(text: &str) -> usize {
    text.len()
}
