// ABOUTME: Read-only git access: the default branch, HEAD, merge bases, and changed files, answered by
// ABOUTME: running the `git` on PATH in the repository root (ADR-002). The only place Cupel runs git.
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
        let mut files = self.names(&["diff", "--name-only", "-z", "--relative", rev, "--"])?;
        files.extend(self.names(&["ls-files", "-z", "--others", "--exclude-standard"])?);
        files.sort();
        files.dedup();
        Ok(files)
    }

    /// Files that differ between the commits `base` and `head`; relative to the root, sorted.
    pub fn changed_between(&self, base: &str, head: &str) -> Result<Vec<String>, GitError> {
        let mut files =
            self.names(&["diff", "--name-only", "-z", "--relative", base, head, "--"])?;
        files.sort();
        files.dedup();
        Ok(files)
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
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .output()
            .map_err(|error| failure(args, error.to_string()))?;
        if !output.status.success() {
            let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(failure(args, message));
        }
        String::from_utf8(output.stdout)
            .map_err(|_| failure(args, "printed output that is not UTF-8".to_owned()))
    }
}

fn failure(args: &[&str], message: String) -> GitError {
    GitError {
        command: format!("git {}", args.join(" ")),
        message,
    }
}
