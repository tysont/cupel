// ABOUTME: Locates the fixture repositories under `repos/` and copies one into a scratch directory,
// ABOUTME: so a run neither writes into the source tree nor sees the enclosing git repository.
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The fixture repository `name`, such as `detect/monorepo`, in this crate's `repos/` directory.
pub fn repo_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("repos")
        .join(name)
}

/// Copies the fixture repository `name` into the existing directory `into`.
///
/// # Panics
///
/// When the fixture does not exist or cannot be copied.
pub fn copy_repo(name: &str, into: &Path) {
    let from = repo_path(name);
    copy_tree(&from, into)
        .unwrap_or_else(|error| panic!("cannot copy fixture {}: {error}", from.display()));
}

/// Copies the tree at `from` into `into`, following symbolic links, so a fixture that links to
/// shared content (the reference plugins, ADR-092) carries a real copy of it.
fn copy_tree(from: &Path, into: &Path) -> io::Result<()> {
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = into.join(entry.file_name());
        if fs::metadata(entry.path())?.is_dir() {
            fs::create_dir_all(&target)?;
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
