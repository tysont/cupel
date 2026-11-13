//! The fee an order pays; only the sibling crate `shop` tests it.

/// Free shipping from 100, else a fee of 5.
pub fn fee(total: u32) -> u32 {
    if total >= 100 {
        return 0;
    }
    5
}
