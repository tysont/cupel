// ABOUTME: Tests for the daemon's server and client over the real transport (Unix sockets, Windows named pipes) with a scripted handler: answers,
// ABOUTME: streaming order, refusals, malformed requests, stop, stale sockets, concurrency, hang-ups, the watcher.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cupel_core::Cancellation;
use cupel_serve::transport::Connection;
use cupel_serve::{
    Body, Build, ClientError, Daemon, Done, Handler, Invocation, Reply, Request, ServeError,
    Served, Status, request, serve,
};
use tempfile::TempDir;

/// Longer than any stall of a loaded machine: a wait this long is one for something that never
/// happens.
const HANG: Duration = Duration::from_secs(600);

/// Whether `condition` holds within `HANG`, polled every 20 ms.
fn within_hang(condition: impl Fn() -> bool) -> bool {
    let patience = Instant::now() + HANG;
    while !condition() {
        if Instant::now() >= patience {
            return false;
        }
        thread::sleep(Duration::from_millis(20));
    }
    true
}

/// A handler scripted by the invocation's first argument, recording each run's stdout when it
/// finishes and every changed path the watcher hands it, and counting the runs it has begun.
#[derive(Default)]
struct Script {
    finished: Mutex<Vec<String>>,
    changed: Mutex<Vec<PathBuf>>,
    begun: AtomicUsize,
    /// Set by a client once the first line of a `lines` run reached it.
    streamed: AtomicBool,
    /// The `gather` runs in flight.
    gathered: AtomicUsize,
}

impl Script {
    fn finished(&self) -> Vec<String> {
        self.finished.lock().unwrap().clone()
    }

    fn begun(&self) -> usize {
        self.begun.load(Ordering::SeqCst)
    }
}

impl Handler for Script {
    fn run(
        &self,
        invocation: Invocation,
        mut stderr: Box<dyn Write + Send>,
        cancel: Cancellation,
    ) -> Done {
        let args = invocation.args;
        self.begun.fetch_add(1, Ordering::SeqCst);
        let stdout = match args[0].as_str() {
            // The run goes on past its first line only once the client has it, and says whether
            // the client had it while the run was still going.
            "lines" => {
                writeln!(stderr, "line 0").unwrap();
                let streamed = within_hang(|| self.streamed.load(Ordering::SeqCst));
                for line in 1..3 {
                    writeln!(stderr, "line {line}").unwrap();
                }
                if streamed { "out" } else { "held back" }.to_owned()
            }
            // Answers its id once `args[1]` gather runs are in flight at once, or "alone".
            "gather" => {
                let wanted: usize = args[1].parse().unwrap();
                self.gathered.fetch_add(1, Ordering::SeqCst);
                let met = within_hang(|| self.gathered.load(Ordering::SeqCst) >= wanted);
                if met {
                    args[2].clone()
                } else {
                    "alone".to_owned()
                }
            }
            "wait" if cancel.wait(HANG) => "finished".to_owned(),
            "wait" => "cancelled".to_owned(),
            _ => args.join(" "),
        };
        self.finished.lock().unwrap().push(stdout.clone());
        Done {
            stdout,
            stderr: "rest\n".to_owned(),
            code: 0,
        }
    }

    fn changed(&self, paths: &[PathBuf]) -> u64 {
        self.changed.lock().unwrap().extend_from_slice(paths);
        paths.len() as u64
    }
}

fn build() -> Build {
    Build {
        version: "0.1.0".to_owned(),
        binary: "1-2".to_owned(),
    }
}

const PATH: &str = "/usr/bin:/bin";

/// A daemon serving a root from a thread; dropping it stops it and waits for it.
struct Running {
    root: PathBuf,
    socket: PathBuf,
    stop: Cancellation,
    script: Arc<Script>,
    thread: Option<JoinHandle<Result<Served, ServeError>>>,
    /// The temporary directory the daemon serves, removed once the daemon has stopped.
    dir: Option<TempDir>,
}

