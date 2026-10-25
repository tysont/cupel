// ABOUTME: What the machine and a process group hold in memory, read without unsafe code (ADR-148): physical memory
// ABOUTME: from the system, and each group's resident set from one `ps` sample shared by every running `Process`.
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// How old a sample may be before the next reader takes a new one.
const SAMPLE_AGE: Duration = Duration::from_millis(200);

/// The resident set of every process group, in bytes, as of `taken`.
struct Sample {
    taken: Instant,
    by_group: Arc<BTreeMap<u32, u64>>,
}

/// The latest sample, shared by every `Process` that watches its child's memory.
static SAMPLE: Mutex<Option<Sample>> = Mutex::new(None);

/// The machine's physical memory in bytes; `None` where it cannot be read.
pub fn physical_memory() -> Option<u64> {
    static PHYSICAL: OnceLock<Option<u64>> = OnceLock::new();
    *PHYSICAL.get_or_init(read_physical_memory)
}

#[cfg(target_os = "linux")]
fn read_physical_memory() -> Option<u64> {
    let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = meminfo.lines().find(|line| line.starts_with("MemTotal:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib.saturating_mul(1024))
}

#[cfg(target_os = "macos")]
fn read_physical_memory() -> Option<u64> {
    let output = std::process::Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn read_physical_memory() -> Option<u64> {
    None
}

/// The resident set, in bytes, of the processes in group `pgid`, from a sample at most
/// `SAMPLE_AGE` old; `None` where no sample can be taken.
pub(crate) fn resident(pgid: u32) -> Option<u64> {
    sample().map(|by_group| by_group.get(&pgid).copied().unwrap_or(0))
}

/// The latest sample, renewed when older than `SAMPLE_AGE`.
fn sample() -> Option<Arc<BTreeMap<u32, u64>>> {
    let mut latest = SAMPLE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(sample) = latest.as_ref() {
        if sample.taken.elapsed() < SAMPLE_AGE {
            return Some(Arc::clone(&sample.by_group));
        }
    }
    let by_group = Arc::new(resident_by_group()?);
    *latest = Some(Sample {
        taken: Instant::now(),
        by_group: Arc::clone(&by_group),
    });
    Some(by_group)
}

/// Every process group's resident set in bytes, summed from `ps`, which macOS and Linux both
/// print as the group ID and the resident set in KiB; `None` where `ps` cannot run.
#[cfg(unix)]
fn resident_by_group() -> Option<BTreeMap<u32, u64>> {
    let output = std::process::Command::new("ps")
        .args(["-A", "-o", "pgid=", "-o", "rss="])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mut by_group = BTreeMap::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let mut fields = line.split_whitespace();
        let (Some(pgid), Some(kib)) = (fields.next(), fields.next()) else {
            continue;
        };
        if let (Ok(pgid), Ok(kib)) = (pgid.parse::<u32>(), kib.parse::<u64>()) {
            *by_group.entry(pgid).or_insert(0) += kib.saturating_mul(1024);
        }
    }
    Some(by_group)
}

#[cfg(not(unix))]
fn resident_by_group() -> Option<BTreeMap<u32, u64>> {
    None
}
