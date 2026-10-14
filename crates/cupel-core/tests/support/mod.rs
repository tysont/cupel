// ABOUTME: Shared test support for cupel-core integration tests: throwaway git repositories built with
// ABOUTME: the real `git`, so git access and scope resolution are tested against real repositories.
#![allow(
    dead_code,
    reason = "each test binary uses a different subset of the helpers"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A temporary git repository whose default branch is `main`.
pub struct GitRepo {
    dir: tempfile::TempDir,
}

impl GitRepo {
    /// A fresh repository, or `None` (after printing `SKIPPED: git not installed`) without git.
    pub fn new() -> Option<Self> {
        let repo = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        let ok = Command::new("git")
            .args(["init", "--quiet", "--initial-branch", "main"])
            .current_dir(repo.path())
            .output()
            .is_ok_and(|output| output.status.success());
        if !ok {
            eprintln!("SKIPPED: git not installed");
            return None;
        }
        Some(repo)
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// The canonical root, as the CLI resolves `--cwd`.
    pub fn root(&self) -> PathBuf {
        self.path().canonicalize().unwrap()
    }

    pub fn write(&self, file: &str, text: &str) {
        let path = self.path().join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// Runs `git <args>` and returns its trimmed standard output.
    pub fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.com",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(self.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    /// Stages everything and commits it; returns the new commit's SHA.
    pub fn commit(&self, message: &str) -> String {
        self.git(&["add", "--all"]);
        self.git(&["commit", "--quiet", "--allow-empty", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
    }
}
