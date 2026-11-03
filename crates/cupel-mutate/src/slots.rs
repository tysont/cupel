// ABOUTME: Where mutation slots live (ADR-080 §8, ADR-159): in the repository's own `.cupel/work/slots/`, one directory per
// ABOUTME: language and slot, kept between runs so builds are incremental, and measured for `cupel doctor`.
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use cupel_core::Writer;

/// The directory of the repository at `repo`'s slots: `.cupel/work/slots/`. `.cupel/.gitignore` leaves it out of
/// version control, and the OS never prunes it on its own schedule, as it prunes a temporary directory.
pub fn root(repo: &Path) -> PathBuf {
    repo.join(Writer::DIR).join("work").join("slots")
}

/// Where slot `index` of `lang` for the repository at `repo` lives: the same on every run, so its builds are
/// incremental.
pub fn slot_dir(repo: &Path, lang: &str, index: usize) -> PathBuf {
    root(repo).join(lang).join(index.to_string())
}

/// A production's slots: the repository's slot directory, made when missing.
#[derive(Debug)]
pub struct Claim {
    dir: PathBuf,
}

impl Claim {
    /// The directory of the repository's slots.
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

/// Claims the slots of the repository at `repo`. Each repository's slots are its own, so no claim touches
/// another's.
///
/// # Errors
///
/// When the repository's slot directory cannot be made.
pub fn claim(repo: &Path) -> io::Result<Claim> {
    let dir = root(repo);
    fs::create_dir_all(&dir)?;
    Ok(Claim { dir })
}

/// The bytes of every regular file under `dir`, symbolic links not followed: the apparent size.
pub fn measure(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter_map(|entry| Some((entry.metadata().ok()?, entry)))
        .map(|(meta, entry)| {
            if meta.is_dir() {
                measure(&entry.path())
            } else if meta.is_file() {
                meta.len()
            } else {
                0
            }
        })
        .sum()
}
