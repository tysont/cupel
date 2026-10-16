// ABOUTME: `Cancellation`, the cooperative token behind `--timeout`: a shared flag plus a deadline
// ABOUTME: that checks and producers poll between units of work (ADR-020 §5).
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::error::{Error, Result};

/// The longest `wait` sleeps before looking at the token again.
const WAIT_SLICE: Duration = Duration::from_millis(10);

/// Tells long-running work to stop. Clones share one flag, so cancelling any clone cancels all.
#[derive(Debug, Clone, Default)]
pub struct Cancellation {
    cancelled: Arc<AtomicBool>,
    deadline: Option<Instant>,
    /// A token whose cancellation cancels this one too (see [`child`](Self::child)).
    parent: Option<Box<Cancellation>>,
}

impl Cancellation {
    /// A token that is cancelled only by [`cancel`](Self::cancel).
    pub fn new() -> Self {
        Self::default()
    }

    /// A token that is also cancelled once `deadline` passes.
    pub fn with_deadline(deadline: Instant) -> Self {
        Self {
            cancelled: Arc::default(),
            deadline: Some(deadline),
            parent: None,
        }
    }

    /// A token cancelled when this one is, or once `timeout` passes; cancelling it, or its
    /// timeout passing, leaves this one alone (a runner's per-call timeout, ADR-077).
    #[must_use]
    pub fn child(&self, timeout: Duration) -> Self {
        Self {
            parent: Some(Box::new(self.clone())),
            ..Self::with_deadline(Instant::now() + timeout)
        }
    }

    /// A token cancelled when this one is, or once `deadline` (if any) passes; cancelling it
    /// leaves this one alone. A daemon's request runs under the daemon's token (ADR-085).
    #[must_use]
    pub fn under(&self, deadline: Option<Instant>) -> Self {
        Self {
            cancelled: Arc::default(),
            deadline,
            parent: Some(Box::new(self.clone())),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
            || self
                .parent
                .as_ref()
                .is_some_and(|parent| parent.is_cancelled())
        {
            self.cancel();
        }
        self.cancelled.load(Ordering::SeqCst)
    }

    /// `Err(Error::Cancelled)` once the token is cancelled, for use with `?` between units of work.
    pub fn checkpoint(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }

    /// Sleeps for `duration`, waking early if the token is cancelled. True when the whole
    /// duration passed.
    pub fn wait(&self, duration: Duration) -> bool {
        let until = Instant::now() + duration;
        loop {
            if self.is_cancelled() {
                return false;
            }
            let now = Instant::now();
            if now >= until {
                return true;
            }
            std::thread::sleep((until - now).min(WAIT_SLICE));
        }
    }
}
