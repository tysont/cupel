// ABOUTME: The fact store's error type: why opening, locking, reading, or writing `.cupel/cupel.db` failed.
// ABOUTME: Its own module so the lock and the store both use it without depending on each other.
use std::path::PathBuf;

/// Why the store could not do what was asked.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The `Writer` refused `.cupel/` (ADR-020).
    #[error(transparent)]
    Write(#[from] cupel_core::Error),
    /// A dry run's writer cannot open the store for writing; use `Store::open_existing`.
    #[error("a dry run does not open the fact store for writing")]
    DryRun,
    /// Another process held the writer lock for too long.
    #[error("the fact store is locked by another writer (`{}`)", .path.display())]
    Locked { path: PathBuf },
    /// The store was migrated by a newer Cupel.
    #[error("the fact store has schema version {found}, newer than this cupel's {known}")]
    TooNew { found: u32, known: u32 },
    #[error("cannot use `{}`: {message}", .path.display())]
    Io { path: PathBuf, message: String },
    #[error("fact store: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// A stored JSON document no longer parses as its type.
    #[error("fact store holds unreadable JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// A stored value no longer parses as its type.
    #[error("fact store holds an invalid value: {0}")]
    Value(String),
}
