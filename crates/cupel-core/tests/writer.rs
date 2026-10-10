// ABOUTME: Tests for `Writer`, the only way Cupel writes files: free under `.cupel/`, elsewhere
// ABOUTME: only with `--write`, judged after resolving `..` and symlinks; dry runs record diffs instead.
use std::fs;
use std::path::{Path, PathBuf};

use cupel_core::{Error, PlannedWrite, RunId, WriteError, Writer};
use tempfile::TempDir;

struct Repo {
    dir: TempDir,
}

impl Repo {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("temporary directory"),
        }
    }

    fn root(&self) -> PathBuf {
        self.dir.path().canonicalize().unwrap()
    }

    fn writer(&self, write: bool) -> Writer {
        Writer::new(self.dir.path(), write).expect("root exists")
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root().join(relative)
    }

    /// Every file under the root, relative and sorted.
    fn files(&self) -> Vec<String> {
        fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
            for entry in fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(&path, root, out);
                } else {
                    out.push(path.strip_prefix(root).unwrap().display().to_string());
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.root(), &self.root(), &mut out);
        out.sort();
        out
    }
}

fn assert_refused(result: Result<PathBuf, Error>, expected: &Path) {
    match result {
        Err(Error::Write(WriteError::Outside { path })) => assert_eq!(path, expected),
        other => panic!(
            "expected a refusal for {}, got {other:?}",
            expected.display()
        ),
    }
}

#[test]
fn writes_under_cupel_without_the_write_flag() {
    let repo = Repo::new();
    let written = repo
        .writer(false)
        .write(Path::new(".cupel/runs/a.json"), b"{}\n")
        .unwrap();
    assert_eq!(written, repo.path(".cupel/runs/a.json"));
    assert_eq!(fs::read_to_string(&written).unwrap(), "{}\n");
}

#[test]
fn creating_the_cupel_directory_also_writes_its_gitignore() {
    let repo = Repo::new();
    repo.writer(false)
        .write(Path::new(".cupel/cache/x"), b"x")
        .unwrap();
    assert_eq!(
        fs::read_to_string(repo.path(".cupel/.gitignore")).unwrap(),
        "*\n!suppressions.toml\n"
    );
    assert_eq!(repo.files(), [".cupel/.gitignore", ".cupel/cache/x"]);
}

#[test]
fn gitignore_helper_writes_the_cupel_gitignore() {
    let repo = Repo::new();
    let path = repo.writer(false).gitignore().unwrap();
    assert_eq!(path, repo.path(".cupel/.gitignore"));
    assert_eq!(fs::read_to_string(path).unwrap(), Writer::GITIGNORE);
}

#[test]
fn refuses_a_relative_path_outside_cupel_without_write() {
    let repo = Repo::new();
    assert_refused(
        repo.writer(false).write(Path::new("src/main.rs"), b"x"),
        &repo.path("src/main.rs"),
    );
    assert!(repo.files().is_empty(), "{:?}", repo.files());
}

#[test]
fn refuses_dot_dot_escapes_from_cupel() {
    let repo = Repo::new();
    let writer = repo.writer(false);
    assert_refused(
        writer.write(Path::new(".cupel/../src/x"), b"x"),
        &repo.path("src/x"),
    );
    assert_refused(
        writer.write(Path::new(".cupel/runs/../../x"), b"x"),
        &repo.path("x"),
    );
    fs::create_dir_all(repo.path(".cupel/runs")).unwrap();
    assert_refused(
        writer.write(Path::new(".cupel/runs/../../y"), b"x"),
        &repo.path("y"),
    );
    assert!(!repo.path("x").exists() && !repo.path("y").exists());
}

#[test]
fn refuses_an_absolute_path_elsewhere_without_write() {
    let repo = Repo::new();
    let elsewhere = Repo::new();
    let target = elsewhere.path("out.json");
    assert_refused(repo.writer(false).write(&target, b"x"), &target);
    assert!(!target.exists());
}

#[test]
fn refuses_a_lookalike_sibling_of_cupel() {
    let repo = Repo::new();
    assert_refused(
        repo.writer(false).write(Path::new(".cupel-evil/x"), b"x"),
        &repo.path(".cupel-evil/x"),
    );
}

