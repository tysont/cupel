// ABOUTME: Tests for the daemon's server and client over real Unix sockets with a scripted handler: answers,
// ABOUTME: streaming order, refusals, malformed requests, stop, stale sockets, concurrency, hang-ups, the watcher.
use std::io::Write;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cupel_core::Cancellation;
use cupel_serve::{
    Body, Build, ClientError, Daemon, Done, Handler, Invocation, Reply, Request, ServeError,
    Served, Status, request, serve,
};

/// A handler scripted by the invocation's first argument.
#[derive(Default)]
struct Script {
    cancelled: AtomicBool,
    changed: Mutex<Vec<PathBuf>>,
}

impl Handler for Script {
    fn run(
        &self,
        invocation: Invocation,
        mut stderr: Box<dyn Write + Send>,
        cancel: Cancellation,
    ) -> Done {
        let args = invocation.args;
        let done = |stdout: &str| Done {
            stdout: stdout.to_owned(),
            stderr: "rest\n".to_owned(),
            code: 0,
        };
        match args[0].as_str() {
            "lines" => {
                for line in 0..3 {
                    writeln!(stderr, "line {line}").unwrap();
                    thread::sleep(Duration::from_millis(100));
                }
                done("out")
            }
            "sleep" => {
                thread::sleep(Duration::from_millis(args[1].parse().unwrap()));
                done(&args[2])
            }
            "wait" => {
                let finished = cancel.wait(Duration::from_secs(30));
                self.cancelled.store(!finished, Ordering::SeqCst);
                done(if finished { "finished" } else { "cancelled" })
            }
            _ => done(&args.join(" ")),
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

/// A daemon serving `root` from a thread; dropping it stops it and waits for it.
struct Running {
    socket: PathBuf,
    stop: Cancellation,
    script: Arc<Script>,
    thread: Option<JoinHandle<Result<Served, ServeError>>>,
}

impl Running {
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
        started.recv_timeout(Duration::from_secs(10)).unwrap();
        Self {
            socket,
            stop,
            script,
            thread: Some(thread),
        }
    }

    /// Waits for the daemon to exit on its own.
    fn finished(mut self) -> Result<Served, ServeError> {
        self.thread.take().unwrap().join().unwrap()
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop.cancel();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
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

fn status(socket: &Path) -> Status {
    match ask(socket, &Request::new(build(), Body::Status)).unwrap() {
        Reply::Status(status) => status,
        other => panic!("not a status: {other:?}"),
    }
}

fn stdout(reply: Result<Reply, ClientError>) -> String {
    match reply {
        Ok(Reply::Done(done)) => done.stdout,
        other => panic!("not done: {other:?}"),
    }
}

#[test]
fn the_daemon_says_what_it_is_and_runs_requests() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = Running::start(dir.path(), false);
    let before = status(&daemon.socket);
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
    assert_eq!(
        stdout(ask(&daemon.socket, &run_request(&["echo", "hi"]))),
        "echo hi"
    );
    assert_eq!(status(&daemon.socket).served, 1);
}

#[test]
fn stderr_reaches_the_client_in_order_while_the_run_goes_on() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = Running::start(dir.path(), false);
    let started = Instant::now();
    let mut lines = Vec::new();
    let reply = request(&daemon.socket, &run_request(&["lines"]), &mut |text| {
        lines.push((text.to_owned(), started.elapsed()));
    });
    let finished = started.elapsed();
    let Ok(Reply::Done(done)) = reply else {
        panic!("not done: {reply:?}")
    };
    let texts: Vec<&str> = lines.iter().map(|(text, _)| text.as_str()).collect();
    assert_eq!(texts, ["line 0\n", "line 1\n", "line 2\n"]);
    assert_eq!(
        (done.stdout.as_str(), done.stderr.as_str()),
        ("out", "rest\n")
    );
    let ahead = finished.saturating_sub(lines[0].1);
    assert!(
        ahead >= Duration::from_millis(150),
        "the first line arrived {ahead:?} before the end, not as it was written"
    );
}

#[test]
fn a_run_from_another_build_protocol_or_path_is_refused_but_status_and_stop_are_answered() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = Running::start(dir.path(), false);
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
    let reasons: Vec<String> = [from_other, old_protocol, other_path]
        .iter()
        .map(|request_| match ask(&daemon.socket, request_) {
            Err(ClientError::Refused(reason)) => reason,
            other => panic!("not refused: {other:?}"),
        })
        .collect();
    assert!(
        reasons[0].contains("another build of cupel (cupel 0.1.0 (binary 1-2))")
            && reasons[0].contains("cupel serve --stop"),
        "{}",
        reasons[0]
    );
    assert!(reasons[1].contains("protocol 0"), "{}", reasons[1]);
    assert!(reasons[2].contains("another PATH"), "{}", reasons[2]);
    assert_eq!(status(&daemon.socket).served, 0);

    let answered = ask(&daemon.socket, &Request::new(other.clone(), Body::Status));
    assert!(matches!(answered, Ok(Reply::Status(_))), "{answered:?}");
    let stopped = ask(&daemon.socket, &Request::new(other, Body::Stop));
    assert_eq!(stopped, Ok(Reply::Stopping));
    assert_eq!(daemon.finished().unwrap().requests, 0);
}

#[test]
fn a_malformed_request_gets_an_error_and_the_daemon_keeps_serving() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = Running::start(dir.path(), false);
    let mut stream = UnixStream::connect(&daemon.socket).unwrap();
    stream.write_all(b"{\"type\":\"dance\"}\n").unwrap();
    let reply: Reply = cupel_serve::read_message(&mut std::io::BufReader::new(&stream))
        .unwrap()
        .unwrap();
    assert!(
        matches!(&reply, Reply::Error { message } if message.starts_with("the message is not valid")),
        "{reply:?}"
    );
    assert_eq!(status(&daemon.socket).served, 0);
}

