// ABOUTME: Freshness of cached tool output (ADR-079, ADR-083): what every file under a project root hashed to when Cupel
// ABOUTME: made a report or a per-test map, which files changed since, and whether a cached report still describes the tree.
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use cupel_core::syntax::ProjectInfo;
use cupel_core::{Exclude, ProduceCtx, Result, Writer};
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

    /// This record, made after a tool ran, with every file that `before` (made before it ran)
    /// recorded otherwise marked as changed, so the next comparison finds it changed: what the
    /// tool measured may be either version.
    pub fn since(mut self, before: &Self) -> Self {
        for (name, recorded) in &mut self.files {
            if before
                .files
                .get(name)
                .is_some_and(|earlier| earlier != recorded)
            {
                recorded.clear();
            }
        }
        self
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
        hash(report) == self.report && self.changes(repo, exclude, written).is_empty()
    }

    /// The files under the root that differ from the record: those added, and those removed or
    /// hashing otherwise (a file not modified since `written` is trusted without rehashing).
    pub fn changes(&self, repo: &Path, exclude: &Exclude, written: SystemTime) -> Changes {
        let tree = Tree {
            repo,
            dir: &self.dir,
            skip_dirs: &self.skip_dirs,
            exclude,
        };
        let files = tree.files();
        let mut changes = Changes::default();
        for (name, path) in &files {
            match self.files.get(name) {
                None => {
                    changes.added.insert(name.clone());
                }
                Some(recorded) => {
                    let same = !recorded.is_empty()
                        && (modified(path).is_some_and(|at| at < written)
                            || fs::read(path).is_ok_and(|bytes| hash(&bytes) == *recorded));
                    if !same {
                        changes.changed.insert(name.clone());
                    }
                }
            }
        }
        let removed = self.files.keys().filter(|name| !files.contains_key(*name));
        changes.changed.extend(removed.cloned());
        changes
    }
}

/// The files under a project root that differ from a record, relative to the root.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Changes {
    /// Files the record does not hold.
    pub added: BTreeSet<String>,
    /// Files the record holds that are gone or hash otherwise.
    pub changed: BTreeSet<String>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.changed.is_empty()
    }

    /// Every added, changed, or removed file.
    pub fn all(&self) -> impl Iterator<Item = &String> {
        self.added.iter().chain(&self.changed)
    }
}

impl Tree<'_> {
    /// The root directory, absolute.
    pub fn root(&self) -> PathBuf {
        self.repo.join(self.dir)
    }

    /// Every file under the root, by its path relative to the root, `/`-separated: hidden
    /// entries, paths a `.gitignore` inside the root ignores, `skip_dirs`, and excluded paths
    /// are left out.
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
        // Only ignore files inside the root count: a parent's, the user's global one, or
        // `.git/info/exclude` would make freshness differ between machines and checkouts.
        let walk = WalkBuilder::new(&root)
            .require_git(false)
            .parents(false)
            .git_global(false)
            .git_exclude(false)
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

/// The cache directory of producer `id` among the `kind` caches (`coverage`, `per-test`) of the
/// repository at `repo`: `.cupel/cache/<kind>/<id>/`, one entry per project root below it.
pub(crate) fn cache_dir(repo: &Path, kind: &str, id: &str) -> PathBuf {
    repo.join(Writer::DIR).join("cache").join(kind).join(id)
}

/// The cache entry name of the project root `dir`: one per root whatever language asked, so two
/// languages sharing a root (JavaScript and TypeScript) share it.
pub(crate) fn entry_name(dir: &str) -> String {
    hash(dir.as_bytes())[..16].to_owned()
}

/// Every entry of the cache directory `dir`.
pub(crate) fn entries(dir: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .collect();
    entries.sort();
    entries
}

/// The record in the cache entry `entry`, when it was written, and the report beside it, when
/// the report hashes as recorded.
pub(crate) fn load(entry: &Path) -> Option<(Sources, SystemTime, Vec<u8>)> {
    let (sources, written) = Sources::load(entry)?;
    let report = fs::read(entry.join(REPORT)).ok()?;
    (hash(&report) == sources.report).then_some((sources, written, report))
}

/// Writes `report` and its record into the cache entry `entry`.
pub(crate) fn store(writer: &Writer, entry: &Path, report: &[u8], sources: &Sources) -> Result<()> {
    writer.write(&entry.join(REPORT), report)?;
    let json = serde_json::to_vec_pretty(sources).unwrap_or_default();
    writer.write(&entry.join(SOURCES), &json)?;
    Ok(())
}

