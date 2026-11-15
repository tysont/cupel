// ABOUTME: The one clock budgets, timeouts, and run durations are read from (ADR-217): monotonic, and stopped while
// ABOUTME: the machine sleeps or is suspended, so `--budget`, `--timeout`, and `duration_ms` count awake time.
use std::time::Instant;

/// Now, on the clock that counts only the time the machine is awake: std's [`Instant`], which
/// at the pinned toolchain reads `CLOCK_UPTIME_RAW` on macOS and `CLOCK_MONOTONIC` on Linux.
/// Neither advances while the machine sleeps or is suspended. std does not promise that, so
/// a test here pins it (ADR-217).
pub fn now() -> Instant {
    Instant::now()
}

#[cfg(all(test, unix))]
mod tests {
    use nix::time::{ClockId, clock_gettime};

    use super::now;

    /// The clock that does not count sleep or suspend on this platform.
    #[cfg(target_vendor = "apple")]
    const SLEEPLESS: ClockId = ClockId::from_raw(nix::libc::CLOCK_UPTIME_RAW);
    #[cfg(not(target_vendor = "apple"))]
    const SLEEPLESS: ClockId = ClockId::CLOCK_MONOTONIC;

    fn sleepless_seconds() -> i64 {
        clock_gettime(SLEEPLESS).unwrap().tv_sec()
    }

    /// The whole seconds of `debug`, an `Instant`'s `Debug` text (`Instant { tv_sec: …, … }`).
    fn seconds(debug: &str) -> i64 {
        let after = debug
            .split("tv_sec: ")
            .nth(1)
            .unwrap_or_else(|| panic!("no tv_sec in {debug:?}: look at the clock again"));
        let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
        digits.parse().unwrap()
    }

    #[test]
    fn the_clock_stops_while_the_machine_sleeps() {
        // A clock that counts sleep (`CLOCK_MONOTONIC` on macOS, `CLOCK_BOOTTIME` on Linux) is
        // ahead of this one by every second the machine slept since it booted.
        let before = sleepless_seconds();
        let instant = now();
        let after = sleepless_seconds();
        let read = seconds(&format!("{instant:?}"));
        assert!(
            (before..=after).contains(&read),
            "the clock read {read} s; the sleep-excluding clock read {before} s to {after} s"
        );
    }
}
