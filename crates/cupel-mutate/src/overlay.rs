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

impl Slot {
    pub fn at(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Makes the slot mirror `tree`: what the tree's `.gitignore`s ignore becomes a symlink to
    /// the original, every other file a clone or hard link unless the slot already holds its
    /// bytes (a copy dated now where it held other bytes), and what the tree no longer has is
    /// removed. `.git`, `.cupel`, and directories named
    /// in `outputs` (a runner's build directories) are left out of the mirror; the slot's own
    /// are kept, so its builds are its own and incremental.
    pub fn sync(&self, tree: &Path, outputs: &[&str]) -> io::Result<()> {
        fs::create_dir_all(&self.root)?;
        let own = Own(outputs.iter().map(|name| (*name).to_owned()).collect());
        let wanted = wanted(tree, &own)?;
        prune(&self.root, &self.root, &wanted, &own)?;
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
                let replaced = fs::symlink_metadata(&at).is_ok();
                remove(&at)?;
                if replaced {
                    copy_now(source, &at)
                } else {
                    mirror(source, &at)
                }
            }
        }
    }
}

/// What a slot keeps for itself: the repository's `.cupel` (a `.cupel` deeper in the tree is
/// the tree's, such as a fixture's committed cache), any `.git`, and the runner's build
/// directories.
#[derive(Clone)]
struct Own(Vec<String>);

impl Own {
    /// Whether the path `relative` to the root is the slot's own.
    fn holds(&self, relative: &Path) -> bool {
        let name = relative.file_name().unwrap_or_default();
        relative == Path::new(".cupel")
            || name == ".git"
            || self.0.iter().any(|output| name == output.as_str())
    }
}

/// Every path a slot of `tree` should hold, relative to it.
fn wanted(tree: &Path, own: &Own) -> io::Result<BTreeMap<PathBuf, Entry>> {
    let mut kept = mirrored(tree, own)?;
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
            if !own.holds(&relative) && !kept.contains_key(&relative) {
                kept.insert(relative, Entry::Link(child.path()));
            }
        }
    }
    Ok(kept)
}

/// What the walk of `tree` keeps (its `.gitignore`s honoured, what the slot `own`s left out),
/// relative to it: directories, files, and the tree's own symlinks.
fn mirrored(tree: &Path, own: &Own) -> io::Result<BTreeMap<PathBuf, Entry>> {
    let filter = own.clone();
    let root = tree.to_path_buf();
    let walk = ignore::WalkBuilder::new(tree)
        .hidden(false)
        .require_git(false)
        .git_global(false)
        .git_exclude(false)
        .parents(false)
        .filter_entry(move |entry| {
            !filter.holds(entry.path().strip_prefix(&root).unwrap_or(entry.path()))
        })
        .build();
    let mut kept = BTreeMap::new();
    for entry in walk.flatten() {
        let relative = entry.path().strip_prefix(tree).unwrap_or(entry.path());
        if !relative.as_os_str().is_empty() {
            kept.insert(relative.to_path_buf(), entry_of(&entry)?);
        }
    }
    Ok(kept)
}

/// How a slot mirrors `entry` of the tree.
fn entry_of(entry: &ignore::DirEntry) -> io::Result<Entry> {
    Ok(if entry.path_is_symlink() {
        // The same link, so a relative one points into the slot as it does in the tree.
        Entry::Link(fs::read_link(entry.path())?)
    } else if entry.file_type().is_some_and(|kind| kind.is_dir()) {
        Entry::Dir
    } else {
        Entry::File(entry.path().to_path_buf())
    })
}

/// Removes everything under `dir` (a directory of the slot at `root`) that `wanted` does not
/// hold as the same kind of entry, keeping what the slot `own`s.
fn prune(root: &Path, dir: &Path, wanted: &BTreeMap<PathBuf, Entry>, own: &Own) -> io::Result<()> {
    for child in fs::read_dir(dir)?.flatten() {
        let path = child.path();
        let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        let kind = child.file_type()?;
        let owned = own.holds(&relative);
        let keep = match wanted.get(&relative) {
            Some(Entry::Dir) => kind.is_dir(),
            Some(Entry::Link(_)) => kind.is_symlink(),
            Some(Entry::File(_)) => kind.is_file(),
            None => owned && kind.is_dir(),
        };
        if !keep {
            remove(&path)?;
        } else if kind.is_dir() && !owned {
            prune(root, &path, wanted, own)?;
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
        copy_now(source, at)?;
    }
    Ok(())
}

/// Places a copy of `source` at `at` (a clone on macOS) dated now: what replaces a slot file
/// that held something else, such as a mutant a stopped run left, so a build that saw that
/// rebuilds (ADR-141 §3); a hard link would carry the tree's older time.
fn copy_now(source: &Path, at: &Path) -> io::Result<()> {
    fs::copy(source, at)?;
    fs::File::options()
        .write(true)
        .open(at)?
        .set_modified(std::time::SystemTime::now())
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
