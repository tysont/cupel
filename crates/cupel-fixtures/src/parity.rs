// ABOUTME: Daemon parity (ADR-085 §11): with DAEMON_PARITY=1, every `run_cupel` also runs its invocation through a
// ABOUTME: fresh `cupel serve` on an identical copy of the directory and fails unless both outputs match after redaction.
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use cupel_core::Envelope;
use cupel_serve::{Body, Build, Done, Invocation, Reply, Request};
use serde_json::Value;
use similar::TextDiff;
use tempfile::TempDir;

use crate::golden::canonical_json;

/// Whether this test process proves parity: `DAEMON_PARITY=1` (`make parity`).
pub fn enabled() -> bool {
    std::env::var("DAEMON_PARITY").as_deref() == Ok("1")
}

/// A copy of a run's directory, taken before the cold run so the daemon's run starts from the
/// same state.
pub struct Twin {
    dir: TempDir,
    pub(crate) root: PathBuf,
    pub(crate) original: PathBuf,
}

impl Twin {
    /// Copies `cwd` into a new temporary directory, under the same name.
    ///
    /// # Panics
    ///
    /// When the directory cannot be copied.
    pub fn of(cwd: &Path) -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory for the twin");
        let name = cwd
            .file_name()
            .map_or_else(|| "root".into(), ToOwned::to_owned);
        let root = dir.path().join(name);
        copy(cwd, &root).unwrap_or_else(|error| panic!("cannot copy {}: {error}", cwd.display()));
        Self {
            dir,
            root,
            original: cwd.to_path_buf(),
        }
    }

    /// Runs `args` through a daemon on the copy and compares its code, redacted stdout, and
    /// redacted stderr with the cold run's.
    ///
    /// # Panics
    ///
    /// When they differ, with a unified diff; when the daemon cannot start or does not run it.
    pub fn assert_same(&self, args: &[&str], cold: (Option<i32>, &str, &str)) {
        let args: Vec<String> = args.iter().map(|arg| self.moved(arg)).collect();
        let served = Daemon::start(self.dir.path()).serve(&self.root, &args);
        let (code, stdout, stderr) = cold;
        let shown = |text: &str, root: &Path| redacted(text, root);
        let cold_text = format!(
            "exit {code:?}\n--- stdout\n{}--- stderr\n{}",
            shown(stdout, &self.original),
            shown(stderr, &self.original)
        );
        let served_text = format!(
            "exit {:?}\n--- stdout\n{}--- stderr\n{}",
            Some(i32::from(served.code)),
            shown(&served.stdout, &self.root),
            shown(&served.stderr, &self.root)
        );
        if cold_text != served_text {
            let diff = TextDiff::from_lines(&cold_text, &served_text)
                .unified_diff()
                .header("cold", "daemon")
                .to_string();
            panic!("cupel {args:?} differs through the daemon (DAEMON_PARITY=1):\n{diff}");
        }
    }

    /// `arg` with every spelling of the original directory replaced by the copy's.
    pub(crate) fn moved(&self, arg: &str) -> String {
        let canonical = self.original.canonicalize().unwrap_or_default();
        let mut moved = arg.to_owned();
        for spelling in [&self.original, &canonical] {
            if let Some(spelling) = spelling.to_str().filter(|s| !s.is_empty()) {
                moved = moved.replace(spelling, &self.root.display().to_string());
            }
        }
        moved
    }
}

/// `text` with every root spelling replaced and, when it is an envelope or a SARIF log, redacted
/// (ADR-003); each event line of a stream has its run ID and timings masked, and in a text
/// rendering the `run` line (ID, start, duration, seed) is masked whole and each check's `N ms`
/// too.
pub(crate) fn redacted(text: &str, root: &Path) -> String {
    if let Ok(envelope) = serde_json::from_str::<Envelope>(text) {
        return canonical_json(&crate::redact(&envelope, root));
    }
    if let Some(log) = crate::sarif::sarif_log(text) {
        return canonical_json(&crate::redact_sarif(&log, root));
    }
    let spellings = [root.canonicalize().ok(), Some(root.to_path_buf())];
    let mut lines = String::new();
    for line in text.split_inclusive('\n') {
        let line = match serde_json::from_str::<Value>(line) {
            _ if line.starts_with("run       run_") => "run       <run>\n".to_owned(),
            Ok(Value::Object(mut event)) if event.contains_key("ev") => {
                for key in ["run", "duration_ms", "est_ms", "eta_ms"] {
                    if let Some(value) = event.get_mut(key) {
                        *value = "<masked>".into();
                    }
                }
                format!("{}\n", Value::Object(event))
            }
            _ => timings_masked(line),
        };
        lines.push_str(&line);
    }
    for spelling in spellings.iter().flatten().filter_map(|path| path.to_str()) {
        lines = lines.replace(spelling, "<root>");
    }
    lines
}

