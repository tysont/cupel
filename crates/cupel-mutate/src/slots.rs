// ABOUTME: Where mutation slots live (ADR-080 §8) and how they are reclaimed (ADR-141 §3): each repository's slots in
// ABOUTME: the temporary directory, locked while a production runs, removed when gone, idle, or beyond the total size.
use std::fs::{self, File, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The file in a repository's slot directory naming the repository; rewritten by every claim, so
/// its modification time is the slots' last use.
const OWNER: &str = "repository";

/// The file recording how many bytes the repository's slots held when its last production ended.
const SIZE: &str = "size";

/// The file every production of the repository holds a shared lock on while it runs.
const LOCK: &str = "lock";

/// How many times a claim takes its lock again when the directory was removed while it waited.
const HOLD_ATTEMPTS: usize = 3;

/// When the slots of other repositories are removed (ADR-141 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reclaim {
    /// Slots unused for longer are removed.
    pub idle: Duration,
    /// While every repository's slots together hold more bytes, the least recently used of the
    /// other repositories' are removed.
    pub total: u64,
}

impl Reclaim {
    /// Seven days idle; 32 GiB in all.
    pub const DEFAULT: Self = Self {
        idle: Duration::from_secs(7 * 24 * 60 * 60),
        total: 32 << 30,
    };
}

/// Every repository's slots live under this directory.
pub fn root() -> PathBuf {
    std::env::temp_dir().join("cupel-mutate")
}

/// Where slot `index` of `lang` for the repository at `repo` lives: under the system temporary
/// directory, so no runner in the repository ever discovers it, and the same on every run, so
/// its builds are incremental.
pub fn slot_dir(repo: &Path, lang: &str, index: usize) -> PathBuf {
    slots_of(&root(), repo).join(lang).join(index.to_string())
}

/// The directory under `root` of the slots of the repository at `repo`.
fn slots_of(root: &Path, repo: &Path) -> PathBuf {
    let repo = canonical(repo);
    let digest = blake3::hash(repo.to_string_lossy().as_bytes()).to_hex();
    root.join(&digest[..16])
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// A production's hold on its repository's slots: while it lives, no other production removes
/// them; when it ends, the bytes they hold are recorded.
#[derive(Debug)]
pub struct Claim {
    dir: PathBuf,
    _lock: File,
}

impl Claim {
    /// The directory of the repository's slots.
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        // A size that cannot be written is measured again by the next claim that needs it.
        let _ = fs::write(self.dir.join(SIZE), measure(&self.dir).to_string());
    }
}

/// Claims the slots of the repository at `repo` under [`root`] and reclaims other repositories'
/// by [`Reclaim::DEFAULT`].
///
/// # Errors
///
/// When the repository's slot directory or its records cannot be written.
pub fn claim(repo: &Path) -> io::Result<Claim> {
    claim_in(&root(), repo, Reclaim::DEFAULT)
}

/// Claims the slots of the repository at `repo` under `root`: records it as their owner, used
/// now, holds them for the production, and removes every other repository's slots no production
/// holds whose repository is gone or that `rule` finds idle or beyond its total size, least
/// recently used first.
///
/// # Errors
///
/// When the repository's slot directory or its records cannot be written.
pub fn claim_in(root: &Path, repo: &Path, rule: Reclaim) -> io::Result<Claim> {
    let own = slots_of(root, repo);
    let lock = hold(&own)?;
    fs::write(
        own.join(OWNER),
        canonical(repo).to_string_lossy().as_bytes(),
    )?;
    reclaim(root, &own, rule);
    Ok(Claim {
        dir: own,
        _lock: lock,
    })
}

/// A shared lock on `dir`'s lock file, both made when missing; taken again when another claim
/// removed the directory while this one waited, so the lock held is the one `dir` holds.
fn hold(dir: &Path) -> io::Result<File> {
    let mut attempts = 0;
    loop {
        fs::create_dir_all(dir)?;
        let path = dir.join(LOCK);
        let file = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)?;
        file.lock_shared()?;
        attempts += 1;
        if attempts == HOLD_ATTEMPTS || is_file(&file, &path) {
            return Ok(file);
        }
    }
}

/// Whether the open `file` is the one at `path`.
#[cfg(unix)]
fn is_file(file: &File, path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (file.metadata(), fs::metadata(path)) {
        (Ok(open), Ok(named)) => open.dev() == named.dev() && open.ino() == named.ino(),
        _ => false,
    }
}

