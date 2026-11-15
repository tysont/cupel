// ABOUTME: What the machine and a run's processes hold in memory, read without unsafe code (ADR-148): physical memory
// ABOUTME: from the system, and a run's resident set and process groups from `ps` samples shared by every `Process`.
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use crate::clock;

/// How old a sample may be before the next reader starts a new one.
const SAMPLE_AGE: Duration = Duration::from_millis(200);

/// How long one `ps` listing may run before its sample is skipped (ADR-236), in milliseconds as
/// the `sampler.timeout` event reports it.
pub(crate) const LIST_TIMEOUT_MS: u64 = 5_000;
const LIST_TIMEOUT: Duration = Duration::from_millis(LIST_TIMEOUT_MS);

/// How often a listing is checked for having ended.
const LIST_POLL: Duration = Duration::from_millis(10);

/// `ps` rounds a process's age to whole seconds, so a process started in the same second as the
/// child may read as this much older than it.
const AGE_SLACK: Duration = Duration::from_secs(1);

/// One running process as `ps` lists it.
#[derive(Debug, Clone)]
struct Listed {
    pid: u32,
    ppid: u32,
    pgid: u32,
    /// Resident set in bytes.
    rss: u64,
    /// How long ago the process started, to the second.
    age: Duration,
    /// Its command's name.
    command: String,
}

/// The processes of a child's run from one sample, as the memory budget counts them.
#[derive(Debug, Clone)]
pub(crate) struct Held {
    processes: Vec<Listed>,
}

impl Held {
    /// Their resident set in bytes.
    pub(crate) fn bytes(&self) -> u64 {
        self.processes.iter().map(|process| process.rss).sum()
    }
}

/// One line per process: its pid, command, and resident set, then its parent, group, and age, which
/// name why the budget counted it.
impl std::fmt::Display for Held {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, process) in self.processes.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            let size = if process.rss >= 1 << 20 {
                format!("{} MiB", process.rss >> 20)
            } else {
                format!("{} KiB", process.rss >> 10)
            };
            write!(
                f,
                "pid {} `{}` {size} (parent {}, group {}, {} s old)",
                process.pid,
                process.command,
                process.ppid,
                process.pgid,
                process.age.as_secs()
            )?;
        }
        Ok(())
    }
}

/// Every running process, from a listing started at `started` and ended at `ended`.
struct Sample {
    started: Instant,
    ended: Instant,
    processes: Arc<Vec<Listed>>,
}

/// What every `Process` that watches its child's memory shares. Its lock is held only to read or
/// write these fields, never across a listing (ADR-236).
struct Sampler {
    latest: Option<Sample>,
    /// When the newest listing under way on its own thread started.
    listing: Option<Instant>,
    /// The last listing that ended or timed out timed out: a kill signals the child's own group
    /// without listing again until one ends in time.
    stalled: bool,
    /// Listings that timed out since [`take_sampler_timeouts`] last took the count.
    timeouts: u64,
}

static SAMPLER: Mutex<Sampler> = Mutex::new(Sampler {
    latest: None,
    listing: None,
    stalled: false,
    timeouts: 0,
});

fn sampler() -> MutexGuard<'static, Sampler> {
    SAMPLER.lock().unwrap_or_else(PoisonError::into_inner)
}

/// How many process listings timed out since the last call, each a memory sample skipped and a
/// `sampler.timeout` event to report (ADR-236).
pub(crate) fn take_sampler_timeouts() -> u64 {
    std::mem::take(&mut sampler().timeouts)
}

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

/// The processes that belong to the run of `pid`, with their resident sets: its process
/// group and every descendant, whichever group a descendant moved to (`cargo nextest` gives each
/// test one of its own), from a sample taken at `not_before` or later and at most `SAMPLE_AGE`
/// old; `None` where no sample can be taken. A child shares its parent's memory between its
/// spawn and its `exec` on Linux (`posix_spawn` clones with `CLONE_VM`), so a sample taken while
/// it was spawned lists it with the parent's resident set; a reader passes the instant its spawn
/// returned and never reads that (ADR-148 §3). `not_before` is also when the child was spawned,
/// which bounds the age of a process its group may hold.
pub(crate) fn resident_since(pid: u32, not_before: Instant) -> Option<Held> {
    let processes = sample_since(not_before)?;
    let members = members(&processes, pid, age_since(not_before));
    Some(Held {
        processes: processes
            .iter()
            .filter(|process| members.contains(&process.pid))
            .cloned()
            .collect(),
    })
}

