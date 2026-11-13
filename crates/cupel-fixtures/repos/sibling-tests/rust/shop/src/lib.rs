//! The shop: what an order costs with its fee, from `lib-a`.

/// The total with the fee `lib-a` charges.
pub fn charged(total: u32) -> u32 {
    total + lib_a::fee(total)
}