#[cfg(unix)]
#[test]
fn refuses_a_symlink_inside_cupel_that_points_elsewhere() {
    let repo = Repo::new();
    let elsewhere = Repo::new();
    fs::create_dir_all(repo.path(".cupel")).unwrap();
    std::os::unix::fs::symlink(elsewhere.root(), repo.path(".cupel/link")).unwrap();
    assert_refused(
        repo.writer(false).write(Path::new(".cupel/link/x"), b"x"),
        &elsewhere.path("x"),
    );
    assert!(!elsewhere.path("x").exists());
}

#[cfg(unix)]
#[test]
fn refuses_a_dangling_symlink_inside_cupel() {
    let repo = Repo::new();
    let elsewhere = Repo::new();
    fs::create_dir_all(repo.path(".cupel")).unwrap();
    std::os::unix::fs::symlink(elsewhere.path("missing"), repo.path(".cupel/dangling")).unwrap();
    let result = repo.writer(true).write(Path::new(".cupel/dangling"), b"x");
    assert!(
        matches!(result, Err(Error::Write(WriteError::Outside { .. }))),
        "{result:?}"
    );
    assert!(!elsewhere.path("missing").exists());
}

#[cfg(unix)]
#[test]
fn refuses_a_cupel_directory_that_is_a_symlink_elsewhere() {
    let repo = Repo::new();
    let elsewhere = Repo::new();
    std::os::unix::fs::symlink(elsewhere.root(), repo.path(".cupel")).unwrap();
    assert_refused(
        repo.writer(false)
            .write(Path::new(".cupel/runs/a.json"), b"x"),
        &elsewhere.path("runs/a.json"),
    );
}

#[test]
fn with_write_allows_paths_outside_cupel_and_outside_the_repository() {
    let repo = Repo::new();
    let elsewhere = Repo::new();
    let writer = repo.writer(true);
    writer
        .write(Path::new("src/main.rs"), b"fn main() {}\n")
        .unwrap();
    writer.write(&elsewhere.path("out.json"), b"{}").unwrap();
    assert_eq!(
        fs::read_to_string(repo.path("src/main.rs")).unwrap(),
        "fn main() {}\n"
    );
    assert!(elsewhere.path("out.json").exists());
}

#[test]
fn relative_paths_resolve_against_the_root_not_the_process_directory() {
    let repo = Repo::new();
    let written = repo
        .writer(false)
        .write(Path::new(".cupel/a"), b"x")
        .unwrap();
    assert!(written.starts_with(repo.root()));
}

#[test]
fn a_scratch_writer_writes_only_inside_its_run_work_directory() {
    let repo = Repo::new();
    let run = RunId::generate();
    let scratch = repo.writer(true).scratch(&run);
    let work = format!(".cupel/work/{run}");
    assert_eq!(scratch.area(), repo.path(&work));
    scratch
        .write(&repo.path(&format!("{work}/tool.out")), b"x")
        .unwrap();
    assert_refused(
        scratch.write(Path::new(".cupel/runs/a.json"), b"x"),
        &repo.path(".cupel/runs/a.json"),
    );
    assert_refused(scratch.write(Path::new("src/x"), b"x"), &repo.path("src/x"));
    assert_eq!(
        scratch.create_dir(Path::new(&work)).unwrap(),
        repo.path(&work)
    );
}

#[test]
fn a_confined_writer_keeps_to_cupel_even_when_write_was_given() {
    let repo = Repo::new();
    let confined = repo.writer(true).confined();
    confined.write(Path::new(".cupel/cache/a"), b"x").unwrap();
    assert_refused(
        confined.write(Path::new("src/x"), b"x"),
        &repo.path("src/x"),
    );
}

#[test]
fn layout_paths_are_under_cupel() {
    let repo = Repo::new();
    let writer = repo.writer(false);
    let run = RunId::generate();
    assert_eq!(writer.root(), repo.root());
    assert_eq!(writer.cupel_dir(), repo.path(".cupel"));
    assert_eq!(writer.cache_dir(), repo.path(".cupel/cache"));
    assert_eq!(
        writer.work_dir(&run),
        repo.path(&format!(".cupel/work/{run}"))
    );
    assert_eq!(
        writer.run_record_path(&run),
        repo.path(&format!(".cupel/runs/{run}.json"))
    );
}