#[cfg(not(unix))]
fn is_file(_: &File, path: &Path) -> bool {
    path.exists()
}

/// An exclusive lock on `dir`'s lock file when no production holds it; `None` while one does, or
/// when the file is gone.
fn idle_lock(dir: &Path) -> Option<File> {
    let file = File::options().write(true).open(dir.join(LOCK)).ok()?;
    match file.try_lock() {
        Ok(()) => Some(file),
        Err(TryLockError::WouldBlock | TryLockError::Error(_)) => None,
    }
}

/// Another repository's slots, locked while this claim looks at them.
struct Idle {
    dir: PathBuf,
    used: SystemTime,
    bytes: u64,
    _lock: File,
}

/// Removes the slots under `root` other than `own` that no production holds: those of a gone
/// repository or unused for longer than `rule.idle`, then, while every repository's slots hold
/// more than `rule.total` bytes, the least recently used of those a locking production made.
fn reclaim(root: &Path, own: &Path, rule: Reclaim) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let now = SystemTime::now();
    let mut total = recorded(own);
    let mut idle = Vec::new();
    for entry in entries.flatten() {
        let dir = entry.path();
        if dir == own || !dir.is_dir() {
            continue;
        }
        // Slots a production made without taking a lock (an earlier Cupel) may be in use now:
        // only the gone and idle rules apply to them.
        let lock = if dir.join(LOCK).exists() {
            let Some(lock) = idle_lock(&dir) else {
                total += recorded(&dir);
                continue;
            };
            Some(lock)
        } else {
            None
        };
        let (owner, used) = owner(&dir);
        let gone = !owner.as_deref().is_some_and(Path::is_dir);
        let stale = now.duration_since(used).is_ok_and(|age| age > rule.idle);
        if gone || stale {
            // Another claim may be removing it too; what is left goes next time.
            let _ = fs::remove_dir_all(&dir);
            continue;
        }
        let bytes = recorded(&dir);
        total += bytes;
        if let Some(lock) = lock {
            idle.push(Idle {
                dir,
                used,
                bytes,
                _lock: lock,
            });
        }
    }
    idle.sort_by_key(|slots| slots.used);
    for slots in idle {
        if total <= rule.total {
            break;
        }
        if fs::remove_dir_all(&slots.dir).is_ok() {
            total = total.saturating_sub(slots.bytes);
        }
    }
}

/// The repository a slot directory belongs to, and when its slots were last used; the epoch
/// when it records none.
fn owner(dir: &Path) -> (Option<PathBuf>, SystemTime) {
    let marker = dir.join(OWNER);
    let owner = fs::read_to_string(&marker)
        .ok()
        .filter(|text| !text.is_empty())
        .map(PathBuf::from);
    let used = fs::metadata(&marker)
        .and_then(|meta| meta.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    (owner, used)
}

/// The bytes `dir`'s slots held when its last production ended; measured, and recorded, when no
/// production recorded them.
fn recorded(dir: &Path) -> u64 {
    if let Some(bytes) = fs::read_to_string(dir.join(SIZE))
        .ok()
        .and_then(|text| text.trim().parse().ok())
    {
        return bytes;
    }
    let bytes = measure(dir);
    // A size that cannot be written is measured again next time.
    let _ = fs::write(dir.join(SIZE), bytes.to_string());
    bytes
}

/// The bytes of every regular file under `dir`, symbolic links not followed: the apparent size.
pub fn measure(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => measure(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map_or(0, |meta| meta.len()),
            _ => 0,
        })
        .sum()
}

/// One repository's slots, as `cupel doctor` reports them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Usage {
    /// The repository they belong to; `None` when no production recorded it.
    pub repository: Option<PathBuf>,
    /// Whether the repository is still a directory.
    pub exists: bool,
    /// The bytes they hold now.
    pub bytes: u64,
    /// When a production last claimed them.
    pub used: SystemTime,
}

/// Every repository's slots under `root`, measured now, most recently used first.
pub fn usage(root: &Path) -> Vec<Usage> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut found: Vec<Usage> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|dir| dir.is_dir())
        .map(|dir| {
            let (repository, used) = owner(&dir);
            Usage {
                exists: repository.as_deref().is_some_and(Path::is_dir),
                repository,
                bytes: measure(&dir),
                used,
            }
        })
        .collect();
    found.sort_by(|a, b| b.used.cmp(&a.used));
    found
}