#[test]
fn stop_ends_the_daemon_and_removes_its_socket() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = Running::start(dir.path(), false);
    let socket = daemon.socket.clone();
    stdout(ask(&socket, &run_request(&["echo"])));
    assert_eq!(
        ask(&socket, &Request::new(build(), Body::Stop)),
        Ok(Reply::Stopping)
    );
    assert_eq!(daemon.finished().unwrap().requests, 1);
    assert!(!socket.exists());
    assert_eq!(
        ask(&socket, &run_request(&["echo"])),
        Err(ClientError::Absent)
    );
}

#[test]
fn a_stale_socket_is_unanswered_and_a_new_daemon_replaces_it() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join(".cupel/serve.sock");
    std::fs::create_dir_all(socket.parent().unwrap()).unwrap();
    drop(UnixListener::bind(&socket).unwrap());
    assert!(socket.exists());
    assert!(
        matches!(
            ask(&socket, &run_request(&["echo"])),
            Err(ClientError::Unanswered(_))
        ),
        "a socket file with no listener"
    );
    let daemon = Running::start(dir.path(), false);
    assert_eq!(stdout(ask(&daemon.socket, &run_request(&["echo"]))), "echo");
}

#[test]
fn a_second_daemon_on_a_live_socket_is_refused_with_the_first_one_s_status() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = Running::start(dir.path(), false);
    let second = Daemon {
        root: dir.path().to_path_buf(),
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
    assert_eq!(stdout(ask(&daemon.socket, &run_request(&["echo"]))), "echo");
}

#[test]
fn concurrent_clients_run_at_once_and_each_gets_its_own_answer() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = Running::start(dir.path(), false);
    let started = Instant::now();
    let clients: Vec<JoinHandle<String>> = (0..4)
        .map(|client| {
            let socket = daemon.socket.clone();
            thread::spawn(move || {
                let id = client.to_string();
                stdout(ask(&socket, &run_request(&["sleep", "400", &id])))
            })
        })
        .collect();
    let answers: Vec<String> = clients.into_iter().map(|c| c.join().unwrap()).collect();
    assert_eq!(answers, ["0", "1", "2", "3"]);
    assert!(
        started.elapsed() < Duration::from_millis(1200),
        "four 400 ms runs took {:?}: they ran one at a time",
        started.elapsed()
    );
    assert_eq!(status(&daemon.socket).served, 4);
}

/// Waits up to ten seconds for `condition`.
fn eventually(condition: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn a_client_that_hangs_up_cancels_its_run() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = Running::start(dir.path(), false);
    let mut stream = UnixStream::connect(&daemon.socket).unwrap();
    cupel_serve::write_message(&mut stream, &run_request(&["wait"])).unwrap();
    thread::sleep(Duration::from_millis(100));
    drop(stream);
    assert!(eventually(|| daemon
        .script
        .cancelled
        .load(Ordering::SeqCst)));
}

#[test]
fn stopping_cancels_runs_in_flight_and_they_still_answer() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = Running::start(dir.path(), false);
    let socket = daemon.socket.clone();
    let waiting = thread::spawn(move || stdout(ask(&socket, &run_request(&["wait"]))));
    // The handler gives no sign that the run started; give it time to.
    thread::sleep(Duration::from_millis(300));
    ask(&daemon.socket, &Request::new(build(), Body::Stop)).unwrap();
    assert_eq!(waiting.join().unwrap(), "cancelled");
    assert_eq!(daemon.finished().unwrap().requests, 1);
}

#[test]
fn removing_the_socket_file_ends_the_daemon() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = Running::start(dir.path(), false);
    std::fs::remove_file(&daemon.socket).unwrap();
    let started = Instant::now();
    assert_eq!(daemon.finished().unwrap().requests, 0);
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn the_watcher_hands_changed_files_to_the_handler() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let daemon = Running::start(&root, true);
    assert!(status(&daemon.socket).watching);
    let file = root.join("a.rs");
    // The watcher may start a moment after the daemon answers; write until it reports.
    assert!(eventually(|| {
        std::fs::write(&file, "fn a() {}\n").unwrap();
        thread::sleep(Duration::from_millis(100));
        daemon.script.changed.lock().unwrap().contains(&file)
    }));
    assert!(status(&daemon.socket).reparsed >= 1);
}