#[test]
fn a_recording_writer_writes_nothing_and_plans_a_diff_per_file() {
    let repo = Repo::new();
    fs::write(
        repo.path("cupel.toml"),
        "depth = \"quick\"\nformat = \"json\"\n",
    )
    .unwrap();
    let before = repo.files();
    let writer = repo.writer(true).recording();
    writer
        .write(
            Path::new("cupel.toml"),
            b"depth = \"deep\"\nformat = \"json\"\n",
        )
        .unwrap();
    writer
        .write(Path::new(".cupel/.gitignore"), b"*\n")
        .unwrap();
    assert_eq!(repo.files(), before, "a recording writer touches nothing");
    assert_eq!(
        writer.planned(),
        [
            PlannedWrite {
                path: "cupel.toml".to_owned(),
                diff: Some(
                    "--- a/cupel.toml\n+++ b/cupel.toml\n@@ -1,2 +1,2 @@\n-depth = \"quick\"\n\
                     +depth = \"deep\"\n format = \"json\"\n"
                        .to_owned()
                ),
            },
            PlannedWrite {
                path: ".cupel/.gitignore".to_owned(),
                diff: Some(
                    "--- /dev/null\n+++ b/.cupel/.gitignore\n@@ -0,0 +1 @@\n+*\n".to_owned()
                ),
            },
        ]
    );
}

#[test]
fn a_recording_writer_still_refuses_what_a_real_run_would_refuse() {
    let repo = Repo::new();
    let writer = repo.writer(false).recording();
    assert_refused(
        writer.write(Path::new("cupel.toml"), b"x"),
        &repo.path("cupel.toml"),
    );
    assert!(writer.planned().is_empty());
}

#[test]
fn recording_is_shared_by_scratch_and_confined_writers() {
    let repo = Repo::new();
    let run = RunId::generate();
    let writer = repo.writer(false).recording();
    writer
        .confined()
        .write(Path::new(".cupel/cache/a"), b"a\n")
        .unwrap();
    writer
        .scratch(&run)
        .write(&writer.work_dir(&run).join("b"), b"b\n")
        .unwrap();
    let paths: Vec<String> = writer.planned().into_iter().map(|p| p.path).collect();
    assert_eq!(
        paths,
        [".cupel/cache/a".to_owned(), format!(".cupel/work/{run}/b")]
    );
    assert!(repo.files().is_empty());
}

#[test]
fn planned_paths_outside_the_repository_are_absolute() {
    let repo = Repo::new();
    let elsewhere = Repo::new();
    let writer = repo.writer(true).recording();
    writer.write(&elsewhere.path("out.json"), b"{}\n").unwrap();
    assert_eq!(
        writer.planned()[0].path,
        elsewhere.path("out.json").display().to_string()
    );
}

#[test]
fn streams_open_under_cupel_and_are_refused_elsewhere_without_write() {
    use std::io::Write as _;
    let repo = Repo::new();
    let writer = repo.writer(false).recording();
    let mut file = writer.stream(Path::new(".cupel/events.jsonl")).unwrap();
    file.write_all(b"{}\n").unwrap();
    assert_eq!(
        fs::read_to_string(repo.path(".cupel/events.jsonl")).unwrap(),
        "{}\n",
        "a stream is an output channel, written even while recording"
    );
    let refused = writer
        .stream(Path::new("events.jsonl"))
        .map(|_| PathBuf::new());
    assert_refused(refused, &repo.path("events.jsonl"));
}

#[test]
fn a_missing_root_is_an_io_error() {
    let repo = Repo::new();
    let missing = repo.path("missing");
    assert!(matches!(
        Writer::new(&missing, false),
        Err(Error::Write(WriteError::Io { .. }))
    ));
}

#[test]
fn a_probe_creates_cupel_and_leaves_only_its_gitignore() {
    let repo = Repo::new();
    repo.writer(false).probe().unwrap();
    assert_eq!(repo.files(), [".cupel/.gitignore"]);
    repo.writer(false).probe().unwrap();
    assert_eq!(
        repo.files(),
        [".cupel/.gitignore"],
        "the probe file is removed"
    );
}

#[test]
fn a_probe_fails_when_cupel_is_not_a_directory() {
    let repo = Repo::new();
    fs::write(repo.path(".cupel"), "").unwrap();
    match repo.writer(false).probe() {
        Err(Error::Write(WriteError::Io { path, .. })) => {
            assert!(path.starts_with(repo.path(".cupel")), "{}", path.display());
        }
        other => panic!("expected an I/O error, got {other:?}"),
    }
}

#[test]
fn a_recording_probe_touches_nothing() {
    let repo = Repo::new();
    repo.writer(false).recording().probe().unwrap();
    assert!(repo.files().is_empty());
    assert!(!repo.path(".cupel").exists());
}
