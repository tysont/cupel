// ABOUTME: The daemon's server loop (ADR-085): claim the socket, accept connections, answer each on its own
// ABOUTME: thread, cancel a run whose client hangs up, and stop on request, on its token, or when the socket goes.
use std::fmt;
use std::io::{self, BufReader, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cupel_core::Cancellation;

use crate::client::{self, ClientError};
use crate::lines::Lines;
use crate::protocol::{
    Body, Build, Done, Invocation, PROTOCOL, Reply, Request, Status, read_message, write_message,
};
use crate::transport::{Connection, Listener};
use crate::watch::Watch;

/// How often the accept loop looks at its stop token and its socket file.
const POLL: Duration = Duration::from_millis(200);

/// What the daemon serves and how.
#[derive(Debug, Clone)]
pub struct Daemon {
    /// The repository it was started for; the watcher watches it.
    pub root: PathBuf,
    pub socket: PathBuf,
    pub build: Build,
    /// The daemon's own `PATH`; a run from a client with another is refused (ADR-085 §5).
    pub path: Option<String>,
    /// Whether to run the file watcher (`serve --watch`).
    pub watch: bool,
}

/// What the daemon runs requests with.
pub trait Handler: Send + Sync {
    /// Runs one invocation as a cold process would, writing to `stderr` what that process would
    /// write as it goes (it reaches the client as written) and stopping early once `cancel` fires.
    fn run(
        &self,
        invocation: Invocation,
        stderr: Box<dyn Write + Send>,
        cancel: Cancellation,
    ) -> Done;

    /// `paths` changed under the root; returns how many files it reparsed.
    fn changed(&self, paths: &[PathBuf]) -> u64;
}

/// Why the daemon did not serve.
#[derive(Debug)]
pub enum ServeError {
    /// Another daemon answers on the socket.
    Running(Box<Status>),
    /// The socket could not be made, or the watcher could not start.
    Io { socket: PathBuf, message: String },
}

impl fmt::Display for ServeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Running(status) => write!(
                f,
                "a daemon (pid {}, {}) already serves at {}",
                status.pid,
                status.build,
                status.socket.display()
            ),
            Self::Io { socket, message } => {
                write!(f, "cannot serve at {}: {message}", socket.display())
            }
        }
    }
}

impl std::error::Error for ServeError {}

/// What a daemon did before it stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Served {
    pub requests: u64,
}

/// What every connection shares.
struct Shared {
    daemon: Daemon,
    handler: Arc<dyn Handler>,
    /// Fires when the daemon stops; every run's token is under it.
    stopping: Cancellation,
    served: AtomicU64,
    reparsed: AtomicU64,
}

/// Serves `daemon` with `handler` until a `stop` request, `stop` firing, or its socket file
/// being removed or replaced. `ready` hears the daemon's status once it accepts connections.
/// Runs in flight when it stops are cancelled and answered before it returns.
pub fn serve(
    daemon: &Daemon,
    handler: Arc<dyn Handler>,
    stop: &Cancellation,
    ready: &mut dyn FnMut(&Status),
) -> Result<Served, ServeError> {
    let mut listener = claim(daemon)?;
    let shared = Arc::new(Shared {
        daemon: daemon.clone(),
        handler,
        stopping: stop.under(None),
        served: AtomicU64::new(0),
        reparsed: AtomicU64::new(0),
    });
    let watch = daemon
        .watch
        .then(|| watch(&shared))
        .transpose()
        .map_err(|error| failed(daemon, &error))?;
    ready(&shared.status());
    let mut workers: Vec<JoinHandle<()>> = Vec::new();
    while !shared.stopping.is_cancelled() && listener.is_current() {
        if let Some(stream) = listener.accept(POLL) {
            let shared = Arc::clone(&shared);
            workers.push(thread::spawn(move || shared.answer(&stream)));
        }
        workers.retain(|worker| !worker.is_finished());
    }
    shared.stopping.cancel();
    drop(watch);
    for worker in workers {
        let _ = worker.join();
    }
    if listener.is_current() {
        let _ = std::fs::remove_file(&daemon.socket);
    }
    Ok(Served {
        requests: shared.served.load(Ordering::SeqCst),
    })
}