/// The process groups the run of `pid`, spawned at `spawned`, spans, from a new sample: `pid`'s
/// own first, then every other group one of its descendants is in. Only `pid`'s where no sample
/// can be taken.
pub(crate) fn groups(pid: u32, spawned: Instant) -> Vec<u32> {
    if sampler().stalled {
        return vec![pid];
    }
    let Some(processes) = list() else {
        return vec![pid];
    };
    let members = members(&processes, pid, age_since(spawned));
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

/// How old a child spawned at `spawned` is, with `AGE_SLACK`: the oldest a process of its run
/// can read as.
fn age_since(spawned: Instant) -> Duration {
    spawned.elapsed() + AGE_SLACK
}

/// The pids of the run of `pid`, a child no older than `age`: `pid` itself, the processes of its
/// group that are no older than it, and every descendant of one of those. A group's ID is the pid
/// its leader had, and pids are reused once free (macOS wraps at 99 998), so a process older than
/// the child in a group numbered `pid` belongs to an earlier process whose pid the child
/// inherited, not to the run (ADR-148 §3).
fn members(processes: &[Listed], pid: u32, age: Duration) -> BTreeSet<u32> {
    let mut members: BTreeSet<u32> = processes
        .iter()
        .filter(|process| process.pid == pid || (process.pgid == pid && process.age <= age))
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

/// The latest sample when it ended less than `SAMPLE_AGE` ago and started at `not_before` or
/// later. Otherwise `None` at once, with a listing for the next sample started on its own thread
/// unless one that started at `not_before` or later is under way, or the sampler is stalled and
/// any is: a reader never waits on `ps`, so a listing that hangs delays no timeout and the budget
/// goes unmeasured instead (ADR-236). A listing from before `not_before` cannot serve the reader,
/// so it starts its own rather than wait for that one to end first.
fn sample_since(not_before: Instant) -> Option<Arc<Vec<Listed>>> {
    let started = {
        let mut sampler = sampler();
        if let Some(sample) = sampler.latest.as_ref()
            && sample.ended.elapsed() < SAMPLE_AGE
            && sample.started >= not_before
        {
            return Some(Arc::clone(&sample.processes));
        }
        if sampler
            .listing
            .is_some_and(|listing| sampler.stalled || listing >= not_before)
        {
            return None;
        }
        let started = clock::now();
        sampler.listing = Some(started);
        started
    };
    if std::thread::Builder::new()
        .name("memory-sample".to_owned())
        .spawn(move || renew(started))
        .is_err()
    {
        ended(started);
    }
    None
}

/// Lists every process from `started` and keeps the listing as the latest sample, unless a
/// listing that started later already is.
fn renew(started: Instant) {
    /// Marks the listing ended however `renew` ends, so a panic in it cannot stop the sampler.
    struct Ended(Instant);
    impl Drop for Ended {
        fn drop(&mut self) {
            ended(self.0);
        }
    }
    let _ended = Ended(started);
    let Some(processes) = list() else {
        return;
    };
    let mut sampler = sampler();
    if sampler
        .latest
        .as_ref()
        .is_none_or(|latest| latest.started < started)
    {
        sampler.latest = Some(Sample {
            started,
            ended: clock::now(),
            processes: Arc::new(processes),
        });
    }
}

/// Clears the listing under way when it is the one that started at `started`.
fn ended(started: Instant) {
    let mut sampler = sampler();
    if sampler.listing == Some(started) {
        sampler.listing = None;
    }
}

/// Every running process from `ps`, which macOS and Linux both print as the pid, the parent's
/// pid, the group ID, the resident set in KiB, the elapsed time since it started, and the command
/// (last, as it may hold spaces); `None` where `ps` cannot run.
#[cfg(unix)]
fn list() -> Option<Vec<Listed>> {
    let mut ps = std::process::Command::new("ps");
    ps.args([
        "-A", "-o", "pid=", "-o", "ppid=", "-o", "pgid=", "-o", "rss=", "-o", "etime=", "-o",
        "comm=",
    ]);
    listed_by(&mut ps, LIST_TIMEOUT)
}

/// The processes `command` lists in `ps`'s columns; `None` where it cannot run, fails, or runs
/// past `timeout`, which is killed, counted, and marks the sampler stalled (ADR-236). The listing
/// goes to an unlinked file read once `command` exits, not to a pipe read to its end: a process
/// that inherited the pipe would hold that end back for as long as it lives.
#[cfg(unix)]
fn listed_by(command: &mut std::process::Command, timeout: Duration) -> Option<Vec<Listed>> {
    use std::io::{Read, Seek};
    use std::process::Stdio;
    let mut out = tempfile::tempfile().ok()?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(out.try_clone().ok()?)
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let Some(status) = ended_within(&mut child, timeout) else {
        let _ = child.kill();
        let _ = child.wait();
        let mut sampler = sampler();
        sampler.timeouts += 1;
        sampler.stalled = true;
        return None;
    };
    sampler().stalled = false;
    if !status.success() {
        return None;
    }
    let mut stdout = Vec::new();
    out.rewind().ok()?;
    out.read_to_end(&mut stdout).ok()?;
    Some(parsed(&String::from_utf8_lossy(&stdout)))
}

/// How `child` exited, when it did within `timeout`; `None` when it is still running then or
/// can no longer be waited for.
#[cfg(unix)]
fn ended_within(
    child: &mut std::process::Child,
    timeout: Duration,
) -> Option<std::process::ExitStatus> {
    let until = clock::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) if clock::now() < until => std::thread::sleep(LIST_POLL),
            _ => return None,
        }
    }
}

