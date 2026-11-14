// ABOUTME: A digit parser the integration tests check: a single ASCII digit is its value, anything
// ABOUTME: else is an error naming the input.
pub fn parse(text: &str) -> Result<u8, String> {
    match text.as_bytes() {
        [digit @ b'0'..=b'9'] => Ok(digit - b'0'),
        _ => Err(format!("not a digit: {text}")),
    }
}
