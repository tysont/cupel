/// `value` limited to 0..=100; tested by example only, so it has no property.
pub fn clamp_percent(value: i64) -> i64 {
    value.clamp(0, 100)
}
