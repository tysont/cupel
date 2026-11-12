// ABOUTME: A fact shelf in memory: what the fact store keeps of parsed files between processes, held in a
// ABOUTME: map so tests can give it to one parse cache after another and count what was kept.
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use cupel_lang::{FactShelf, ParseCache};

/// Each file's derived values by file key and name, as a store would keep them.
#[derive(Default)]
pub struct MemoryShelf {
    files: Mutex<BTreeMap<String, BTreeMap<String, Vec<u8>>>>,
}

impl MemoryShelf {
    /// How many values are kept, over every file.
    pub fn kept(&self) -> usize {
        self.files().values().map(BTreeMap::len).sum()
    }

    /// A fresh parse cache, as a new process makes it, that keeps its values on `shelf`.
    pub fn cache(shelf: &Arc<Self>) -> Arc<ParseCache> {
        let cache = Arc::new(ParseCache::default());
        cache.shelve(Arc::clone(shelf) as Arc<dyn FactShelf>);
        cache
    }

    fn files(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, BTreeMap<String, Vec<u8>>>> {
        self.files.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl FactShelf for MemoryShelf {
    fn load(&self, key: &str) -> Vec<(String, Vec<u8>)> {
        self.files()
            .get(key)
            .map(|values| values.clone().into_iter().collect())
            .unwrap_or_default()
    }

    fn keep(&self, key: &str, name: &str, payload: Vec<u8>) {
        self.files()
            .entry(key.to_owned())
            .or_default()
            .insert(name.to_owned(), payload);
    }
}