impl Running {
    /// A daemon for a new temporary directory.
    fn new(watch: bool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let mut running = Self::start(&root, watch);
        running.dir = Some(dir);
        running
    }

    /// A daemon for `root`.
    fn start(root: &Path, watch: bool) -> Self {
        let socket = root.join(".cupel/serve.sock");
        let script = Arc::new(Script::default());
        let stop = Cancellation::new();
        let (ready, started) = mpsc::channel();
        let daemon = Daemon {
            root: root.to_path_buf(),
            socket: socket.clone(),
            build: build(),
            path: Some(PATH.to_owned()),
            watch,
        };
        let handler: Arc<dyn Handler> = script.clone();
        let stopped = stop.clone();
        let thread = thread::spawn(move || {
            serve(&daemon, handler, &stopped, &mut |status: &Status| {
                ready.send(status.clone()).unwrap();
            })
        });
        // Generous: on macOS a freshly built binary's first FSEvents registration (the watcher's
        // `FSEventStreamStart`) has taken 8-14 s here, and a wait cut short leaves it to start over
        // on the next run; a built binary that has registered once is ready in well under a second.
        started.recv_timeout(HANG).unwrap();
        Self {
            root: root.to_path_buf(),
            socket,
            stop,
            script,
            thread: Some(thread),
            dir: None,
        }
    }

    /// Waits for the daemon to exit on its own.
    fn finished(mut self) -> Result<Served, ServeError> {
        self.thread.take().unwrap().join().unwrap()
    }

    fn ask(&self, request_: &Request) -> Result<Reply, ClientError> {
        request(&self.socket, request_, &mut |_| {})
    }

    fn status(&self) -> Status {
        match self.ask(&Request::new(build(), Body::Status)).unwrap() {
            Reply::Status(status) => status,
            other => panic!("not a status: {other:?}"),
        }
    }

