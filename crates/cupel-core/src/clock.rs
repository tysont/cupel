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

    /// `now()` and the second the sleep-excluding clock read around it, taken again until that
    /// clock reads the same second before and after.
    fn now_within_a_second() -> (i64, String) {
        loop {
            let before = sleepless_seconds();
            let instant = now();
            if sleepless_seconds() == before {
                return (before, format!("{instant:?}"));
            }
        }
    }

    #[test]
    fn the_clock_stops_while_the_machine_sleeps() {
        // A clock that counts sleep (`CLOCK_MONOTONIC` on macOS, `CLOCK_BOOTTIME` on Linux) is
        // ahead of this one by every second the machine slept since it booted.
        let (sleepless, instant) = now_within_a_second();
        assert_eq!(
            seconds(&instant),
            sleepless,
            "the clock is not the one that stops while the machine sleeps"
        );
    }
}
