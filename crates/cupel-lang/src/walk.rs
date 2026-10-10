// ABOUTME: The file walk: every source file of a registered language under a root, in parallel,
// ABOUTME: honoring `.gitignore` (inside or outside a git repository) and skipping hidden entries.
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use ignore::{WalkBuilder, WalkState};

use crate::language::LanguageRegistry;

/// Source files under `root` that some language in `languages` owns, relative to `root`,
/// `/`-separated, sorted.
pub fn walk(root: &Path, languages: &LanguageRegistry) -> Vec<String> {
    let found = Mutex::new(Vec::new());
    WalkBuilder::new(root)
        .require_git(false)
        .build_parallel()
        .run(|| {
            Box::new(|entry| {
                let Ok(entry) = entry else {
                    return WalkState::Continue;
                };
                if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                    return WalkState::Continue;
                }
                let Some(relative) = relative(root, entry.path()) else {
                    return WalkState::Continue;
                };
                if languages.for_path(&relative).is_some() {
                    found
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .push(relative);
                }
                WalkState::Continue
            })
        });
    let mut files = found.into_inner().unwrap_or_else(PoisonError::into_inner);
    files.sort();
    files
}

/// `path` relative to `root`, with `/` separators; `None` for a non-UTF-8 path.
pub(crate) fn relative(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let parts: Option<Vec<&str>> = relative
        .components()
        .map(|component| component.as_os_str().to_str())
        .collect();
    Some(parts?.join("/"))
}
