// ABOUTME: The production's phase recorder (ADR-150): every worker records the phase it finished, what it served, and
// ABOUTME: how long it took; the rows, in the order the phases ended, are the fact's timeline.
use std::sync::Mutex;
use std::time::Instant;

use cupel_core::{MutationPhase, PhaseDuration};

/// The phases one production went through so far.
#[derive(Debug, Default)]
pub struct Phases(Mutex<Vec<PhaseDuration>>);

/// What a phase served: the file whose mutants it was for, the slot it ran in, and the tests
/// it ran (`None` for the whole suite, or none).
#[derive(Debug, Clone, Copy, Default)]
pub struct Served<'a> {
    pub file: Option<&'a str>,
    pub slot: Option<usize>,
    pub tests: Option<usize>,
}

impl Phases {
    /// Records that `phase`, which `served` what it says, ran from `started` until now.
    pub fn record(&self, phase: MutationPhase, served: Served<'_>, started: Instant) {
        let row = PhaseDuration {
            phase,
            file: served.file.map(str::to_owned),
            slot: served
                .slot
                .map(|slot| u32::try_from(slot).unwrap_or(u32::MAX)),
            tests: served
                .tests
                .map(|tests| u32::try_from(tests).unwrap_or(u32::MAX)),
            duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        };
        if let Ok(mut rows) = self.0.lock() {
            rows.push(row);
        }
    }

    /// The rows recorded, in the order the phases ended.
    pub fn rows(self) -> Vec<PhaseDuration> {
        self.0.into_inner().unwrap_or_default()
    }
}
