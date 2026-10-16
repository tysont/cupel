// ABOUTME: The `serve` verb (ADR-085): run the daemon for this repository in the foreground, or stop the one
// ABOUTME: that serves it; and the handler that runs every request through the same `run` a cold process calls.
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cupel_core::{ActionKind, Cancellation, Envelope, ExitCode, Severity, Status, Writer};
use cupel_serve::{
    Body, Build, ClientError, Daemon, Done, Handler, Invocation, Request, ServeError,
};
use serde_json::json;

use crate::app::{Environment, run};
use crate::cli::ServeArgs;
use crate::error::CupelError;
use crate::problem::Problem;
use crate::run::{Invocation as Verb, Run};
use crate::session::{Session, Stream};

/// The component `serve`'s findings come from.
const CHECK: &str = "serve";

/// How long `serve --stop` waits for the daemon to finish its runs and remove its socket.
const STOP_WAIT: Duration = Duration::from_secs(30);

/// Where clients look for the daemon of the repository at `root`.
pub fn default_socket(root: &Path) -> PathBuf {
    root.join(Writer::DIR).join("serve.sock")
}

/// This binary's build, which a daemon and its clients must share (ADR-085 §4).
pub fn build() -> Build {
    Build::current(env!("CARGO_PKG_VERSION"))
}

/// `socket` as findings show it: relative to `root` when inside it.
pub fn shown(root: &Path, socket: &Path) -> String {
    socket
        .strip_prefix(root)
        .unwrap_or(socket)
        .display()
        .to_string()
}

/// `cupel serve`: serves until stopped, or with `--stop`, stops the daemon serving this
/// repository.
pub fn verb(
    invocation: &Verb<'_>,
    args: &ServeArgs,
    quiet: bool,
    environment: &Environment,
) -> Result<Envelope, CupelError> {
    if args.mcp {
        return Err(CupelError::pending_flag("--mcp", "P7.7"));
    }
    let root = invocation.writer.root();
    let socket = args
        .socket
        .as_ref()
        .map_or_else(|| default_socket(root), |socket| root.join(socket));
    let run = invocation.run;
    if args.stop {
        return Ok(stop(run, root, &socket));
    }
    let daemon = Daemon {
        root: root.to_path_buf(),
        socket,
        build: build(),
        path: environment.path.clone(),
        watch: args.watch,
    };
    let handler = Arc::new(Requests {
        session: Arc::clone(&environment.session),
    });
    let mut notices = environment.stderr.clone();
    let served = cupel_serve::serve(&daemon, handler, run.cancellation(), &mut |status| {
        if !quiet {
            let watching = if status.watching { ", watching" } else { "" };
            let _ = writeln!(
                notices,
                "cupel: serving {} at {} (pid {}{watching})",
                status.root.display(),
                shown(root, &status.socket),
                status.pid
            );
        }
    });
    Ok(match served {
        Ok(served) => reported(
            run,
            Status::Pass,
            ExitCode::Pass,
            stopped(root, &daemon.socket, Some(served.requests)),
        ),
        Err(error) => reported(
            run,
            Status::Error,
            ExitCode::Environment,
            unserved(root, &daemon.socket, &error),
        ),
    })
}

fn reported(run: &Run, status: Status, exit_code: ExitCode, problem: Problem) -> Envelope {
    run.reported(status, exit_code, vec![problem.finding(&run.repro())])
}

/// Asks the daemon at `socket` to stop and waits for it to; a stale socket file is removed.
fn stop(run: &Run, root: &Path, socket: &Path) -> Envelope {
    let stopping = Request::new(build(), Body::Stop);
    let problem = match cupel_serve::request(socket, &stopping, &mut |_| {}) {
        Ok(_) => {
            let deadline = Instant::now() + STOP_WAIT;
            while socket.exists() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(20));
            }
            stopped(root, socket, None)
        }
        Err(error) => {
            if matches!(error, ClientError::Unanswered(_)) {
                let _ = std::fs::remove_file(socket);
            }
            not_running(root, socket, &error)
        }
    };
    reported(run, Status::Pass, ExitCode::Pass, problem)
}

/// The daemon at `socket` stopped, after serving `requests` when this process was it.
fn stopped(root: &Path, socket: &Path, requests: Option<u64>) -> Problem {
    let shown = shown(root, socket);
    let title = match requests {
        Some(1) => format!("The daemon at {shown} stopped after serving 1 request"),
        Some(requests) => {
            format!("The daemon at {shown} stopped after serving {requests} requests")
        }
        None => format!("Stopped the daemon at {shown}"),
    };
    Problem {
        check: CHECK,
        kind: "serve.stopped",
        subject: shown.clone(),
        severity: Severity::Info,
        title,
        detail: None,
        evidence: json!({"socket": shown, "requests": requests}),
        action: ActionKind::Review,
        fix: "Nothing to do; `cupel serve` starts it again.".to_owned(),
    }
}

/// Nothing answered at `socket`.
fn not_running(root: &Path, socket: &Path, error: &ClientError) -> Problem {
    let shown = shown(root, socket);
    let detail = match error {
        ClientError::Unanswered(_) => {
            Some("Removed the socket file nothing answered on.".to_owned())
        }
        ClientError::Absent => None,
        other => Some(other.to_string()),
    };
    Problem {
        check: CHECK,
        kind: "serve.not-running",
        subject: shown.clone(),
        severity: Severity::Info,
        title: format!("No daemon answers at {shown}"),
        detail,
        evidence: json!({"socket": shown}),
        action: ActionKind::Review,
        fix: "Nothing to do; `cupel serve` starts one.".to_owned(),
    }
}

/// Why the daemon did not start.
fn unserved(root: &Path, socket: &Path, error: &ServeError) -> Problem {
    let shown = shown(root, socket);
    let (kind, fix, evidence) = match error {
        ServeError::Running(status) => (
            "serve.running",
            "Use the running daemon, or stop it first: `cupel serve --stop`.".to_owned(),
            json!({"socket": shown, "pid": status.pid, "version": status.version}),
        ),
        ServeError::Io { message, .. } => (
            "serve.failed",
            "Make .cupel/ writable, or pass --socket a shorter path (a Unix socket path is \
             limited to about 100 bytes)."
                .to_owned(),
            json!({"socket": shown, "error": message}),
        ),
    };
    Problem {
        check: CHECK,
        kind,
        subject: shown,
        severity: Severity::Fail,
        title: error.to_string(),
        detail: None,
        evidence,
        action: ActionKind::Configure,
        fix,
    }
}

/// Runs the daemon's requests: each through [`run`], with the daemon's session.
struct Requests {
    session: Arc<Session>,
}

impl Handler for Requests {
    fn run(
        &self,
        invocation: Invocation,
        stderr: Box<dyn Write + Send>,
        cancel: Cancellation,
    ) -> Done {
        let environment = Environment {
            stdout_is_terminal: invocation.stdout_is_terminal,
            variables: invocation.variables,
            no_color_variable: invocation.no_color,
            stderr: Stream::new(stderr),
            session: Arc::clone(&self.session),
            cancel,
            ..Environment::new(
                invocation.args.into_iter().map(OsString::from).collect(),
                invocation.current_dir,
            )
        };
        let output = run(&environment);
        // The stream sends a last unterminated line when dropped, before the final reply.
        drop(environment);
        self.session.sweep();
        Done {
            stdout: output.stdout,
            stderr: output.stderr,
            code: output.code,
        }
    }

    fn changed(&self, paths: &[PathBuf]) -> u64 {
        self.session.reparse(paths)
    }
}
