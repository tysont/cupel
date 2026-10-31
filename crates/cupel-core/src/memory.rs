// ABOUTME: What the machine and a run's processes hold in memory, read without unsafe code (ADR-148): physical memory
// ABOUTME: from the system, and a run's resident set and process groups from `ps` samples shared by every `Process`.
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// How old a sample may be before the next reader takes a new one.
const SAMPLE_AGE: Duration = Duration::from_millis(200);

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
/// pid, the group ID, the resident set in KiB, the elapsed time since it started, and the command
/// (last, as it may hold spaces); `None` where `ps` cannot run.
#[cfg(unix)]
fn list() -> Option<Vec<Listed>> {
    let output = std::process::Command::new("ps")
        .args([
            "-A", "-o", "pid=", "-o", "ppid=", "-o", "pgid=", "-o", "rss=", "-o", "etime=", "-o",
            "comm=",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mut processes = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
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
    Some(processes)
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
        SAMPLE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(|sample| sample.taken)
    }

    /// `ps` is Unix: Windows takes no sample (`list` is `None` there).
    #[cfg(unix)]
    #[test]
    fn a_reading_not_before_an_instant_renews_a_shared_sample_older_than_it() {
        let me = std::process::id();
        let start = Instant::now();
        assert_eq!(
            resident_since(me, start).map(|held| held.bytes() > 0),
            Some(true),
            "ps lists this process"
        );
        let before = latest_taken().expect("the reading left a sample");
        let mark = Instant::now();
        assert_eq!(
            resident_since(me, mark).map(|held| held.bytes() > 0),
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
