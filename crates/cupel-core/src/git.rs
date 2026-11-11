// ABOUTME: Read-only git access: the default branch, HEAD, merge bases, and changed files, answered by
// ABOUTME: running the `git` on PATH in the repository root (ADR-002). The only place Cupel runs git.
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// A git work tree whose root is the repository root Cupel runs in (or contains it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Git {
    root: PathBuf,
}

/// Where `HEAD` is and whether the working tree differs from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    /// Null on an unborn branch (no commit yet).
    pub sha: Option<String>,
    /// Null when `HEAD` is detached.
    pub branch: Option<String>,
    /// Tracked changes or untracked, non-ignored files exist.
    pub dirty: bool,
}

/// A git command that failed; `message` is git's own explanation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitError {
    pub command: String,
    pub message: String,
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}` failed: {}", self.command, self.message)
    }
}

impl std::error::Error for GitError {}

/// One path a range changes, relative to the root, and how.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Change {
    pub path: String,
    pub kind: ChangeKind,
}

/// Whether a path exists only after the change, on both sides, or only before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
}

fn sorted(mut changes: Vec<Change>) -> Vec<Change> {
    changes.sort();
    changes.dedup_by(|a, b| a.path == b.path);
    changes
}

fn paths(changes: Vec<Change>) -> Vec<String> {
    changes.into_iter().map(|change| change.path).collect()
}

impl Git {
    /// The work tree `root` is in; `None` outside one or when `git` cannot run.
    pub fn open(root: &Path) -> Option<Self> {
        let git = Self {
            root: root.to_path_buf(),
        };
        let inside = git.text(&["rev-parse", "--is-inside-work-tree"]).ok()?;
        (inside == "true").then_some(git)
    }

    /// The branch `origin/HEAD` points at, else a local `main` or `master`, else the branch
    /// `HEAD` names (ADR-002, ADR-021 §7).
    pub fn default_branch(&self) -> Option<String> {
        if let Ok(remote) = self.text(&["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]) {
            return Some(remote.strip_prefix("origin/").unwrap_or(&remote).to_owned());
        }
        ["main", "master"]
            .into_iter()
            .find(|branch| {
                self.text(&[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    &format!("refs/heads/{branch}"),
                ])
                .is_ok()
            })
            .map(str::to_owned)
            .or_else(|| self.text(&["symbolic-ref", "--short", "HEAD"]).ok())
    }

    pub fn head(&self) -> Head {
        Head {
            sha: self
                .text(&["rev-parse", "--verify", "--quiet", "HEAD"])
                .ok(),
            branch: self
                .text(&["symbolic-ref", "--short", "--quiet", "HEAD"])
                .ok(),
            dirty: self
                .output(&["status", "--porcelain", "--untracked-files=normal"])
                .is_ok_and(|out| !out.trim().is_empty()),
        }
    }

    /// The commit `rev` names, failing when it names none.
    pub fn commit(&self, rev: &str) -> Result<String, GitError> {
        self.text(&[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{rev}^{{commit}}"),
        ])
        .map_err(|error| GitError {
            message: format!("`{rev}` is not a commit in this repository"),
            ..error
        })
    }

    /// The best common ancestor of `a` and `b`.
    pub fn merge_base(&self, a: &str, b: &str) -> Result<String, GitError> {
        self.text(&["merge-base", a, b])
    }

    /// Files that differ between `rev` and the working tree, including staged changes and
    /// untracked, non-ignored files; relative to the root, `/`-separated, sorted, unique.
    pub fn changed_since(&self, rev: &str) -> Result<Vec<String>, GitError> {
        Ok(paths(self.changes_since(rev)?))
    }

    /// Files that differ between the commits `base` and `head`; relative to the root, sorted.
    pub fn changed_between(&self, base: &str, head: &str) -> Result<Vec<String>, GitError> {
        Ok(paths(self.changes_between(base, head)?))
    }

    /// How each file differs between `rev` and the working tree (staged changes included;
    /// untracked, non-ignored files are `Added`), sorted by path (ADR-056).
    pub fn changes_since(&self, rev: &str) -> Result<Vec<Change>, GitError> {
        let mut changes = self.statuses(&[rev])?;
        changes.extend(
            self.names(&["ls-files", "-z", "--others", "--exclude-standard"])?
                .into_iter()
                .map(|path| Change {
                    path,
                    kind: ChangeKind::Added,
                }),
        );
        Ok(sorted(changes))
    }

    /// How each file differs between the commits `base` and `head`, sorted by path. Renames
    /// are a deletion and an addition (ADR-056).
    pub fn changes_between(&self, base: &str, head: &str) -> Result<Vec<Change>, GitError> {
        Ok(sorted(self.statuses(&[base, head])?))
    }

    /// The lines of each file that differ between `base` and `head` (the working tree when
    /// `head` is `None`), as inclusive ranges of the new side; a pure deletion is the lines
    /// around where it was. Files are relative to the root, `/`-separated (ADR-077 §5).
    pub fn changed_lines(
        &self,
        base: &str,
        head: Option<&str>,
    ) -> Result<BTreeMap<String, Vec<(u32, u32)>>, GitError> {
        Ok(hunks(&self.diff_lines(base, head)?, Side::New))
    }

    /// The same differences as [`Git::changed_lines`], as inclusive ranges of the old side: the
    /// lines of `base` the change replaced or removed, and the lines around a pure insertion. A
    /// file the change added has none (ADR-175).
    pub fn base_lines(
        &self,
        base: &str,
        head: Option<&str>,
    ) -> Result<BTreeMap<String, Vec<(u32, u32)>>, GitError> {
        Ok(hunks(&self.diff_lines(base, head)?, Side::Old))
    }

    /// `git diff -U0` from `base` to `head`, or to the working tree.
    fn diff_lines(&self, base: &str, head: Option<&str>) -> Result<String, GitError> {
        let mut args = vec![
            "diff",
            "-U0",
            "--no-color",
            "--no-ext-diff",
            "--no-renames",
            "--relative",
            base,
        ];
        args.extend(head);
        args.push("--");
        self.output(&args)
    }

    /// The contents of `path` (relative to the root) at the commit `rev`.
    pub fn show(&self, rev: &str, path: &str) -> Result<Vec<u8>, GitError> {
        self.bytes(&["show", &format!("{rev}:./{path}")])
    }

    /// Writes every file of the commit `rev` under `dest` through a private index, leaving the
    /// repository's index and work tree alone, and returns the directory under `dest` that
    /// corresponds to the root (ADR-130).
    pub fn export(&self, rev: &str, dest: &Path) -> Result<PathBuf, GitError> {
        let top = Self {
            root: PathBuf::from(self.text(&["rev-parse", "--show-toplevel"])?),
        };
        let prefix = self.output(&["rev-parse", "--show-prefix"])?;
        let index = dest.with_extension("index");
        let target = format!("--prefix={}/", dest.display());
        let index_env = [("GIT_INDEX_FILE", index.as_os_str())];
        let exported = top
            .bytes_with(&["read-tree", rev], &index_env)
            .and_then(|_| {
                top.bytes_with(&["checkout-index", "--all", "--force", &target], &index_env)
            });
        let _ = std::fs::remove_file(&index);
        exported?;
        Ok(dest.join(prefix.trim()))
    }

    /// `git diff --name-status` against `revs`, relative to the root.
    fn statuses(&self, revs: &[&str]) -> Result<Vec<Change>, GitError> {
        let mut args = vec!["diff", "--name-status", "--no-renames", "-z", "--relative"];
        args.extend_from_slice(revs);
        args.push("--");
        let fields = self.names(&args)?;
        Ok(fields
            .chunks_exact(2)
            .map(|pair| Change {
                path: pair[1].clone(),
                kind: match pair[0].as_str() {
                    "A" => ChangeKind::Added,
                    "D" => ChangeKind::Deleted,
                    _ => ChangeKind::Modified,
                },
            })
            .collect())
    }

    /// NUL-separated names printed by `git <args>`.
    fn names(&self, args: &[&str]) -> Result<Vec<String>, GitError> {
        Ok(self
            .output(args)?
            .split('\0')
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect())
    }

    /// Trimmed, non-empty standard output of `git <args>`.
    fn text(&self, args: &[&str]) -> Result<String, GitError> {
        let out = self.output(args)?;
        let text = out.trim();
        if text.is_empty() {
            return Err(failure(args, "printed nothing".to_owned()));
        }
        Ok(text.to_owned())
    }

    /// Standard output of `git <args>` run in the root, failing on a non-zero exit.
    fn output(&self, args: &[&str]) -> Result<String, GitError> {
        String::from_utf8(self.bytes(args)?)
            .map_err(|_| failure(args, "printed output that is not UTF-8".to_owned()))
    }

    /// Raw standard output of `git <args>` run in the root, failing on a non-zero exit.
    fn bytes(&self, args: &[&str]) -> Result<Vec<u8>, GitError> {
        self.bytes_with(args, &[])
    }

    /// `bytes` with the environment variables `env` set for git.
    fn bytes_with(&self, args: &[&str], env: &[(&str, &OsStr)]) -> Result<Vec<u8>, GitError> {
        let output = Command::new("git")
            .args(args)
            .envs(env.iter().copied())
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .output()
            .map_err(|error| failure(args, error.to_string()))?;
        if !output.status.success() {
            let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(failure(args, message));
        }
        Ok(output.stdout)
    }
}

/// The line ranges of `side` of each file in a `git diff -U0` (ADR-077 §5).
fn hunks(diff: &str, side: Side) -> BTreeMap<String, Vec<(u32, u32)>> {
    let mut files: BTreeMap<String, Vec<(u32, u32)>> = BTreeMap::new();
    let mut current: Option<String> = None;
    // A file's `---`/`+++` names come before its first hunk; inside the hunks, a removed or added
    // line can start with the same characters.
    let mut in_header = false;
    for line in diff.lines() {
        if line.starts_with("diff ") {
            in_header = true;
        } else if let Some(path) = line.strip_prefix(side.file_marker()).filter(|_| in_header) {
            current = path.strip_prefix(side.prefix()).map(str::to_owned);
        } else if let Some(header) = line.strip_prefix("@@ ") {
            in_header = false;
            if let (Some(file), Some(range)) = (&current, side.range(header)) {
                files.entry(file.clone()).or_default().push(range);
            }
        }
    }
    files
}

/// Which side of a diff a hunk's lines are read from.
#[derive(Clone, Copy)]
enum Side {
    Old,
    New,
}

impl Side {
    /// The line naming a file's version on this side: `--- a/…` or `+++ b/…`.
    fn file_marker(self) -> &'static str {
        match self {
            Self::Old => "--- ",
            Self::New => "+++ ",
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Self::Old => "a/",
            Self::New => "b/",
        }
    }

    /// This side's range of a hunk header `-a,b +c,d @@`: for the new side, lines `c` to
    /// `c + d - 1`, or the lines `c` and `c + 1` around a deletion (`d` is 0); for the old side,
    /// the same of `a` and `b`.
    fn range(self, header: &str) -> Option<(u32, u32)> {
        let sign = match self {
            Self::Old => '-',
            Self::New => '+',
        };
        let lines = header
            .split_whitespace()
            .find_map(|part| part.strip_prefix(sign))?;
        let (start, count) = lines.split_once(',').unwrap_or((lines, "1"));
        let start: u32 = start.parse().ok()?;
        let count: u32 = count.parse().ok()?;
        Some(if count == 0 {
            (start.max(1), start + 1)
        } else {
            (start, start + count - 1)
        })
    }
}

fn failure(args: &[&str], message: String) -> GitError {
    GitError {
        command: format!("git {}", args.join(" ")),
        message,
    }
}
