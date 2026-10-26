// ABOUTME: Two iterator helpers the properties and the plain tests check.
// ABOUTME: Each is one expression.
pub fn doubled(values: &[u8]) -> Vec<u16> {
    values.iter().map(|value| u16::from(*value) * 2).collect()
}

pub fn evens(values: &[u8]) -> Vec<u8> {
    values.iter().copied().filter(|value| value % 2 == 0).collect()
}
