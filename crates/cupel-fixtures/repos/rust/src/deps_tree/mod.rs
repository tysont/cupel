mod leaf;

pub const LIMIT: u32 = 10;

pub fn capped(value: u32) -> u32 {
    leaf::cap(value)
}