/// `line` with every ` · `-separated `N ms` (a check's duration in text) masked.
fn timings_masked(line: &str) -> String {
    let (body, end) = line
        .strip_suffix('\n')
        .map_or((line, ""), |body| (body, "\n"));
    let parts: Vec<&str> = body
        .split(" · ")
        .map(|part| {
            let millis = part
                .strip_suffix(" ms")
                .filter(|digits| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()));
            if millis.is_some() { "<ms>" } else { part }
        })
        .collect();
    format!("{}{end}", parts.join(" · "))
}

/// A `cupel serve --socket` started for one comparison and stopped after it.
struct Daemon {
    child: Child,
    socket: PathBuf,
}

impl Daemon {
    /// Starts a daemon whose root is `dir` (not the copy, so the copy has no daemon of its own),
    /// with `dir` as its temporary directory, as a cold run has its own.
    fn start(dir: &Path) -> Self {
        let socket = dir.join("d.sock");
        let mut command = Command::new(assert_cmd::cargo::cargo_bin("cupel"));
        command
            .args([
                "serve",
                "--socket",
                "d.sock",
                "--quiet",
                "--timeout",
                "3600s",
            ])
            .current_dir(dir)
            .env("TMPDIR", dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("CUPEL_") {
                command.env_remove(key);
            }
        }
        let child = command.spawn().expect("cupel serve starts");
        let daemon = Self { child, socket };
        // Longer than any stall of a loaded machine: only a daemon that never answers is waited
        // on this long.
        let deadline = Instant::now() + Duration::from_secs(600);
        while daemon.ask(Body::Status).is_err() {
            assert!(
                Instant::now() < deadline,
                "the parity daemon never answered"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        daemon
    }

    fn ask(&self, body: Body) -> Result<Reply, cupel_serve::ClientError> {
        let build = Build::of(
            env!("CARGO_PKG_VERSION"),
            &assert_cmd::cargo::cargo_bin("cupel"),
        );
        cupel_serve::request(&self.socket, &Request::new(build, body), &mut |_| {})
    }

    /// Serves `args` as a `--dry-run` first, so the real request meets warm trees, then for
    /// real; what the real one returned. Streamed stderr comes before the rest, as a process's
    /// does.
    fn serve(self, root: &Path, args: &[String]) -> Done {
        let run = |args: Vec<String>| Body::Run {
            path: std::env::var("PATH").ok(),
            invocation: Invocation {
                args,
                current_dir: Some(root.to_path_buf()),
                stdout_is_terminal: false,
                variables: std::collections::BTreeMap::new(),
                no_color: std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty()),
            },
        };
        let warm = std::iter::once("--dry-run".to_owned()).chain(args.iter().cloned());
        let _ = self.ask(run(warm.collect()));
        let mut streamed = String::new();
        let build = Build::of(
            env!("CARGO_PKG_VERSION"),
            &assert_cmd::cargo::cargo_bin("cupel"),
        );
        let request = Request::new(build, run(args.to_vec()));
        let reply = cupel_serve::request(&self.socket, &request, &mut |text| {
            streamed.push_str(text);
        });
        match reply {
            Ok(Reply::Done(mut done)) => {
                done.stderr = streamed + &done.stderr;
                done
            }
            other => panic!("the parity daemon did not run {args:?}: {other:?}"),
        }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.ask(Body::Stop);
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Copies the tree at `from` to `into`: directories, files, and symbolic links; anything else
/// (a socket) is left out.
/// Copies the tree at `from` into `into`. An entry that is gone by the time it is reached (a lock
/// git's detached maintenance held for a moment under `.git`) is left out: the twin is what the
/// tree holds, and a failure names the path it stopped at.
fn copy(from: &Path, into: &Path) -> io::Result<()> {
    fs::create_dir_all(into)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let (path, target) = (entry.path(), into.join(entry.file_name()));
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => copy(&path, &target)?,
            Ok(kind) => match copy_entry(kind, &path, &target) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(at(&path, &error)),
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(at(&path, &error)),
        }
    }
    Ok(())
}

/// Copies one symlink or regular file; anything else (a socket, a pipe) is left out.
fn copy_entry(kind: fs::FileType, path: &Path, target: &Path) -> io::Result<()> {
    if kind.is_symlink() {
        symlink(&fs::read_link(path)?, path, target)
    } else if kind.is_file() {
        fs::copy(path, target).map(|_| ())
    } else {
        Ok(())
    }
}

/// Makes `target` a symbolic link to `link`, the target of the link at `path`.
#[cfg(unix)]
fn symlink(link: &Path, _path: &Path, target: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(link, target)
}

/// Makes `target` a symbolic link to `link`, the target of the link at `path`: a directory link
/// when `path` leads to a directory, as Windows tells the two apart.
#[cfg(windows)]
fn symlink(link: &Path, path: &Path, target: &Path) -> io::Result<()> {
    if fs::metadata(path).is_ok_and(|metadata| metadata.is_dir()) {
        std::os::windows::fs::symlink_dir(link, target)
    } else {
        std::os::windows::fs::symlink_file(link, target)
    }
}

/// `error` with the path it happened at in its message.
fn at(path: &Path, error: &io::Error) -> io::Error {
    io::Error::new(error.kind(), format!("{}: {error}", path.display()))
}
