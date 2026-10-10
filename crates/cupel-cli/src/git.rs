// ABOUTME: The few git questions P0 asks, answered by running the `git` on PATH in the repository root.
// ABOUTME: An interim measure until ADR-002 (P2.1) decides how Cupel accesses git (ADR-021 §7).
use std::path::Path;
use std::process::{Command, Stdio};

/// The repository's default branch: the branch `origin/HEAD` points at, else a local `main` or
/// `master`, else the branch `HEAD` names. `None` outside a repository or without git.
pub fn default_branch(root: &Path) -> Option<String> {
    if let Some(remote) = git(
        root,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    ) {
        return Some(remote.strip_prefix("origin/").unwrap_or(&remote).to_owned());
    }
    ["main", "master"]
        .into_iter()
        .find(|branch| {
            git(
                root,
                &[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    &format!("refs/heads/{branch}"),
                ],
            )
            .is_some()
        })
        .map(str::to_owned)
        .or_else(|| git(root, &["symbolic-ref", "--short", "HEAD"]))
}

/// The trimmed standard output of `git <args>` run in `root`, when it succeeds with output.
fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (output.status.success() && !text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `git <args>` in `dir`; false when git is not installed.
    fn run_git(dir: &Path, args: &[&str]) -> bool {
        Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .is_ok_and(|output| output.status.success())
    }

    #[test]
    fn a_directory_outside_any_repository_has_no_default_branch() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(default_branch(dir.path()), None);
    }

    #[test]
    fn a_local_main_branch_wins_over_the_checked_out_branch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        if !run_git(path, &["init", "--quiet", "--initial-branch", "main"]) {
            eprintln!("SKIPPED: git not installed");
            return;
        }
        assert_eq!(default_branch(path).as_deref(), Some("main"), "unborn HEAD");
        let committed = run_git(
            path,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.com",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "first",
            ],
        );
        assert!(committed);
        assert!(run_git(path, &["checkout", "--quiet", "-b", "feature"]));
        assert_eq!(default_branch(path).as_deref(), Some("main"));
    }
}
