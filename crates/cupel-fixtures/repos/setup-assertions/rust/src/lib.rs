// ABOUTME: Bit masks the tests check value by value, and a pair type the arranging tests build.
// ABOUTME: Each function is one expression, so the tests' shapes are all that differ.
pub fn lower_mask(n: u64) -> u64 {
    if n == 0 { 0 } else { (1 << n) - 1 }
}

pub fn halfway(n: u64) -> u64 {
    if n == 0 { 0 } else { 1 << (n - 1) }
}

pub struct Pair {
    pub left: u64,
    pub right: u64,
}

impl Pair {
    pub fn sum(&self) -> u64 {
        self.left + self.right
    }

    pub fn product(&self) -> u64 {
        self.left * self.right
    }
}
