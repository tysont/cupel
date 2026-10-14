// ABOUTME: Freshness of coverage reports (ADR-077): what every file under a project root hashed to when Cupel
// ABOUTME: made a report, and whether a cached report, or one the tool left in the tree, still describes it.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use cupel_core::Exclude;
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};

/// The file beside a cached report that records what it was made from.
pub(crate) const SOURCES: &str = "sources.json";
/// The cached report itself.
pub(crate) const REPORT: &str = "report";

/// The project root a report covers, as the freshness walk sees it.
pub(crate) struct Tree<'a> {
    /// The repository root, absolute.
    pub repo: &'a Path,
    /// The project root, relative to the repository root; `.` for itself.
    pub dir: &'a str,
    /// Directory names the ecosystem generates, skipped below the root.
    pub skip_dirs: &'a [String],
    pub exclude: &'a Exclude,
}

/// What a cached report was made from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Sources {
    /// The project root, relative to the repository root; `.` for itself.
    pub dir: String,
    /// The directory the tool ran in, absolute, which the report's absolute paths start with.
    pub ran_in: PathBuf,
    /// Directory names skipped below the root when the files were hashed.
    pub skip_dirs: Vec<String>,
    /// The report's blake3 hash.
    pub report: String,
    /// Every file under the root, relative to it, with its blake3 hash.
    pub files: BTreeMap<String, String>,
}

impl Sources {
    /// The record of `tree` now, for `report`, made by a tool that ran in `ran_in`.
    pub fn record(tree: &Tree<'_>, ran_in: &Path, report: &[u8]) -> Self {
        Self {
            dir: tree.dir.to_owned(),
            ran_in: ran_in.to_path_buf(),
            skip_dirs: tree.skip_dirs.to_vec(),
            report: hash(report),
            files: tree
                .files()
                .into_iter()
                .filter_map(|(name, path)| Some((name, hash(&fs::read(path).ok()?))))
                .collect(),
        }
    }

    /// The record in the cache entry `entry`, with when it was written; `None` when there is none.
    pub fn load(entry: &Path) -> Option<(Self, SystemTime)> {
        let path = entry.join(SOURCES);
        let written = fs::metadata(&path).and_then(|meta| meta.modified()).ok()?;
        let sources = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
        Some((sources, written))
    }

    /// Whether `report` is the report recorded and every file under the root is as recorded:
    /// the same files, each either not modified since `written` or hashing as it did.
    pub fn fresh(
        &self,
        repo: &Path,
        exclude: &Exclude,
        written: SystemTime,
        report: &[u8],
    ) -> bool {
        if hash(report) != self.report {
            return false;
        }
        let tree = Tree {
            repo,
            dir: &self.dir,
            skip_dirs: &self.skip_dirs,
            exclude,
        };
        let files = tree.files();
        files.len() == self.files.len()
            && files.iter().all(|(name, path)| {
                self.files.get(name).is_some_and(|recorded| {
                    modified(path).is_some_and(|at| at < written)
                        || fs::read(path).is_ok_and(|bytes| hash(&bytes) == *recorded)
                })
            })
    }
}

impl Tree<'_> {
    /// The root directory, absolute.
    pub fn root(&self) -> PathBuf {
        self.repo.join(self.dir)
    }

    /// Every file under the root, by its path relative to the root, `/`-separated: hidden
    /// entries, `.gitignore`d paths, `skip_dirs`, and excluded paths are left out.
    pub fn files(&self) -> BTreeMap<String, PathBuf> {
        let root = self.root();
        let prefix = if self.dir == "." {
            String::new()
        } else {
            format!("{}/", self.dir)
        };
        let skip = self.skip_dirs.to_vec();
        let exclude = self.exclude.clone();
        let base = root.clone();
        let mut files = BTreeMap::new();
        let walk = WalkBuilder::new(&root)
            .require_git(false)
            .filter_entry(move |entry| {
                let Some(name) = relative(&base, entry.path()) else {
                    return true;
                };
                let is_dir = entry.file_type().is_some_and(|kind| kind.is_dir());
                let generated = is_dir
                    && entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| skip.iter().any(|skip| skip == name));
                name.is_empty() || !(generated || exclude.excludes(&format!("{prefix}{name}")))
            })
            .build();
        for entry in walk.flatten() {
            if entry.file_type().is_some_and(|kind| kind.is_file())
                && let Some(name) = relative(&root, entry.path())
            {
                files.insert(name, entry.into_path());
            }
        }
        files
    }

    /// Whether the file at `report` was modified after every file under the root outside the
    /// directory holding it (a tool writes its other output there too).
    pub fn is_newest(&self, report: &Path) -> bool {
        let Some(made) = modified(report) else {
            return false;
        };
        let root = self.root();
        let beside = report.parent().filter(|dir| *dir != root.as_path());
        self.files()
            .values()
            .filter(|path| {
                path.as_path() != report && !beside.is_some_and(|dir| path.starts_with(dir))
            })
            .all(|path| modified(path).is_some_and(|at| at < made))
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|meta| meta.modified()).ok()
}

