// ABOUTME: The `serve` verb (ADR-085, ADR-121): run the daemon or the MCP server for this repository in the
// ABOUTME: foreground, or stop the daemon; both run every request through the same `run` a cold process calls.
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cupel_core::{ActionKind, Cancellation, Envelope, ExitCode, Severity, Status, Writer};
use cupel_serve::mcp::{self, Call, Called, Server, Tool, Tools};
use cupel_serve::{
    Body, Build, ClientError, Daemon, Done, Handler, Invocation, Request, ServeError, Watch,
};
use serde_json::json;

use crate::cli::ServeArgs;
use crate::environment::{Environment, Output};
use crate::format::Rendering;
use crate::problem::Problem;
use crate::run::{Invocation as Verb, Run};
use crate::session::{Session, Stream};
use crate::{mcp as catalog, render};

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

/// How the daemon runs an invocation: the function a cold process calls.
pub type Runs = fn(&Environment) -> Output;

/// `cupel serve`: serves until stopped, running each request with `run`; with `--mcp`, serves
/// MCP on stdin and stdout instead; with `--stop`, stops the daemon serving this repository.
pub fn verb(
    invocation: &Verb<'_>,
    args: &ServeArgs,
    (quiet, run): (bool, Runs),
    environment: &Environment,
) -> Envelope {
    let requests = Requests {
        session: Arc::clone(&environment.session),
        run,
        path: environment.path.clone(),
    };
    if args.mcp {
        return serve_mcp(invocation, args.watch, quiet, requests, environment);
    }
    let root = invocation.writer.root();
    let socket = args
        .socket
        .as_ref()
        .map_or_else(|| default_socket(root), |socket| root.join(socket));
    if args.stop {
        return stop(invocation.run, root, &socket);
    }
    let daemon = Daemon {
        root: root.to_path_buf(),
        socket,
        build: build(),
        path: environment.path.clone(),
        watch: args.watch,
    };
    let handler = Arc::new(requests);
    let run = invocation.run;
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
    match served {
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
    }
}

/// `serve --mcp`: serves the MCP tools on the invocation's stdin and stdout until stdin ends or
/// the run is cancelled (ADR-121); with `watch`, reparses changed files as the daemon does.
fn serve_mcp(
    invocation: &Verb<'_>,
    watch: bool,
    quiet: bool,
    requests: Requests,
    environment: &Environment,
) -> Envelope {
    let (run, root) = (invocation.run, invocation.writer.root());
    let session = Arc::clone(&requests.session);
    let watcher = if watch {
        match Watch::start(root, move |paths| {
            session.reparse(&paths);
        }) {
            Ok(watcher) => Some(watcher),
            Err(error) => {
                return reported(run, Status::Error, ExitCode::Environment, unwatched(&error));
            }
        }
    } else {
        None
    };
    if !quiet {
        let watching = if watch { ", watching" } else { "" };
        let mut notices = environment.stderr.clone();
        let _ = writeln!(
            notices,
            "cupel: serving {} over MCP on stdio (pid {}{watching})",
            root.display(),
            std::process::id()
        );
    }
    let verbs = Verbs {
        requests,
        root: root.to_path_buf(),
        variables: environment.variables.clone(),
        no_color: environment.no_color_variable,
    };
    let identity = Server {
        name: "cupel".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    };
    let served = mcp::serve(
        &identity,
        Arc::new(verbs),
        environment.stdin.take(),
        Box::new(environment.stdout.clone()),
        run.cancellation(),
    );
    drop(watcher);
    reported(
        run,
        Status::Pass,
        ExitCode::Pass,
        mcp_stopped(served.requests),
    )
}

/// The MCP server stopped after answering `requests` tool calls.
fn mcp_stopped(requests: u64) -> Problem {
    let calls = if requests == 1 { "call" } else { "calls" };
    Problem {
        check: CHECK,
        kind: "serve.mcp-stopped",
        subject: "stdio".to_owned(),
        severity: Severity::Info,
        title: format!("The MCP server stopped after answering {requests} tool {calls}"),
        detail: None,
        evidence: json!({"requests": requests}),
        action: ActionKind::Review,
        fix: "Nothing to do; the MCP client starts `cupel serve --mcp` again.".to_owned(),
    }
}

