// ABOUTME: Worker overlays (ADR-080 §8): persistent slots outside the repository that mirror its tree (clones or
// ABOUTME: hard links, ignored entries as symlinks), where mutants are written as private copies and restored.
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// What one path of a slot should be.
#[derive(Debug, PartialEq, Eq)]
enum Entry {
    Dir,
    /// A mirror of this file of the tree.
    File(PathBuf),
    /// A symlink to this path of the tree: something the tree ignores.
    Link(PathBuf),
}

/// One worker's mirror of the repository.
#[derive(Debug, Clone)]
pub struct Slot {
    root: PathBuf,
}

/// Where slot `index` of `lang` for the repository at `repo` lives: under the system temporary
/// directory, so no runner in the repository ever discovers it, and the same on every run, so
/// its builds are incremental.
pub fn slot_dir(repo: &Path, lang: &str, index: usize) -> PathBuf {
    let repo = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    let digest = blake3::hash(repo.to_string_lossy().as_bytes()).to_hex();
    std::env::temp_dir()
        .join("cupel-mutate")
        .join(&digest[..16])
        .join(lang)
        .join(index.to_string())
}

impl Slot {
    pub fn at(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Makes the slot mirror `tree`: what the tree's `.gitignore`s ignore becomes a symlink to
    /// the original, every other file a clone or hard link unless the slot already holds its
    /// bytes, and what the tree no longer has is removed. `.git` and `.cupel` are left out; the
    /// slot's own `.cupel` (runner reports) is kept.
    pub fn sync(&self, tree: &Path) -> io::Result<()> {
        fs::create_dir_all(&self.root)?;
        let wanted = wanted(tree)?;
        prune(&self.root, &self.root, &wanted)?;
        for (path, entry) in &wanted {
            self.place(path, entry)?;
        }
        Ok(())
    }

    /// Writes `bytes` as the slot's `path`, a private copy: any link is removed first, so the
    /// tree's file is never written, and the copy's modification time is now.
    pub fn write(&self, path: &str, bytes: &[u8]) -> io::Result<()> {
        let target = self.root.join(path);
        remove(&target)?;
        fs::write(&target, bytes)
    }

    fn place(&self, path: &Path, entry: &Entry) -> io::Result<()> {
        let at = self.root.join(path);
        match entry {
            Entry::Dir => fs::create_dir_all(&at),
            Entry::Link(target) => {
                if fs::read_link(&at).ok().as_deref() == Some(target.as_path()) {
                    return Ok(());
                }
                remove(&at)?;
                link(target, &at)
            }
            Entry::File(source) => {
                if same_file(source, &at) {
                    return Ok(());
                }
                remove(&at)?;
                mirror(source, &at)
            }
        }
    }
}

/// Every path a slot of `tree` should hold, relative to it.
fn wanted(tree: &Path) -> io::Result<BTreeMap<PathBuf, Entry>> {
    let mut kept: BTreeMap<PathBuf, Entry> = BTreeMap::new();
    let walk = ignore::WalkBuilder::new(tree)
        .hidden(false)
        .require_git(false)
        .git_global(false)
        .git_exclude(false)
        .parents(false)
        .filter_entry(|entry| !left_out(entry.file_name()))
        .build();
    for entry in walk.flatten() {
        let Ok(relative) = entry.path().strip_prefix(tree) else {
            continue;
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        let is_dir = entry.file_type().is_some_and(|kind| kind.is_dir());
        let mirrored = if is_dir {
            Entry::Dir
        } else {
            Entry::File(entry.path().to_path_buf())
        };
        kept.insert(relative.to_path_buf(), mirrored);
    }
    let dirs: Vec<PathBuf> = std::iter::once(PathBuf::new())
        .chain(
            kept.iter()
                .filter(|(_, entry)| **entry == Entry::Dir)
                .map(|(path, _)| path.clone()),
        )
        .collect();
    for dir in dirs {
        for child in fs::read_dir(tree.join(&dir))?.flatten() {
            let relative = dir.join(child.file_name());
            if !left_out(&child.file_name()) && !kept.contains_key(&relative) {
                kept.insert(relative, Entry::Link(child.path()));
            }
        }
    }
    Ok(kept)
}

/// Directories no slot holds: the tree's history and Cupel's own state.
fn left_out(name: &std::ffi::OsStr) -> bool {
    name == ".git" || name == ".cupel"
}

/// Removes everything under `dir` (a directory of the slot at `root`) that `wanted` does not
/// hold as the same kind of entry, keeping the slot's own `.cupel`.
fn prune(root: &Path, dir: &Path, wanted: &BTreeMap<PathBuf, Entry>) -> io::Result<()> {
    for child in fs::read_dir(dir)?.flatten() {
        let path = child.path();
        let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        let kind = child.file_type()?;
        let keep = match wanted.get(&relative) {
            Some(Entry::Dir) => kind.is_dir(),
            Some(Entry::Link(_)) => kind.is_symlink(),
            Some(Entry::File(_)) => kind.is_file(),
            None => relative.as_os_str() == ".cupel",
        };
        if !keep {
            remove(&path)?;
        } else if kind.is_dir() && relative.as_os_str() != ".cupel" {
            prune(root, &path, wanted)?;
        }
    }
    Ok(())
}

/// Whether the slot's `at` already mirrors `source`: the same file, or the same bytes.
fn same_file(source: &Path, at: &Path) -> bool {
    let (Ok(theirs), Ok(ours)) = (fs::metadata(source), fs::symlink_metadata(at)) else {
        return false;
    };
    if !ours.is_file() || theirs.len() != ours.len() {
        return false;
    }
    same_inode(&theirs, &ours) || fs::read(source).ok() == fs::read(at).ok()
}

#[cfg(unix)]
fn same_inode(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    a.dev() == b.dev() && a.ino() == b.ino()
}

#[cfg(not(unix))]
fn same_inode(_: &fs::Metadata, _: &fs::Metadata) -> bool {
    false
}

/// Places a mirror of `source` at `at`: a clone where the filesystem makes one (`clonefile` on
/// macOS), else a hard link, else a copy. A clone or copy is dated now; a link shares the tree's
/// file and its time.
fn mirror(source: &Path, at: &Path) -> io::Result<()> {
    if cfg!(target_os = "macos") || fs::hard_link(source, at).is_err() {
        fs::copy(source, at)?;
        fs::File::options()
            .write(true)
            .open(at)?
            .set_modified(std::time::SystemTime::now())?;
    }
    Ok(())
}

#[cfg(unix)]
fn link(target: &Path, at: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, at)
}

#[cfg(not(unix))]
fn link(target: &Path, at: &Path) -> io::Result<()> {
    if target.is_dir() {
        std::os::windows::fs::symlink_dir(target, at)
    } else {
        std::os::windows::fs::symlink_file(target, at)
    }
}

/// Removes whatever is at `path`; nothing there is fine.
fn remove(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
