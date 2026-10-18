// ABOUTME: `Process`, the one way producers run an external program (ADR-076 §3): input given, output captured,
// ABOUTME: the child polled against the run's `Cancellation`, and stopped with its process group when it fires.
use std::io::{ErrorKind, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::cancel::Cancellation;
use crate::error::{Error, Result};

/// How often a running child is checked for exit and cancellation.
const POLL: Duration = Duration::from_millis(10);

/// How long a spawn waits for another process to close a program it still holds open for writing.
const BUSY_WAIT: Duration = Duration::from_secs(2);

/// A program to run: its arguments, directory, extra environment, and whether its standard error
/// is merged into its standard output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    program: String,
    args: Vec<String>,
    dir: PathBuf,
    env: Vec<(String, String)>,
    merged: bool,
    /// Written to the child's standard input, which is then closed; `None` for no input.
    input: Option<Vec<u8>>,
    /// How long a cancelled child has to exit after SIGTERM before SIGKILL; `None` kills at once.
    grace: Option<Duration>,
}

/// What a finished process wrote and how it exited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Captured {
    /// `None` when the process was ended by a signal.
    pub exit_code: Option<i32>,
    /// Standard output; with [`Process::merged`], standard error too, interleaved as written.
    pub stdout: String,
    /// Standard error; empty with [`Process::merged`].
    pub stderr: String,
}

impl Process {
    /// `program`, run in `dir`.
    pub fn new(program: &str, dir: impl Into<PathBuf>) -> Self {
        Self {
            program: program.to_owned(),
            args: Vec::new(),
            dir: dir.into(),
            env: Vec::new(),
            merged: false,
            input: None,
            grace: None,
        }
    }

    #[must_use]
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// Sets `key` to `value` in the child's environment, which otherwise is this process's.
    #[must_use]
    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.env.push((key.to_owned(), value.to_owned()));
        self
    }

    /// Sends standard error to the same pipe as standard output, so the two keep their order.
    #[must_use]
    pub fn merged(mut self) -> Self {
        self.merged = true;
        self
    }

    /// Writes `input` to the child's standard input, then closes it (a plugin's request).
    #[must_use]
    pub fn input(mut self, input: Vec<u8>) -> Self {
        self.input = Some(input);
        self
    }

    /// When cancelled, sends SIGTERM to the child's process group and kills the group only if
    /// it is still running after `grace` (ADR-089 §7); without this the group is killed at once.
    #[must_use]
    pub fn terminate(mut self, grace: Duration) -> Self {
        self.grace = Some(grace);
        self
    }

    /// The program and its arguments.
    pub fn command_line(&self) -> Vec<String> {
        std::iter::once(self.program.clone())
            .chain(self.args.iter().cloned())
            .collect()
    }

    /// Runs the program to completion, or until `cancel` fires, when it is killed (with its
    /// process group on Unix) and the result is `Error::Cancelled`.
    pub fn run(&self, cancel: &Cancellation) -> Result<Captured> {
        cancel.checkpoint()?;
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .current_dir(&self.dir)
            .stdin(if self.input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            });
        for key in std::env::vars_os().map(|(key, _)| key) {
            if is_coverage_harness(&key.to_string_lossy()) {
                command.env_remove(key);
            }
        }
        for (key, value) in &self.env {
            command.env(key, value);
        }
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        let merged = if self.merged {
            let (reader, writer) = std::io::pipe().map_err(|error| self.failed(&error))?;
            let copy = writer.try_clone().map_err(|error| self.failed(&error))?;
            command.stdout(copy).stderr(writer);
            Some(reader)
        } else {
            command.stdout(Stdio::piped()).stderr(Stdio::piped());
            None
        };
        let mut child = spawn(&mut command).map_err(|error| self.failed(&error))?;
        // The command holds the parent's copies of a merged pipe's write end; the reader sees
        // the end of the output only once they are closed.
        drop(command);
        let writing = child
            .stdin
            .take()
            .zip(self.input.clone())
            .map(|(stdin, input)| write(stdin, input));
        let (stdout, stderr) = match merged {
            Some(reader) => (Some(read(reader)), None),
            None => (child.stdout.take().map(read), child.stderr.take().map(read)),
        };
        let exit_code = wait(&mut child, cancel, self.grace);
        if let Some(writing) = writing {
            let _ = writing.join();
        }
        let (stdout, stderr) = (joined(stdout), joined(stderr));
        Ok(Captured {
            exit_code: exit_code?,
            stdout,
            stderr,
        })
    }

    fn failed(&self, error: &std::io::Error) -> Error {
        Error::Process {
            program: self.program.clone(),
            message: error.to_string(),
        }
    }
}

