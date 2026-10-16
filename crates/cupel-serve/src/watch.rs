// ABOUTME: The file watcher behind `serve --watch`: `notify` events under the root, gathered until they stop
// ABOUTME: for a moment, handed over as one list of changed paths on the watcher's own thread.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// How long the events of one burst may be apart: an editor's save, a `git checkout`.
const QUIET: Duration = Duration::from_millis(50);

/// Watches a directory tree until dropped.
pub struct Watch {
    watcher: Option<RecommendedWatcher>,
    thread: Option<JoinHandle<()>>,
}

impl Watch {
    /// Watches `root` recursively, calling `on_change` with the paths each burst of events
    /// touched (reads excluded), in order, on a thread of its own.
    pub fn start(
        root: &Path,
        on_change: impl Fn(Vec<PathBuf>) + Send + 'static,
    ) -> notify::Result<Self> {
        let (events, received) = mpsc::channel::<notify::Result<Event>>();
        let mut watcher = notify::recommended_watcher(events)?;
        watcher.watch(root, RecursiveMode::Recursive)?;
        let thread = thread::spawn(move || {
            while let Ok(first) = received.recv() {
                let mut paths = BTreeSet::new();
                add(first, &mut paths);
                while let Ok(next) = received.recv_timeout(QUIET) {
                    add(next, &mut paths);
                }
                if !paths.is_empty() {
                    on_change(paths.into_iter().collect());
                }
            }
        });
        Ok(Self {
            watcher: Some(watcher),
            thread: Some(thread),
        })
    }
}

impl Drop for Watch {
    /// Stops watching, then waits for the thread to hand over what it holds.
    fn drop(&mut self) {
        drop(self.watcher.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The paths `event` touched, unless it only read them.
fn add(event: notify::Result<Event>, paths: &mut BTreeSet<PathBuf>) {
    if let Ok(event) = event
        && !matches!(event.kind, EventKind::Access(_))
    {
        paths.extend(event.paths);
    }
}
