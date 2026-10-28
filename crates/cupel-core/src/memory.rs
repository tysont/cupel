// ABOUTME: What the machine and a run's processes hold in memory, read without unsafe code (ADR-148): physical memory
// ABOUTME: from the system, and a run's resident set and process groups from `ps` samples shared by every `Process`.
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// How old a sample may be before the next reader takes a new one.
const SAMPLE_AGE: Duration = Duration::from_millis(200);

/// One running process as `ps` lists it.
#[derive(Debug, Clone, Copy)]
struct Listed {
    pid: u32,
    ppid: u32,
    pgid: u32,
    /// Resident set in bytes.
    rss: u64,
}

/// Every running process, as of `taken`.
struct Sample {
    taken: Instant,
    processes: Arc<Vec<Listed>>,
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

/// The resident set, in bytes, of the processes that belong to the run of `pid`: its process
/// group and every descendant, whichever group a descendant moved to (`cargo nextest` gives each
/// test one of its own), from a sample taken at `not_before` or later and at most `SAMPLE_AGE`
/// old; `None` where no sample can be taken. A child shares its parent's memory between its
/// spawn and its `exec` on Linux (`posix_spawn` clones with `CLONE_VM`), so a sample taken while
/// it was spawned lists it with the parent's resident set; a reader passes the instant its spawn
/// returned and never reads that (ADR-148 §3).
pub(crate) fn resident_since(pid: u32, not_before: Instant) -> Option<u64> {
    let processes = sample_since(not_before)?;
    let members = members(&processes, pid);
    Some(
        processes
            .iter()
            .filter(|process| members.contains(&process.pid))
            .map(|process| process.rss)
            .sum(),
    )
}

/// The process groups the run of `pid` spans, from a new sample: `pid`'s own first, then every
/// other group one of its descendants is in. Only `pid`'s where no sample can be taken.
pub(crate) fn groups(pid: u32) -> Vec<u32> {
    let Some(processes) = list() else {
        return vec![pid];
    };
    let members = members(&processes, pid);
    let mut groups = vec![pid];
    for process in processes
        .iter()
        .filter(|process| members.contains(&process.pid))
    {
        if !groups.contains(&process.pgid) {
            groups.push(process.pgid);
        }
    }
    groups
}

/// The pids of `pid`'s group and of every descendant of a process in it.
fn members(processes: &[Listed], pid: u32) -> BTreeSet<u32> {
    let mut members: BTreeSet<u32> = processes
        .iter()
        .filter(|process| process.pgid == pid || process.pid == pid)
        .map(|process| process.pid)
        .collect();
    loop {
        let before = members.len();
        for process in processes {
            if members.contains(&process.ppid) {
                members.insert(process.pid);
            }
        }
        if members.len() == before {
            return members;
        }
    }
}

/// The latest sample, renewed when older than `SAMPLE_AGE` or taken before `not_before`.
fn sample_since(not_before: Instant) -> Option<Arc<Vec<Listed>>> {
    let mut latest = SAMPLE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(sample) = latest.as_ref()
        && sample.taken.elapsed() < SAMPLE_AGE
        && sample.taken >= not_before
    {
        return Some(Arc::clone(&sample.processes));
    }
    let processes = Arc::new(list()?);
    *latest = Some(Sample {
        taken: Instant::now(),
        processes: Arc::clone(&processes),
    });
    Some(processes)
}

/// Every running process from `ps`, which macOS and Linux both print as the pid, the parent's
/// pid, the group ID, and the resident set in KiB; `None` where `ps` cannot run.
#[cfg(unix)]
fn list() -> Option<Vec<Listed>> {
    let output = std::process::Command::new("ps")
        .args([
            "-A", "-o", "pid=", "-o", "ppid=", "-o", "pgid=", "-o", "rss=",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mut processes = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let fields: Vec<u64> = line
            .split_whitespace()
            .filter_map(|field| field.parse().ok())
            .collect();
        if let [pid, ppid, pgid, kib] = fields[..]
            && let (Ok(pid), Ok(ppid), Ok(pgid)) =
                (u32::try_from(pid), u32::try_from(ppid), u32::try_from(pgid))
        {
            processes.push(Listed {
                pid,
                ppid,
                pgid,
                rss: kib.saturating_mul(1024),
            });
        }
    }
    Some(processes)
}

#[cfg(not(unix))]
fn list() -> Option<Vec<Listed>> {
    None
}

/// `ps` is Unix: Windows takes no sample (`list` is `None` there), so the test is Unix-only.
#[cfg(all(test, unix))]
mod tests {
    use super::*;

    /// When the latest shared sample was taken.
    fn latest_taken() -> Option<Instant> {
        SAMPLE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(|sample| sample.taken)
    }

    #[test]
    fn a_reading_not_before_an_instant_renews_a_shared_sample_older_than_it() {
        let me = std::process::id();
        let start = Instant::now();
        assert_eq!(
            resident_since(me, start).map(|bytes| bytes > 0),
            Some(true),
            "ps lists this process"
        );
        let before = latest_taken().expect("the reading left a sample");
        let mark = Instant::now();
        assert_eq!(resident_since(me, mark).map(|bytes| bytes > 0), Some(true));
        let after = latest_taken().expect("the reading left a sample");
        assert_eq!(
            (before <= mark, after >= mark),
            (true, true),
            "the sample from {:?} before the mark was reused",
            mark.duration_since(before)
        );
    }
}
