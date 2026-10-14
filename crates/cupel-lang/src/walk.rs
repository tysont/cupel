// ABOUTME: The file walk: every source file of a registered language under a root, in parallel, honoring
// ABOUTME: `.gitignore` (in or out of a git repository), skipping hidden and project-generated directories.
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use cupel_core::Exclude;
use cupel_core::syntax::ProjectInfo;
use ignore::{WalkBuilder, WalkState};

use crate::language::LanguageRegistry;

/// Source files under `root` that some language in `languages` owns, relative to `root`,
/// `/`-separated, sorted. Below each root of `project`, directories named in its `skip_dirs`
/// are skipped (ADR-037); so are the directories every run excludes (ADR-065).
pub fn walk(root: &Path, languages: &LanguageRegistry, project: &ProjectInfo) -> Vec<String> {
    walk_excluding(root, languages, project, &Exclude::default())
}

/// [`walk`], never entering or returning a path `exclude` excludes (`[scope] exclude`).
pub fn walk_excluding(
    root: &Path,
    languages: &LanguageRegistry,
    project: &ProjectInfo,
    exclude: &Exclude,
) -> Vec<String> {
    let found = Mutex::new(Vec::new());
    let base = root.to_path_buf();
    let project = project.clone();
    let excluded = exclude.clone();
    WalkBuilder::new(root)
        .require_git(false)
        .filter_entry(move |entry| {
            let Some(path) = relative(&base, entry.path()) else {
                return true;
            };
            let is_dir = entry.file_type().is_some_and(|kind| kind.is_dir());
            let kept = path.is_empty() || !excluded.excludes(&path);
            kept && !(is_dir && generated(&project, &path))
        })
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

/// Whether `dir` (relative, `/`-separated) is named in the `skip_dirs` of a project root above it.
fn generated(project: &ProjectInfo, dir: &str) -> bool {
    let (parent, name) = dir.rsplit_once('/').unwrap_or((".", dir));
    project.roots.iter().any(|root| {
        let below = root.path == "."
            || parent == root.path
            || parent.starts_with(&format!("{}/", root.path));
        below && root.skip_dirs.iter().any(|skip| skip == name)
    })
}