/// The directories skipped below the project root `dir` when hashing it: the generated ones
/// its `ProjectInfo` root names, and `extra` (a tool's own output directories).
pub(crate) fn skip_dirs(ctx: &ProduceCtx, dir: &str, extra: &[&str]) -> Vec<String> {
    let mut skip: Vec<String> = ctx
        .upstream
        .get::<ProjectInfo>()
        .and_then(|info| info.roots.iter().find(|root| root.path == dir))
        .map(|root| root.skip_dirs.clone())
        .unwrap_or_default();
    for output in extra {
        if !skip.iter().any(|skipped| skipped == output) {
            skip.push((*output).to_owned());
        }
    }
    skip
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
        let fresh = |at: SystemTime, report: &[u8]| sources.fresh(root, &EXCLUDE, at, report);
        let mut seen = vec![fresh(written, b"report"), fresh(written, b"another report")];
        fs::write(root.join("target/out.o"), "rebuilt").unwrap();
        fs::write(root.join(".hidden"), "y").unwrap();
        seen.push(fresh(written, b"report"));
        fs::write(root.join("tests/a.rs"), "#[test] fn u() {}\n").unwrap();
        seen.push(fresh(written, b"report"));
        // A file older than the record is trusted without rehashing.
        seen.push(fresh(
            SystemTime::now() + Duration::from_secs(60),
            b"report",
        ));
        assert_eq!(seen, [true, false, true, false, true]);
    }

    #[test]
    fn a_rewritten_file_with_the_same_content_stays_fresh_and_added_or_removed_files_do_not() {
        let (dir, sources, _) = recorded();
        let root = dir.path();
        let before = SystemTime::now() - Duration::from_secs(60);
        let fresh = || sources.fresh(root, &EXCLUDE, before, b"report");
        fs::write(root.join("src/lib.rs"), "fn a() {}\n").unwrap();
        let mut seen = vec![fresh()];
        fs::write(root.join("src/new.rs"), "").unwrap();
        seen.push(fresh());
        fs::remove_file(root.join("src/new.rs")).unwrap();
        fs::remove_file(root.join("tests/a.rs")).unwrap();
        seen.push(fresh());
        assert_eq!(seen, [true, false, false]);
    }

    #[test]
    fn changes_name_the_added_the_rewritten_and_the_removed_files_but_not_one_rewritten_alike() {
        let (dir, sources, written) = recorded();
        let root = dir.path();
        fs::write(root.join("src/lib.rs"), "fn a() {}\n").unwrap();
        fs::write(root.join("tests/a.rs"), "#[test] fn u() {}\n").unwrap();
        fs::write(root.join("src/new.rs"), "").unwrap();
        fs::write(root.join("target/new.o"), "").unwrap();
        let seen = sources.changes(root, &EXCLUDE, written);
        assert_eq!(
            (
                seen.added.iter().map(String::as_str).collect::<Vec<_>>(),
                seen.changed.iter().map(String::as_str).collect::<Vec<_>>()
            ),
            (vec!["src/new.rs"], vec!["tests/a.rs"])
        );
        fs::remove_file(root.join("src/lib.rs")).unwrap();
        let seen = sources.changes(root, &EXCLUDE, written);
        assert_eq!(
            seen.all().map(String::as_str).collect::<Vec<_>>(),
            ["src/new.rs", "src/lib.rs", "tests/a.rs"]
        );
    }

    #[test]
    fn a_file_edited_while_a_tool_ran_is_changed_since_and_a_file_the_tool_added_is_not() {
        let (dir, before, written) = recorded();
        let root = dir.path();
        fs::write(root.join("src/lib.rs"), "fn edited() {}\n").unwrap();
        fs::write(root.join("Cargo.lock"), "").unwrap();
        let after = Sources::record(&tree(root), root, b"report").since(&before);
        // Even with every file older than the record, as when the record is written after the run.
        let later = SystemTime::now() + Duration::from_secs(60);
        assert_eq!(
            after.changes(root, &EXCLUDE, later),
            after.changes(root, &EXCLUDE, written)
        );
        let changes = after.changes(root, &EXCLUDE, later);
        assert_eq!(
            changes.all().map(String::as_str).collect::<Vec<_>>(),
            ["src/lib.rs"]
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
        let mut seen = vec![tree(root).is_newest(&report)];
        fs::write(root.join("src/lib.rs"), "fn b() {}\n").unwrap();
        seen.push(tree(root).is_newest(&report));
        seen.push(tree(root).is_newest(&root.join("missing.info")));
        assert_eq!(seen, [true, false, false]);
    }
}
