// ABOUTME: The file copies' unmutated baseline (ADR-185): how long the whole suite takes in a slot, measured once per
// ABOUTME: production and stored by a suite hash over the tests, the runner's project files and command, and Cupel.
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use cupel_core::{RunnerCommand, Writer};
use cupel_store::Store;

use crate::overlay;
use crate::plan::digest;

/// One production's baseline: what its suite hash covers beyond the tree, and its time once known.
pub struct Baseline<'a> {
    /// The suite hash of a mutant that runs the whole suite: its test IDs and test files.
    pub suite_hash: &'a str,
    /// The engine, the runner, and its tool's version.
    pub toolchain_hash: &'a str,
    elapsed: OnceLock<Duration>,
}

impl<'a> Baseline<'a> {
    pub fn new(suite_hash: &'a str, toolchain_hash: &'a str) -> Self {
        Self {
            suite_hash,
            toolchain_hash,
            elapsed: OnceLock::new(),
        }
    }

    /// How long the baseline took, once this production measured or read it.
    pub fn elapsed(&self) -> Option<Duration> {
        self.elapsed.get().copied()
    }

    /// Keeps `elapsed` as the production's baseline time.
    pub fn keep(&self, elapsed: Duration) -> Duration {
        *self.elapsed.get_or_init(|| elapsed)
    }

    /// The key the baseline is stored under: the suite hash, the toolchain, Cupel's version, the
    /// content of every file the slots mirror under the directories `commands` run in (the whole
    /// tree at `root` without any), the commands themselves, and the test threads each run gets.
    /// What a test loads is not known without running it; the files of its project roots hold
    /// it. `None` when those files cannot be read.
    pub fn key(
        &self,
        root: &Path,
        commands: &[RunnerCommand],
        outputs: &[&str],
        threads: u32,
    ) -> Option<String> {
        let mut dirs: Vec<&str> = commands
            .iter()
            .map(|command| command.dir.as_str())
            .collect();
        if dirs.is_empty() {
            dirs.push(".");
        }
        let files = overlay::content_hash(root, &dirs, outputs).ok()?;
        let threads = threads.to_string();
        let mut parts = vec![
            self.suite_hash,
            self.toolchain_hash,
            env!("CARGO_PKG_VERSION"),
            &files,
            &threads,
        ];
        for command in commands {
            parts.extend([command.dir.as_str(), command.command.as_str()]);
        }
        Some(digest(&parts))
    }
}

/// The baseline time stored under `key` in the store of `writer`'s repository.
pub fn stored(writer: &Writer, key: &str) -> Option<Duration> {
    let store = Store::open_existing(writer).ok()??;
    let elapsed = store.mutation_baseline(key).ok()??;
    Some(Duration::from_millis(elapsed))
}

/// Stores `elapsed` under `key` in the store of `writer`'s repository. A store that cannot be
/// written (a dry run's) keeps nothing, and the next production measures it again.
pub fn store(writer: &Writer, key: &str, elapsed: Duration) {
    if let Ok(store) = Store::open(writer) {
        let ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
        let _ = store.put_mutation_baseline(key, ms);
    }
}
