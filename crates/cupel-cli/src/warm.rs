// ABOUTME: The daemon's warm builds (ADR-155 §6): once the watcher's changes settle and no request runs, every ready
// ABOUTME: suite joint builds what the next run would build first, holding the machine-wide heavy-command lock.
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cupel_core::config::{Config, ConfigRequest};
use cupel_core::syntax::ProjectInfo;
use cupel_core::{Availability, Cancellation, Ctx, ProduceCtx, Produced, Writer};

use crate::catalog::Catalog;
use crate::project;

/// How long the watcher's changes must have stopped before a warm build starts.
const SETTLE: Duration = Duration::from_secs(2);

/// How often a warm build that waits (on changes still coming, a request, the lock) looks again.
const RETRY: Duration = Duration::from_millis(250);

/// The variable naming the machine-wide heavy-command lock, as `scripts/heavy.sh` reads it.
const LOCK_VARIABLE: &str = "CUPEL_HEAVY_LOCK";

/// Builds ahead of the daemon's runs on a thread of its own until `stop` fires.
pub struct Warmer {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

struct Shared {
    root: PathBuf,
    pending: Mutex<Pending>,
    wake: Condvar,
    /// The requests running now.
    running: AtomicUsize,
    stop: Cancellation,
}

/// The changed paths no warm build has seen yet, and when the last arrived.
#[derive(Default)]
struct Pending {
    changed: Vec<PathBuf>,
    last: Option<Instant>,
}

/// A request running in the daemon; while one is held, no warm build starts.
pub struct Running<'a>(&'a AtomicUsize);

impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Warmer {
    /// Warms the repository at `root` after the changes it is told of, until `stop` fires.
    pub fn start(root: PathBuf, stop: Cancellation) -> Self {
        let shared = Arc::new(Shared {
            root,
            pending: Mutex::default(),
            wake: Condvar::new(),
            running: AtomicUsize::new(0),
            stop,
        });
        let worker = Arc::clone(&shared);
        Self {
            shared,
            thread: Some(thread::spawn(move || worker.serve())),
        }
    }

    /// `paths` changed under the root.
    pub fn changed(&self, paths: &[PathBuf]) {
        let mut pending = self.shared.lock();
        pending.changed.extend(paths.iter().cloned());
        pending.last = Some(Instant::now());
        self.shared.wake.notify_all();
    }

    /// Marks a request as running until the guard is dropped.
    pub fn running(&self) -> Running<'_> {
        self.shared.running.fetch_add(1, Ordering::SeqCst);
        Running(&self.shared.running)
    }
}

impl Drop for Warmer {
    /// Stops the thread, cancelling a warm build in flight, and waits for it.
    fn drop(&mut self) {
        self.shared.stop.cancel();
        self.shared.wake.notify_all();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, Pending> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Waits for changes that have settled while no request runs and the heavy-command lock is
    /// free, then warms with them, holding the lock; until `stop` fires.
    fn serve(&self) {
        while !self.stop.is_cancelled() {
            let Some((changed, _lock)) = self.next() else {
                continue;
            };
            warm(&self.root, &changed, &self.stop);
        }
    }

    /// The changes to warm with and the heavy-command lock, when a warm build may start now;
    /// otherwise `None` after waiting a moment.
    fn next(&self) -> Option<(Vec<PathBuf>, File)> {
        let pending = self.lock();
        let settled = pending.last.is_some_and(|last| last.elapsed() >= SETTLE);
        if !settled || self.running.load(Ordering::SeqCst) > 0 {
            drop(self.wake.wait_timeout(pending, RETRY));
            return None;
        }
        let Some(lock) = heavy_lock() else {
            drop(self.wake.wait_timeout(pending, RETRY));
            return None;
        };
        let mut pending = pending;
        let changed = std::mem::take(&mut pending.changed);
        pending.last = None;
        Some((changed, lock))
    }
}

/// The machine-wide heavy-command lock (`CUPEL_HEAVY_LOCK`, else `cupel-heavy.lock` in `/tmp`),
/// taken without waiting, as `scripts/heavy.sh` takes it for a command of default priority: not
/// while a command holds it, nor while a command of first priority waits on its turnstile.
fn heavy_lock() -> Option<File> {
    let path = std::env::var_os(LOCK_VARIABLE).map_or_else(default_lock, PathBuf::from);
    let mut turnstile = path.clone().into_os_string();
    turnstile.push(".turnstile");
    let turnstile = open(Path::new(&turnstile))?;
    turnstile.try_lock().ok()?;
    let lock = open(&path)?;
    lock.try_lock().ok()?;
    Some(lock)
}

#[cfg(unix)]
fn default_lock() -> PathBuf {
    PathBuf::from("/tmp/cupel-heavy.lock")
}

#[cfg(not(unix))]
fn default_lock() -> PathBuf {
    std::env::temp_dir().join("cupel-heavy.lock")
}

fn open(path: &Path) -> Option<File> {
    OpenOptions::new().create(true).append(true).open(path).ok()
}

/// Every ready suite joint of every language warms the repository at `root` for `changed`, with
/// its configuration and project roots as a run would see them; a joint that fails to build is
/// left for the run to report.
fn warm(root: &Path, changed: &[PathBuf], cancel: &Cancellation) {
    let Ok(config) = Config::load(&ConfigRequest {
        root: root.to_path_buf(),
        ..ConfigRequest::default()
    }) else {
        return;
    };
    let Ok(writer) = Writer::new(root, false) else {
        return;
    };
    let ctx = Ctx {
        exclude: config.file.scope.exclude.clone(),
        lang: config.file.lang.clone(),
        ..Ctx::new(root.to_path_buf())
    };
    let catalog = Catalog::builtin();
    let info = project::detect(&writer, &ctx.exclude);
    for lang in catalog.language_registry().langs() {
        for joint in catalog.producers().joints_for(&lang) {
            if cancel.is_cancelled() || !matches!(joint.availability(&ctx), Availability::Ready) {
                continue;
            }
            let mut produce = ProduceCtx::new(ctx.clone(), writer.confined(), cancel.clone());
            produce.config = config.lang_config(&lang.id);
            let roots = info
                .roots
                .iter()
                .filter(|found| found.lang == lang.id)
                .cloned()
                .collect();
            produce
                .upstream
                .insert(Produced::new(ProjectInfo { roots }).share());
            let _ = joint.warm(&produce, changed);
        }
    }
}
