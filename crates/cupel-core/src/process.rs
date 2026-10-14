// ABOUTME: `Process`, the one way producers run an external program (ADR-076 §3): output captured, the child
// ABOUTME: polled against the run's `Cancellation`, and killed with its process group when `--timeout` fires.
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::cancel::Cancellation;
use crate::error::{Error, Result};

/// How often a running child is checked for exit and cancellation.
const POLL: Duration = Duration::from_millis(10);

/// A program to run: its arguments, directory, extra environment, and whether its standard error
/// is merged into its standard output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    program: String,
    args: Vec<String>,
    dir: PathBuf,
    env: Vec<(String, String)>,
    merged: bool,
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
            .stdin(Stdio::null());
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
        let mut child = command.spawn().map_err(|error| self.failed(&error))?;
        // The command holds the parent's copies of a merged pipe's write end; the reader sees
        // the end of the output only once they are closed.
        drop(command);
        let (stdout, stderr) = match merged {
            Some(reader) => (Some(read(reader)), None),
            None => (child.stdout.take().map(read), child.stderr.take().map(read)),
        };
        let exit_code = wait(&mut child, cancel);
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

/// Waits for `child` to exit, killing it when `cancel` fires first.
fn wait(child: &mut Child, cancel: &Cancellation) -> Result<Option<i32>> {
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return Ok(status.code());
        }
        if cancel.is_cancelled() {
            kill(child);
            return Err(Error::Cancelled);
        }
        std::thread::sleep(POLL);
    }
}

/// Kills `child` and, on Unix, every process in its group, then reaps it.
fn kill(child: &mut Child) {
    #[cfg(unix)]
    let _ = Command::new("kill")
        .args(["-KILL", &format!("-{}", child.id())])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
    let _ = child.wait();
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
