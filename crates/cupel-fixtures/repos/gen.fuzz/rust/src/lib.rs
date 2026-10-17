//! Byte helpers the fuzz targets exercise.

/// The first byte of `data`, which indexes without checking: the empty input panics.
pub fn first(data: &[u8]) -> u8 {
    data[0]
}

/// The first byte of `data`, if it has one.
pub fn checked_first(data: &[u8]) -> Option<u8> {
    data.first().copied()
}
