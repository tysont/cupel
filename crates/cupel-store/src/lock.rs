// ABOUTME: The single-writer lock: an exclusive advisory lock on `.cupel/cupel.lock`, held for the
// ABOUTME: length of one write transaction and waited for up to a deadline (ADR-045 §3).
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::StoreError;

/// How long a writer waits for another to finish.
const WAIT: Duration = Duration::from_secs(5);
/// How often it tries again while waiting.
const RETRY: Duration = Duration::from_millis(10);

/// Held while a write transaction runs; dropping it releases the lock.
pub struct WriterLock {
    _file: File,
}

impl WriterLock {
    /// Takes the lock at `path`, creating the file if needed, waiting up to five seconds.
    pub fn acquire(path: &Path) -> Result<Self, StoreError> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)
            .map_err(|error| io(path, &error))?;
        let deadline = Instant::now() + WAIT;
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self { _file: file }),
                Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::sleep(RETRY);
                }
                Err(TryLockError::WouldBlock) => {
                    return Err(StoreError::Locked {
                        path: path.to_path_buf(),
                    });
                }
                Err(TryLockError::Error(error)) => return Err(io(path, &error)),
            }
        }
    }
}

fn io(path: &Path, error: &std::io::Error) -> StoreError {
    StoreError::Io {
        path: PathBuf::from(path),
        message: error.to_string(),
    }
}
