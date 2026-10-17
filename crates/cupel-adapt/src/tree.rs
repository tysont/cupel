// ABOUTME: Whole trees for producers that run code somewhere other than the working tree: a commit exported with
// ABOUTME: `git archive` (a differential oracle's ref, ADR-093 §7; a bench's base commit, ADR-106).
use std::fs;
use std::path::Path;

use cupel_core::{Cancellation, Error, Process, output_tail};

/// Writes the tree of `commit` in the repository at `root` into `tree`, through an archive
/// under `scratch`. Nothing is written in the repository and no ignored file (a virtual
/// environment, `node_modules`, a build directory) comes along.
pub fn export(
    root: &Path,
    commit: &str,
    scratch: &Path,
    tree: &Path,
    cancel: &Cancellation,
) -> cupel_core::Result<()> {
    let archive = scratch.join("tree.tar");
    fs::create_dir_all(tree).map_err(|error| git_error(&error.to_string()))?;
    let archive_text = archive.to_string_lossy().into_owned();
    let steps = [
        Process::new("git", root).args(["archive", "--format=tar", "-o", &archive_text, commit]),
        Process::new("tar", scratch).args(["-xf", &archive_text, "-C", &tree.to_string_lossy()]),
    ];
    for step in steps {
        let ran = step.run(cancel)?;
        if ran.exit_code != Some(0) {
            return Err(git_error(&format!(
                "cannot export {commit}: {}",
                output_tail(&ran.stderr)
                    .unwrap_or_default()
                    .replace('\n', " | ")
            )));
        }
    }
    Ok(())
}

/// A git failure, as a process error.
pub fn git_error(message: &str) -> Error {
    Error::Process {
        program: "git".to_owned(),
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) {
        let ran = Process::new("git", dir)
            .args(args.iter().copied())
            .run(&Cancellation::new())
            .unwrap();
        assert_eq!(ran.exit_code, Some(0), "git {args:?}: {}", ran.stderr);
    }

    #[test]
    fn a_commit_is_exported_without_the_working_tree_or_ignored_files() {
        let repo = tempfile::tempdir().unwrap();
        let root = repo.path();
        git(root, &["init", "-q"]);
        fs::write(root.join(".gitignore"), "build/\n").unwrap();
        fs::write(root.join("kept.txt"), "at the commit\n").unwrap();
        git(root, &["add", "."]);
        git(
            root,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-qm",
                "one",
            ],
        );
        fs::write(root.join("kept.txt"), "changed since\n").unwrap();
        fs::create_dir(root.join("build")).unwrap();
        fs::write(root.join("build/out"), "ignored\n").unwrap();
        let scratch = tempfile::tempdir().unwrap();
        let tree = scratch.path().join("tree");

        export(root, "HEAD", scratch.path(), &tree, &Cancellation::new()).unwrap();

        assert_eq!(
            fs::read_to_string(tree.join("kept.txt")).unwrap(),
            "at the commit\n"
        );
        assert!(!tree.join("build").exists());
    }

    #[test]
    fn a_commit_git_cannot_name_is_an_error_naming_it() {
        let repo = tempfile::tempdir().unwrap();
        git(repo.path(), &["init", "-q"]);
        let scratch = tempfile::tempdir().unwrap();

        let error = export(
            repo.path(),
            "no-such-ref",
            scratch.path(),
            &scratch.path().join("tree"),
            &Cancellation::new(),
        )
        .unwrap_err();

        assert!(
            error.to_string().contains("cannot export no-such-ref"),
            "{error}"
        );
    }
}
