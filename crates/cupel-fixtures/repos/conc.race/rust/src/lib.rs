//! Publishes counts from threads.
use std::thread;

static mut STORED: u64 = 0;

/// Hands `value` to a thread that stores it, and reads the store before the thread is joined:
/// a data race between the write and the read.
pub fn publish(value: u64) -> u64 {
    let writer = thread::spawn(move || unsafe { STORED = value });
    let seen = unsafe { STORED };
    writer.join().unwrap();
    seen
}

/// The last of four values, read one past the end: undefined behavior.
pub fn last(values: &[u8; 4]) -> u8 {
    let start = values.as_ptr();
    unsafe { *start.add(4) }
}