/// The processes in `ps`'s columns, one per line; a line that does not parse is left out.
#[cfg(unix)]
fn parsed(text: &str) -> Vec<Listed> {
    let mut processes = Vec::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if let [pid, ppid, pgid, kib, etime, ref command @ ..] = fields[..]
            && let (Ok(pid), Ok(ppid), Ok(pgid), Ok(kib), Some(age)) = (
                pid.parse::<u32>(),
                ppid.parse::<u32>(),
                pgid.parse::<u32>(),
                kib.parse::<u64>(),
                elapsed(etime),
            )
        {
            processes.push(Listed {
                pid,
                ppid,
                pgid,
                rss: kib.saturating_mul(1024),
                age,
                command: command.join(" "),
            });
        }
    }
    processes
}

/// The elapsed time `ps` prints as `[[dd-]hh:]mm:ss`; `None` for anything else.
fn elapsed(etime: &str) -> Option<Duration> {
    let (days, clock) = match etime.split_once('-') {
        Some((days, clock)) => (days.parse::<u64>().ok()?, clock),
        None => (0, etime),
    };
    let parts: Vec<u64> = clock
        .split(':')
        .map(|part| part.parse::<u64>().ok())
        .collect::<Option<_>>()?;
    let (hours, minutes, seconds) = match parts[..] {
        [minutes, seconds] => (0, minutes, seconds),
        [hours, minutes, seconds] => (hours, minutes, seconds),
        _ => return None,
    };
    Some(Duration::from_secs(
        ((days * 24 + hours) * 60 + minutes) * 60 + seconds,
    ))
}