/// Listens at the daemon's endpoint, replacing a stale endpoint file nothing answers on.
fn claim(daemon: &Daemon) -> Result<Listener, ServeError> {
    let status = Request::new(daemon.build.clone(), Body::Status);
    match client::request(&daemon.socket, &status, &mut |_| {}) {
        Ok(Reply::Status(running)) => return Err(ServeError::Running(Box::new(running))),
        Err(ClientError::Absent) => {}
        _ => {
            let _ = std::fs::remove_file(&daemon.socket);
        }
    }
    if let Some(parent) = daemon.socket.parent() {
        std::fs::create_dir_all(parent).map_err(|error| failed(daemon, &error))?;
    }
    Listener::bind(&daemon.socket).map_err(|error| failed(daemon, &error))
}

fn failed(daemon: &Daemon, error: &dyn fmt::Display) -> ServeError {
    ServeError::Io {
        socket: daemon.socket.clone(),
        message: error.to_string(),
    }
}

/// The watcher, handing changed files to the handler and counting what it reparsed.
fn watch(shared: &Arc<Shared>) -> notify::Result<Watch> {
    let watched = Arc::clone(shared);
    Watch::start(&shared.daemon.root, move |paths| {
        let reparsed = watched.handler.changed(&paths);
        watched.reparsed.fetch_add(reparsed, Ordering::SeqCst);
    })
}

impl Shared {
    fn status(&self) -> Status {
        Status {
            version: self.daemon.build.version.clone(),
            build: self.daemon.build.clone(),
            pid: std::process::id(),
            root: self.daemon.root.clone(),
            socket: self.daemon.socket.clone(),
            watching: self.daemon.watch,
            served: self.served.load(Ordering::SeqCst),
            reparsed: self.reparsed.load(Ordering::SeqCst),
        }
    }

    /// Reads one request from `stream` and answers it.
    fn answer(&self, stream: &Connection) {
        let Ok(input) = stream.try_clone() else {
            return;
        };
        let mut input = BufReader::new(input);
        let reply = match read_message::<Request>(&mut input) {
            Ok(None) => return,
            Ok(Some(request)) => self.reply(request, stream, input),
            Err(error) => Reply::Error {
                message: error.to_string(),
            },
        };
        let _ = write_message(&mut &*stream, &reply);
    }

    fn reply(&self, request: Request, stream: &Connection, input: BufReader<Connection>) -> Reply {
        match request.body {
            Body::Status => Reply::Status(self.status()),
            Body::Stop => {
                self.stopping.cancel();
                Reply::Stopping
            }
            Body::Run { path, invocation } => {
                match self.refusal(request.protocol, &request.build, path.as_deref()) {
                    Some(reason) => Reply::Refused { reason },
                    None => Reply::Done(self.run(invocation, stream, input)),
                }
            }
        }
    }

    /// Why this daemon will not run a request from `build` speaking `protocol` with `path`.
    fn refusal(&self, protocol: u32, build: &Build, path: Option<&str>) -> Option<String> {
        let socket = self.daemon.socket.display();
        let restart = "Restart it from this one: `cupel serve --stop`, then `cupel serve`.";
        if protocol != PROTOCOL {
            Some(format!(
                "this client speaks daemon protocol {protocol} and the daemon at {socket} speaks \
                 {PROTOCOL}. {restart}"
            ))
        } else if *build != self.daemon.build {
            Some(format!(
                "the daemon at {socket} runs another build of cupel ({}) than this client \
                 ({build}). {restart}",
                self.daemon.build
            ))
        } else if path != self.daemon.path.as_deref() {
            Some(format!(
                "the daemon at {socket} runs with another PATH than this client, so it would \
                 find other tools. {restart}"
            ))
        } else {
            None
        }
    }

    /// Runs `invocation`, cancelling it if the client hangs up first.
    fn run(
        &self,
        invocation: Invocation,
        stream: &Connection,
        input: BufReader<Connection>,
    ) -> Done {
        let cancel = self.stopping.under(None);
        let hang_up = cancel.clone();
        thread::spawn(move || {
            input.into_inner().wait_for_hang_up();
            hang_up.cancel();
        });
        // Each complete line goes to the client as a `stderr` reply when written.
        let stderr: Box<dyn Write + Send> = match stream.try_clone() {
            Ok(mut stream) => Box::new(Lines::new(move |text: &str| {
                let text = text.to_owned();
                write_message(&mut stream, &Reply::Stderr { text })
            })),
            Err(_) => Box::new(io::sink()),
        };
        let done = self.handler.run(invocation, stderr, cancel);
        self.served.fetch_add(1, Ordering::SeqCst);
        done
    }
}