    /// Why the daemon refused `request_`.
    fn refusal(&self, request_: &Request) -> String {
        match self.ask(request_) {
            Err(ClientError::Refused(reason)) => reason,
            other => panic!("not refused: {other:?}"),
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop.cancel();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        drop(self.dir.take());
    }
}

fn run_request(args: &[&str]) -> Request {
    Request::new(
        build(),
        Body::Run {
            path: Some(PATH.to_owned()),
            invocation: Invocation {
                args: args.iter().map(|arg| (*arg).to_owned()).collect(),
                ..Invocation::default()
            },
        },
    )
}

fn ask(socket: &Path, request_: &Request) -> Result<Reply, ClientError> {
    request(socket, request_, &mut |_| {})
}

fn stdout(reply: Result<Reply, ClientError>) -> String {
    match reply {
        Ok(Reply::Done(done)) => done.stdout,
        other => panic!("not done: {other:?}"),
    }
}

#[test]
fn the_daemon_says_what_it_is_and_runs_requests() {
    let daemon = Running::new(false);
    let before = daemon.status();
    assert_eq!(
        (
            before.version.as_str(),
            before.pid,
            before.served,
            before.watching
        ),
        ("0.1.0", std::process::id(), 0, false)
    );
    assert_eq!(before.socket, daemon.socket);
    assert_eq!(stdout(daemon.ask(&run_request(&["echo", "hi"]))), "echo hi");
    assert_eq!(daemon.status().served, 1);
}

#[test]
fn stderr_reaches_the_client_in_order_while_the_run_goes_on() {
    let daemon = Running::new(false);
    let mut lines = Vec::new();
    let reply = request(&daemon.socket, &run_request(&["lines"]), &mut |text| {
        lines.push(text.to_owned());
        daemon.script.streamed.store(true, Ordering::SeqCst);
    });
    let Ok(Reply::Done(done)) = reply else {
        panic!("not done: {reply:?}")
    };
    assert_eq!(lines, ["line 0\n", "line 1\n", "line 2\n"]);
    // "out": the run saw the client holding its first line before it wrote the second.
    assert_eq!(
        (done.stdout.as_str(), done.stderr.as_str()),
        ("out", "rest\n")
    );
}

#[test]
fn a_run_from_another_build_protocol_or_path_is_refused_but_status_and_stop_are_answered() {
    let daemon = Running::new(false);
    let other = Build {
        version: "0.2.0".to_owned(),
        binary: "3-4".to_owned(),
    };
    let mut from_other = run_request(&["echo"]);
    from_other.build = other.clone();
    let mut old_protocol = run_request(&["echo"]);
    old_protocol.protocol = 0;
    let mut other_path = run_request(&["echo"]);
    other_path.body = Body::Run {
        path: Some("/opt/bin".to_owned()),
        invocation: Invocation::default(),
    };
    let reasons = [
        daemon.refusal(&from_other),
        daemon.refusal(&old_protocol),
        daemon.refusal(&other_path),
    ];
    assert!(
        reasons[0].contains("another build of cupel (cupel 0.1.0 (binary 1-2))")
            && reasons[0].contains("cupel serve --stop"),
        "{}",
        reasons[0]
    );
    assert!(reasons[1].contains("protocol 0"), "{}", reasons[1]);
    assert!(reasons[2].contains("another PATH"), "{}", reasons[2]);
    assert_eq!(daemon.status().served, 0);

    let answered = daemon.ask(&Request::new(other.clone(), Body::Status));
    assert!(matches!(answered, Ok(Reply::Status(_))), "{answered:?}");
    let stopped = daemon.ask(&Request::new(other, Body::Stop));
    assert_eq!(stopped, Ok(Reply::Stopping));
    assert_eq!(daemon.finished().unwrap().requests, 0);
}

#[test]
fn a_malformed_request_gets_an_error_and_the_daemon_keeps_serving() {
    let daemon = Running::new(false);
    let mut stream = Connection::connect(&daemon.socket).unwrap();
    stream.write_all(b"{\"type\":\"dance\"}\n").unwrap();
    let reply: Reply = cupel_serve::read_message(&mut std::io::BufReader::new(&stream))
        .unwrap()
        .unwrap();
    assert!(
        matches!(&reply, Reply::Error { message } if message.starts_with("the message is not valid")),
        "{reply:?}"
    );
    assert_eq!(daemon.status().served, 0);
}

#[test]
fn stop_ends_the_daemon_and_removes_its_socket() {
    let daemon = Running::new(false);
    let socket = daemon.socket.clone();
    stdout(daemon.ask(&run_request(&["echo"])));
    assert_eq!(
        daemon.ask(&Request::new(build(), Body::Stop)),
        Ok(Reply::Stopping)
    );
    assert_eq!(daemon.finished().unwrap().requests, 1);
    assert!(!socket.exists());
    assert_eq!(
        ask(&socket, &run_request(&["echo"])),
        Err(ClientError::Absent)
    );
}

/// An endpoint file no daemon answers behind: a socket nothing listens on, or on Windows a file
/// naming a pipe that does not exist.
fn stale_endpoint(socket: &Path) {
    #[cfg(unix)]
    drop(std::os::unix::net::UnixListener::bind(socket).unwrap());
    #[cfg(windows)]
    std::fs::write(socket, "stale\n").unwrap();
}

#[test]
fn a_stale_socket_is_unanswered_and_a_new_daemon_replaces_it() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join(".cupel/serve.sock");
    std::fs::create_dir_all(socket.parent().unwrap()).unwrap();
    stale_endpoint(&socket);
    assert!(socket.exists());
    assert!(
        matches!(
            ask(&socket, &run_request(&["echo"])),
            Err(ClientError::Unanswered(_))
        ),
        "a socket file with no listener"
    );
    let daemon = Running::start(dir.path(), false);
    assert_eq!(stdout(daemon.ask(&run_request(&["echo"]))), "echo");
}

