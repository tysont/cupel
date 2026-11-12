// ABOUTME: The fact store as the parse cache's shelf (ADR-181, ADR-188): a run reads each file's derived values
// ABOUTME: from the store by this build and the file's key, and keeps what it derives to record after the run.
use std::sync::{Mutex, OnceLock, PoisonError};

use cupel_core::{RunId, Writer};
use cupel_lang::FactShelf;
use cupel_store::{ParsedFact, Store};

/// The deflate level values are kept at: the fastest, which already shrinks their JSON about
/// ninefold.
const LEVEL: u8 = 1;

/// The store of one repository, opened on the first read, and the values derived since, each
/// deflated.
pub struct StoreShelf {
    writer: Writer,
    /// This binary's build (version and executable, as the daemon's, ADR-085 §4): values another
    /// build derived are never read.
    build: String,
    /// `None` when there is no store to read: a dry run's repository without one, or a store
    /// that cannot be opened.
    store: OnceLock<Option<Mutex<Store>>>,
    kept: Mutex<Vec<ParsedFact>>,
}

impl StoreShelf {
    /// The shelf of `writer`'s repository for this build; `None` when the build has no
    /// executable to tell it from another of the same version.
    pub fn new(writer: &Writer) -> Option<Self> {
        let build = crate::serve::build();
        if build.binary.is_empty() {
            return None;
        }
        Some(Self {
            writer: writer.clone(),
            build: build.to_string(),
            store: OnceLock::new(),
            kept: Mutex::default(),
        })
    }

    fn store(&self) -> Option<&Mutex<Store>> {
        self.store
            .get_or_init(|| {
                let store = if self.writer.is_recording() {
                    Store::open_existing(&self.writer).ok().flatten()
                } else {
                    Store::open(&self.writer).ok()
                };
                store.map(Mutex::new)
            })
            .as_ref()
    }

    /// Records the values kept since the last record, as derived in `run`.
    pub fn record(&self, run: &RunId) -> Result<(), String> {
        let kept = std::mem::take(&mut *lock(&self.kept));
        if kept.is_empty() {
            return Ok(());
        }
        let Some(store) = self.store() else {
            return Ok(());
        };
        lock(store)
            .record_parsed_facts(&self.build, run, &kept)
            .map_err(|error| error.to_string())
    }
}

impl FactShelf for StoreShelf {
    /// The values kept for `key`; one that does not inflate is left out, so it is derived again.
    fn load(&self, key: &str) -> Vec<(String, Vec<u8>)> {
        let stored = self
            .store()
            .and_then(|store| lock(store).parsed_facts(&self.build, key).ok())
            .unwrap_or_default();
        stored
            .into_iter()
            .filter_map(|(name, payload)| {
                let payload = miniz_oxide::inflate::decompress_to_vec(&payload).ok()?;
                Some((name, payload))
            })
            .collect()
    }

    fn keep(&self, key: &str, name: &str, payload: Vec<u8>) {
        let payload = miniz_oxide::deflate::compress_to_vec(&payload, LEVEL);
        lock(&self.kept).push(ParsedFact {
            key: key.to_owned(),
            name: name.to_owned(),
            payload,
        });
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kept_value_is_recorded_compressed_and_a_later_shelf_loads_it_as_kept() {
        let dir = tempfile::tempdir().unwrap();
        let writer = Writer::new(dir.path(), false).unwrap();
        let value = br#"{"label":"call","children":[],"span":{"line":1,"col":1}}"#.repeat(200);
        let first = StoreShelf::new(&writer).unwrap();
        first.keep("file-a", "norm-tree", value.clone());
        first.record(&RunId::generate()).unwrap();

        let stored = Store::open(&writer)
            .unwrap()
            .parsed_facts(&first.build, "file-a")
            .unwrap();
        assert!(
            stored[0].1.len() * 4 < value.len(),
            "{} bytes stored of {}",
            stored[0].1.len(),
            value.len()
        );
        let later = StoreShelf::new(&writer).unwrap();
        assert_eq!(later.load("file-a"), [("norm-tree".to_owned(), value)]);
        assert_eq!(later.load("file-b"), []);
    }
}
