use super::LIMIT;

pub fn cap(value: u32) -> u32 {
    value.min(LIMIT)
}