#[test]
fn a_second_daemon_on_a_live_socket_is_refused_with_the_first_one_s_status() {
    let daemon = Running::new(false);
    let second = Daemon {
        root: daemon.root.clone(),
        socket: daemon.socket.clone(),
        build: build(),
        path: None,
        watch: false,
    };
    let refused = serve(
        &second,
        Arc::new(Script::default()),
        &Cancellation::new(),
        &mut |_| panic!("the second daemon must not start"),
    );
    let Err(ServeError::Running(status)) = refused else {
        panic!("not refused: {refused:?}")
    };
    assert_eq!(status.pid, std::process::id());
    assert_eq!(stdout(daemon.ask(&run_request(&["echo"]))), "echo");
}

#[test]
fn concurrent_clients_run_at_once_and_each_gets_its_own_answer() {
    let daemon = Running::new(false);
    // Each run answers its id only once all four are in flight together.
    let clients: Vec<JoinHandle<String>> = (0..4)
        .map(|client| {
            let socket = daemon.socket.clone();
            thread::spawn(move || {
                let id = client.to_string();
                stdout(ask(&socket, &run_request(&["gather", "4", &id])))
            })
        })
        .collect();
    let answers: Vec<String> = clients.into_iter().map(|c| c.join().unwrap()).collect();
    assert_eq!(answers, ["0", "1", "2", "3"]);
    assert_eq!(daemon.status().served, 4);
}

#[test]
fn a_client_that_hangs_up_cancels_its_run() {
    let daemon = Running::new(false);
    let mut stream = Connection::connect(&daemon.socket).unwrap();
    cupel_serve::write_message(&mut stream, &run_request(&["wait"])).unwrap();
    assert!(within_hang(|| daemon.script.begun() == 1), "the run began");
    drop(stream);
    within_hang(|| !daemon.script.finished().is_empty());
    assert_eq!(daemon.script.finished(), ["cancelled"]);
}

#[test]
fn stopping_cancels_runs_in_flight_and_they_still_answer() {
    let daemon = Running::new(false);
    let socket = daemon.socket.clone();
    let waiting = thread::spawn(move || stdout(ask(&socket, &run_request(&["wait"]))));
    assert!(within_hang(|| daemon.script.begun() == 1), "the run began");
    daemon.ask(&Request::new(build(), Body::Stop)).unwrap();
    assert_eq!(waiting.join().unwrap(), "cancelled");
    assert_eq!(daemon.finished().unwrap().requests, 1);
}

#[test]
fn removing_the_socket_file_ends_the_daemon() {
    let daemon = Running::new(false);
    std::fs::remove_file(&daemon.socket).unwrap();
    // Nothing else ends this daemon: `finished` returns only because it noticed.
    assert_eq!(daemon.finished().unwrap().requests, 0);
}

#[test]
fn the_watcher_hands_changed_files_to_the_handler() {
    let daemon = Running::new(true);
    let file = daemon.root.join("a.rs");
    // The watcher may start a moment after the daemon answers; write until it reports.
    within_hang(|| {
        std::fs::write(&file, "fn a() {}\n").unwrap();
        thread::sleep(Duration::from_millis(100));
        daemon.script.changed.lock().unwrap().contains(&file)
    });
    let status = daemon.status();
    let changed = daemon.script.changed.lock().unwrap().contains(&file);
    assert_eq!(
        (status.watching, changed, status.reparsed >= 1),
        (true, true, true)
    );
}

#[test]
fn an_endpoint_that_cannot_be_made_is_an_io_error_naming_it() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("not-a-directory");
    std::fs::write(&blocker, "").unwrap();
    let socket = blocker.join("serve.sock");
    let daemon = Daemon {
        root: dir.path().to_path_buf(),
        socket: socket.clone(),
        build: build(),
        path: None,
        watch: false,
    };
    let refused = serve(
        &daemon,
        Arc::new(Script::default()),
        &Cancellation::new(),
        &mut |_| panic!("a daemon with no endpoint must not start"),
    );
    let Err(ServeError::Io { socket: at, .. }) = refused else {
        panic!("not an I/O error: {refused:?}")
    };
    assert_eq!(at, socket);
}