/// Waits for `child` to exit, stopping it when `cancel` fires first: at once, or after SIGTERM
/// and `grace`.
/// Spawns `command`. Linux refuses to execute a program some process has open for writing
/// (`ETXTBSY`); a child another thread forked holds a just-written script's descriptor until it
/// execs, so the spawn is retried until the program closes, within `BUSY_WAIT`.
fn spawn(command: &mut Command) -> std::io::Result<Child> {
    let deadline = Instant::now() + BUSY_WAIT;
    loop {
        match command.spawn() {
            Err(error)
                if error.kind() == ErrorKind::ExecutableFileBusy && Instant::now() < deadline =>
            {
                std::thread::sleep(POLL);
            }
            spawned => return spawned,
        }
    }
}

fn wait(child: &mut Child, cancel: &Cancellation, grace: Option<Duration>) -> Result<Option<i32>> {
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return Ok(status.code());
        }
        if cancel.is_cancelled() {
            if let Some(grace) = grace {
                terminate(child, grace);
            }
            kill(child);
            return Err(Error::Cancelled);
        }
        std::thread::sleep(POLL);
    }
}

/// Sends SIGTERM to `child`'s process group and waits up to `grace` for `child` to exit.
fn terminate(child: &mut Child, grace: Duration) {
    #[cfg(unix)]
    {
        use nix::sys::signal::{Signal, killpg};
        use nix::unistd::Pid;
        if let Ok(pid) = i32::try_from(child.id()) {
            let _ = killpg(Pid::from_raw(pid), Signal::SIGTERM);
        }
    }
    let until = std::time::Instant::now() + grace;
    while std::time::Instant::now() < until {
        if let Ok(Some(_)) = child.try_wait() {
            return;
        }
        std::thread::sleep(POLL);
    }
}

/// Kills `child` and, on Unix, every process in its group, then reaps it. The group is the
/// child's own (`process_group(0)` above), so its descendants die with it and cannot keep the
/// output pipes open after it is gone.
fn kill(child: &mut Child) {
    #[cfg(unix)]
    {
        use nix::sys::signal::{Signal, killpg};
        use nix::unistd::Pid;
        if let Ok(pid) = i32::try_from(child.id()) {
            let _ = killpg(Pid::from_raw(pid), Signal::SIGKILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Whether `key` is a variable a coverage harness measuring Cupel itself (cargo-llvm-cov) set:
/// its profile file, its state, and the rustc wrapper it installs for Cupel's own crates. A child
/// inheriting them is measured as Cupel, or fails to build, so a child gets none of them. Another
/// rustc wrapper is the user's own and stays.
fn is_coverage_harness(key: &str) -> bool {
    key.starts_with("CARGO_LLVM_COV")
        || key.starts_with("__CARGO_LLVM_COV_")
        || key == "LLVM_PROFILE_FILE"
        || (matches!(key, "RUSTC_WRAPPER" | "RUSTC_WORKSPACE_WRAPPER")
            && std::env::var_os("__CARGO_LLVM_COV_RUSTC_WRAPPER").is_some())
}

/// Writes `input` to `sink` on its own thread and closes it; a child that exits without
/// reading it all ends the write early.
fn write(mut sink: impl Write + Send + 'static, input: Vec<u8>) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let _ = sink.write_all(&input);
    })
}

/// Reads `source` to its end on its own thread.
fn read(mut source: impl Read + Send + 'static) -> JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = source.read_to_end(&mut bytes);
        bytes
    })
}

/// What `reader` read, as text; empty for a stream that was not captured.
fn joined(reader: Option<JoinHandle<Vec<u8>>>) -> String {
    let bytes = reader
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Where `program` is on `PATH`, if it is there as a file.
pub fn find_program(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}