#[cfg(not(unix))]
fn list() -> Option<Vec<Listed>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(id: u32, parent: u32, group: u32, age_secs: u64) -> Listed {
        Listed {
            pid: id,
            ppid: parent,
            pgid: group,
            rss: 1,
            age: Duration::from_secs(age_secs),
            command: String::new(),
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_process_left_holding_the_listings_output_does_not_hold_up_the_sample() {
        // On macOS a pipe is made close-on-exec only after it is made, so a run another thread
        // spawns in between keeps the listing's output open for as long as it lives. Every wait
        // loop takes its sample under one lock, so a listing that waited for that run to close
        // the output would stop every timeout, the one that would end the run among them.
        let mut ps = std::process::Command::new("sh");
        ps.args(["-c", "echo '10 1 10 4 00:01 ps'; sleep 20 &"]);
        let started = Instant::now();
        let listed = listed_by(&mut ps, LIST_TIMEOUT).unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the listing waited {:?} for a process holding its output",
            started.elapsed()
        );
        assert_eq!(listed.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn a_listing_that_outlasts_its_timeout_is_skipped_counted_and_stops_the_kill_listing() {
        let _serial = sampler_tests();
        let mut ps = std::process::Command::new("sh");
        ps.args(["-c", "exec sleep 30"]);
        let started = Instant::now();
        let listed = listed_by(&mut ps, Duration::from_millis(200));
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the listing ran {:?} past its timeout",
            started.elapsed()
        );
        // Stalled, a kill signals the child's own group without waiting on another listing.
        assert_eq!(
            (
                listed.is_none(),
                take_sampler_timeouts(),
                groups(4_000_000, Instant::now())
            ),
            (true, 1, vec![4_000_000])
        );
        let _ = list();
    }

    #[test]
    fn an_older_process_in_the_childs_group_is_a_stranger_whose_group_id_the_child_inherited() {
        let processes = [
            listed(10, 1, 10, 1),   // the child, spawned a second ago
            listed(20, 1, 10, 500), // alive since before the child, in a group numbered 10
            listed(50, 20, 50, 0),  // the stranger's own child
            listed(30, 1, 10, 1),   // reparented to launchd, still in the child's group
            listed(40, 30, 40, 0),  // a descendant that moved to a group of its own
        ];
        assert_eq!(
            members(&processes, 10, Duration::from_secs(2)),
            BTreeSet::from([10, 30, 40])
        );
    }

    #[test]
    fn held_processes_are_named_one_per_line_with_their_size_parent_group_and_age() {
        let held = Held {
            processes: vec![
                Listed {
                    rss: 300 << 20,
                    command: "perl".to_string(),
                    ..listed(11, 10, 11, 3)
                },
                Listed {
                    rss: 512 << 10,
                    command: "sh".to_string(),
                    ..listed(10, 1, 10, 4)
                },
            ],
        };
        assert_eq!(
            (held.bytes(), held.to_string()),
            (
                (300 << 20) + (512 << 10),
                "pid 11 `perl` 300 MiB (parent 10, group 11, 3 s old)\n\
                 pid 10 `sh` 512 KiB (parent 1, group 10, 4 s old)"
                    .to_string()
            )
        );
    }

    #[test]
    fn elapsed_reads_every_form_ps_prints() {
        assert_eq!(
            (
                elapsed("00:05"),
                elapsed("12:34"),
                elapsed("01:02:03"),
                elapsed("03-09:11:26"),
                elapsed("junk")
            ),
            (
                Some(Duration::from_secs(5)),
                Some(Duration::from_secs(754)),
                Some(Duration::from_secs(3723)),
                Some(Duration::from_secs(3 * 86400 + 9 * 3600 + 11 * 60 + 26)),
                None
            )
        );
    }

    /// When the latest shared sample was taken.
    fn latest_taken() -> Option<Instant> {
        sampler().latest.as_ref().map(|sample| sample.started)
    }

    /// Held by each test that reads or sets the shared sampler, so none sees another's state.
    static SAMPLER_TESTS: Mutex<()> = Mutex::new(());

    fn sampler_tests() -> std::sync::MutexGuard<'static, ()> {
        SAMPLER_TESTS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `pid`'s run from the first sample taken at `since` or later: a reading starts the listing
    /// and returns at once, and a later one finds it.
    fn resident_once_sampled(pid: u32, since: Instant) -> Option<Held> {
        let until = Instant::now() + LIST_TIMEOUT + Duration::from_secs(1);
        loop {
            let held = resident_since(pid, since);
            if held.is_some() || Instant::now() >= until {
                return held;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_reading_after_a_listing_under_way_started_starts_its_own_and_is_served_by_it() {
        let _serial = sampler_tests();
        // A listing that started before the child's spawn cannot serve it; waiting for it and
        // then for another would let a run that ends within two listings escape its budget.
        sampler().listing = Some(Instant::now());
        let mark = Instant::now() + Duration::from_millis(1);
        std::thread::sleep(Duration::from_millis(2));
        assert_eq!(
            resident_once_sampled(std::process::id(), mark).map(|held| held.bytes() > 0),
            Some(true),
            "no listing started after the mark served the reading"
        );
        sampler().listing = None;
    }

    #[cfg(unix)]
    #[test]
    fn a_reading_while_another_listing_is_under_way_returns_at_once_with_no_sample() {
        let _serial = sampler_tests();
        let started = Instant::now();
        sampler().listing = Some(started);
        let held = resident_since(std::process::id(), started);
        let waited = started.elapsed();
        sampler().listing = None;
        assert_eq!(
            (held.is_none(), waited < Duration::from_millis(100)),
            (true, true),
            "the reading waited {waited:?}"
        );
    }

    /// `ps` is Unix: Windows takes no sample (`list` is `None` there).
    #[cfg(unix)]
    #[test]
    fn a_reading_not_before_an_instant_renews_a_shared_sample_older_than_it() {
        let _serial = sampler_tests();
        let me = std::process::id();
        let start = Instant::now();
        assert_eq!(
            resident_once_sampled(me, start).map(|held| held.bytes() > 0),
            Some(true),
            "ps lists this process"
        );
        let before = latest_taken().expect("the reading left a sample");
        let mark = Instant::now();
        assert_eq!(
            resident_once_sampled(me, mark).map(|held| held.bytes() > 0),
            Some(true)
        );
        let after = latest_taken().expect("the reading left a sample");
        assert_eq!(
            (before <= mark, after >= mark),
            (true, true),
            "the sample from {:?} before the mark was reused",
            mark.duration_since(before)
        );
    }
}