/// The watcher behind `--watch` could not start.
fn unwatched(error: &dyn std::fmt::Display) -> Problem {
    Problem {
        check: CHECK,
        kind: "serve.failed",
        subject: "stdio".to_owned(),
        severity: Severity::Fail,
        title: format!("cannot watch the repository: {error}"),
        detail: None,
        evidence: json!({"error": error.to_string()}),
        action: ActionKind::Configure,
        fix: "Serve without --watch, or raise the limit on watched files.".to_owned(),
    }
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

/// Runs the daemon's requests and the MCP server's calls: each through `run`, with the
/// serving process's session.
struct Requests {
    session: Arc<Session>,
    run: Runs,
    /// The serving process's `PATH`, where a run finds `cupel-check-*` and `cupel-produce-*`
    /// plugins (ADR-089); the daemon serves only clients with the same one.
    path: Option<String>,
}

impl Requests {
    /// The environment of `invocation`, writing to `stderr` and stopping when `cancel` fires.
    fn environment(
        &self,
        invocation: Invocation,
        stderr: Stream,
        cancel: Cancellation,
    ) -> Environment {
        Environment {
            stdout_is_terminal: invocation.stdout_is_terminal,
            variables: invocation.variables,
            no_color_variable: invocation.no_color,
            stderr,
            path: self.path.clone(),
            session: Arc::clone(&self.session),
            cancel,
            ..Environment::new(
                invocation.args.into_iter().map(OsString::from).collect(),
                invocation.current_dir,
            )
        }
    }

    /// Runs `environment`, then drops the trees no file parses to any more.
    fn execute(&self, environment: Environment) -> Done {
        let output = (self.run)(&environment);
        // The stream sends a last unterminated line when dropped, before the final reply.
        drop(environment);
        self.session.sweep();
        Done {
            stdout: output.stdout,
            stderr: output.stderr,
            code: output.code,
        }
    }
}

impl Handler for Requests {
    fn run(
        &self,
        invocation: Invocation,
        stderr: Box<dyn Write + Send>,
        cancel: Cancellation,
    ) -> Done {
        self.execute(self.environment(invocation, Stream::new(stderr), cancel))
    }

    fn changed(&self, paths: &[PathBuf]) -> u64 {
        self.session.reparse(paths)
    }
}

/// The MCP tools (ADR-121): each call is the command line its arguments spell, run in the
/// repository as a cold process started there with the server's environment would run it.
struct Verbs {
    requests: Requests,
    root: PathBuf,
    /// The server's `CUPEL_*` variables.
    variables: BTreeMap<String, String>,
    no_color: bool,
}

impl Tools for Verbs {
    fn tools(&self) -> Vec<Tool> {
        catalog::tools()
    }

    fn call(&self, name: &str, call: Call) -> Called {
        let mut stderr = Stream::new(call.stderr);
        let command = match catalog::command(name, &call.arguments) {
            Ok(command) => command,
            Err((command, usage)) => {
                let envelope = Run::start(command.clone()).failed(&usage);
                return Called {
                    command,
                    document: render::render(&envelope, Rendering::Json, false, None),
                    is_error: true,
                };
            }
        };
        let invocation = Invocation {
            args: command.clone(),
            current_dir: Some(self.root.clone()),
            stdout_is_terminal: false,
            variables: self.variables.clone(),
            no_color: self.no_color,
        };
        let mut environment = self
            .requests
            .environment(invocation, stderr.clone(), call.cancel);
        environment.events = Some(Stream::new(call.events));
        let done = self.requests.execute(environment);
        let _ = stderr.write_all(done.stderr.as_bytes());
        Called {
            command,
            document: done.stdout,
            is_error: catalog::is_error(done.code),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::sink;
    use std::thread;
    use std::time::Duration;

    use cupel_serve::mcp::Arguments;
    use serde_json::Value;

    use super::*;

    #[test]
    fn a_tool_call_runs_under_its_token_and_ends_partial_when_it_fires() {
        let dir = tempfile::tempdir().unwrap();
        let verbs = Verbs {
            requests: Requests {
                session: Arc::default(),
                run: crate::app::run,
                path: None,
            },
            root: dir.path().to_path_buf(),
            variables: BTreeMap::from([("CUPEL_DEBUG_SLEEP_MS".to_owned(), "60000".to_owned())]),
            no_color: false,
        };
        let cancel = Cancellation::new();
        let firing = cancel.clone();
        let started = Instant::now();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(200));
            firing.cancel();
        });
        let call = Call {
            arguments: Arguments::default(),
            events: Box::new(sink()),
            stderr: Box::new(sink()),
            cancel,
        };
        let called = verbs.call("survey", call);
        assert!(started.elapsed() < Duration::from_secs(30));
        let envelope: Value = serde_json::from_str(&called.document).unwrap();
        assert_eq!(
            (envelope["status"].as_str(), envelope["exit_code"].as_u64()),
            (Some("partial"), Some(5))
        );
        assert!(!called.is_error);
        assert_eq!(called.command, ["survey", "--format", "json"]);
    }
}