/// `path` relative to `root`, `/`-separated; `None` for a path outside it or not UTF-8.
fn relative(root: &Path, path: &Path) -> Option<String> {
    let parts: Option<Vec<&str>> = path
        .strip_prefix(root)
        .ok()?
        .components()
        .map(|part| part.as_os_str().to_str())
        .collect();
    Some(parts?.join("/"))
}

/// The blake3 hash of `bytes`, as hex.
pub(crate) fn hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    /// A project root with a source, a test, a hidden file, and a generated directory, and the
    /// record of it made now.
    fn recorded() -> (tempfile::TempDir, Sources, SystemTime) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for (path, text) in [
            ("src/lib.rs", "fn a() {}\n"),
            ("tests/a.rs", "#[test] fn t() {}\n"),
            (".hidden", "x"),
            ("target/out.o", "binary"),
        ] {
            fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
            fs::write(root.join(path), text).unwrap();
        }
        let sources = Sources::record(&tree(root), root, b"report");
        // As `sources.json` would be: written after the files, which then change later.
        std::thread::sleep(Duration::from_millis(20));
        let written = SystemTime::now();
        std::thread::sleep(Duration::from_millis(20));
        (dir, sources, written)
    }

    fn tree(root: &Path) -> Tree<'_> {
        static SKIP: std::sync::LazyLock<Vec<String>> =
            std::sync::LazyLock::new(|| vec!["target".to_owned()]);
        Tree {
            repo: root,
            dir: ".",
            skip_dirs: &SKIP,
            exclude: &EXCLUDE,
        }
    }

    static EXCLUDE: std::sync::LazyLock<Exclude> = std::sync::LazyLock::new(Exclude::default);

    #[test]
    fn a_record_lists_every_file_but_hidden_and_generated_ones() {
        let (_dir, sources, _) = recorded();
        let files: Vec<&str> = sources.files.keys().map(String::as_str).collect();
        assert_eq!(files, ["src/lib.rs", "tests/a.rs"]);
    }

    #[test]
    fn an_unchanged_tree_is_fresh_and_any_change_to_a_file_or_the_report_is_not() {
        let (dir, sources, written) = recorded();
        let root = dir.path();
        assert!(sources.fresh(root, &EXCLUDE, written, b"report"));
        assert!(!sources.fresh(root, &EXCLUDE, written, b"another report"));
        fs::write(root.join("target/out.o"), "rebuilt").unwrap();
        fs::write(root.join(".hidden"), "y").unwrap();
        assert!(
            sources.fresh(root, &EXCLUDE, written, b"report"),
            "ignored files"
        );

        fs::write(root.join("tests/a.rs"), "#[test] fn u() {}\n").unwrap();
        assert!(
            !sources.fresh(root, &EXCLUDE, written, b"report"),
            "a changed test"
        );
        let later = SystemTime::now() + Duration::from_secs(60);
        assert!(
            sources.fresh(root, &EXCLUDE, later, b"report"),
            "a file older than the record is trusted without rehashing"
        );
    }

    #[test]
    fn a_rewritten_file_with_the_same_content_stays_fresh_and_added_or_removed_files_do_not() {
        let (dir, sources, _) = recorded();
        let root = dir.path();
        let before = SystemTime::now() - Duration::from_secs(60);
        fs::write(root.join("src/lib.rs"), "fn a() {}\n").unwrap();
        assert!(
            sources.fresh(root, &EXCLUDE, before, b"report"),
            "rehashed, same content"
        );
        fs::write(root.join("src/new.rs"), "").unwrap();
        assert!(
            !sources.fresh(root, &EXCLUDE, before, b"report"),
            "a new file"
        );
        fs::remove_file(root.join("src/new.rs")).unwrap();
        fs::remove_file(root.join("tests/a.rs")).unwrap();
        assert!(
            !sources.fresh(root, &EXCLUDE, before, b"report"),
            "a removed file"
        );
    }

    #[test]
    fn a_report_is_newest_when_written_after_every_file_outside_its_own_directory() {
        let (dir, _, _) = recorded();
        let root = dir.path();
        let report = root.join("coverage/lcov.info");
        fs::create_dir_all(report.parent().unwrap()).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        fs::write(&report, "SF:a\n").unwrap();
        std::thread::sleep(Duration::from_millis(20));
        fs::write(root.join("coverage/index.html"), "").unwrap();
        assert!(tree(root).is_newest(&report));
        fs::write(root.join("src/lib.rs"), "fn b() {}\n").unwrap();
        assert!(!tree(root).is_newest(&report));
        assert!(!tree(root).is_newest(&root.join("missing.info")));
    }
}
