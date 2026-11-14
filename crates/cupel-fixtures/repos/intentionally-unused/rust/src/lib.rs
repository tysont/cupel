// ABOUTME: A decoder trait whose object safety a `_`-named function checks at compile time, beside a
// ABOUTME: private function nothing calls: only the second is dead code.

pub trait Decode {
    fn decode(&self, text: &str) -> usize;
}

// The existence of this function makes the compiler catch if `Decode` stops being object-safe.
fn _assert_trait_object(_d: &dyn Decode) {}

fn unused_length(text: &str) -> usize {
    text.len()
}
