// ABOUTME: What outlives one invocation (ADR-085 §3): a cold process's own, used once, or the daemon's, shared by
// ABOUTME: every request it serves; `Stream`, where an invocation writes as it goes; and `Input`, what it reads.
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use cupel_lang::{ParseCache, Reparse};

/// The caches an invocation may find warm: the parse cache, trees by content hash and grammar.
#[derive(Default)]
pub struct Session {
    cache: Arc<ParseCache>,
}

impl Session {
    pub fn cache(&self) -> &Arc<ParseCache> {
        &self.cache
    }

    /// Reparses each of `paths` the cache parsed before from its previous tree; how many changed.
    pub fn reparse(&self, paths: &[PathBuf]) -> u64 {
        let reparsed = paths
            .iter()
            .filter(|path| self.cache.reparse(path) == Reparse::Reparsed)
            .count();
        u64::try_from(reparsed).unwrap_or(u64::MAX)
    }

    /// Drops the trees no existing file last parsed to, after a request.
    pub fn sweep(&self) {
        self.cache.sweep();
    }
}

/// A destination for text written while the invocation runs: this process's standard error or
/// standard output, or, in the daemon, the client's connection. Clones write to the same
/// destination.
#[derive(Clone)]
pub struct Stream(Arc<Mutex<Box<dyn Write + Send>>>);

impl Stream {
    /// This process's standard error.
    pub fn stderr() -> Self {
        Self::new(io::stderr())
    }

    pub fn new(out: impl Write + Send + 'static) -> Self {
        Self(Arc::new(Mutex::new(Box::new(out))))
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .flush()
    }
}

/// What an invocation may read as its input: this process's standard input, which only
/// `serve --mcp` reads (ADR-121), or nothing. Clones share it; the first to take it gets it.
#[derive(Clone, Default)]
pub struct Input(Arc<Mutex<Option<Box<dyn Read + Send>>>>);

impl Input {
    /// This process's standard input.
    pub fn stdin() -> Self {
        Self::new(io::stdin())
    }

    pub fn new(input: impl Read + Send + 'static) -> Self {
        Self(Arc::new(Mutex::new(Some(Box::new(input)))))
    }

    /// The input, once; empty after it has been taken, and for an invocation that has none.
    pub fn take(&self) -> Box<dyn Read + Send> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .unwrap_or_else(|| Box::new(io::empty()))
    }
}
