// ABOUTME: Integration tests for `cupel serve` and auto-connect (ADR-085) with the real binary: answers, bypass,
// ABOUTME: stale sockets, other builds, concurrency, streamed events, stop, watch-and-reparse, and doctor's report.
mod support;

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use cupel_core::Envelope;
use cupel_serve::{Body, Build, Reply, Request, Status};
use serde_json::{Value, json};
use support::{Outcome, Repo, cupel, run_with};

/// `src/lib.rs` with a function of `branches` decisions (over the default `cyclomatic_max` of 10
/// from 10 on) and a simple one, so `static.complexity` has a finding to report.
fn lib(branches: i32) -> String {
    let chain: Vec<String> = (1..=branches)
        .map(|n| format!("if a == {n} {{ {n} }}"))
        .collect();
    format!(
        "pub fn branchy(a: i32) -> i32 {{\n    {} else {{ 0 }}\n}}\n\npub fn plain() -> i32 {{\n    1\n}}\n",
        chain.join(" else ")
    )
}

const CHECK: [&str; 5] = ["check", "--only", "static.complexity", "--format", "json"];

/// A repository holding `src/lib.rs`.
fn repo() -> Repo {
    let repo = Repo::new();
    repo.write("src/lib.rs", &lib(11));
    repo
}

/// A `cupel serve` process for one repository; dropping it stops it, killing it if it will not.
struct Daemon {
    child: Option<Child>,
    socket: PathBuf,
}

impl Daemon {
    fn start(repo: &Repo, extra: &[&str]) -> Self {
        let mut command = Command::new(assert_cmd::cargo::cargo_bin("cupel"));
        command
            .args(["serve", "--timeout", "300s", "--format", "json"])
            .args(extra)
            .current_dir(repo.root())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, _) in std::env::vars() {
            if key.starts_with("CUPEL_") {
                command.env_remove(key);
            }
        }
        let daemon = Self {
            child: Some(command.spawn().expect("cupel serve starts")),
            socket: repo.root().join(".cupel/serve.sock"),
        };
        assert!(
            eventually(|| daemon.try_status().is_some()),
            "the daemon never answered"
        );
        daemon
    }

    fn try_status(&self) -> Option<Status> {
        let status = Request::new(build(), Body::Status);
        match cupel_serve::request(&self.socket, &status, &mut |_| {}) {
            Ok(Reply::Status(status)) => Some(status),
            _ => None,
        }
    }

    fn status(&self) -> Status {
        self.try_status().expect("the daemon answers")
    }

    /// Stops the daemon with `cupel serve --stop` and returns that and the daemon's own outcome.
    fn stop(mut self, repo: &Repo) -> (Outcome, Outcome) {
        let stopped = repo.run(&["serve", "--stop", "--format", "json"]);
        let output = self.child.take().unwrap().wait_with_output().unwrap();
        let served = Outcome {
            code: output.status.code().unwrap(),
            stdout: String::from_utf8(output.stdout).unwrap(),
            stderr: String::from_utf8(output.stderr).unwrap(),
        };
        (stopped, served)
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        let stop = Request::new(build(), Body::Stop);
        let _ = cupel_serve::request(&self.socket, &stop, &mut |_| {});
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// The build of the `cupel` binary the tests run.
fn build() -> Build {
    Build::of(
        env!("CARGO_PKG_VERSION"),
        &assert_cmd::cargo::cargo_bin("cupel"),
    )
}

/// Waits up to fifteen seconds for `condition`.
fn eventually(condition: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        thread::sleep(Duration::from_millis(25));
    }
    false
}

/// `outcome`'s envelope with every volatile field redacted (ADR-003).
fn redacted(outcome: &Outcome, root: &Path) -> Value {
    let envelope: Envelope = serde_json::from_str(&outcome.stdout)
        .unwrap_or_else(|error| panic!("{error}: {}{}", outcome.stdout, outcome.stderr));
    cupel_fixtures::redact(&envelope, root)
}

/// `args` run cold in `repo`.
fn cold(repo: &Repo, args: &[&str]) -> Outcome {
    repo.run_with(cupel().env("CUPEL_NO_DAEMON", "1"), args)
}

#[test]
fn a_running_daemon_answers_the_cli_with_what_a_cold_run_prints() {
    let repo = repo();
    let daemon = Daemon::start(&repo, &[]);
    let served = repo.run(&CHECK);
    assert_eq!(daemon.status().served, 1, "the run went through the daemon");
    let local = cold(&repo, &CHECK);
    assert_eq!(daemon.status().served, 1, "CUPEL_NO_DAEMON=1 runs locally");
    assert_eq!(
        (served.code, served.stderr.as_str()),
        (local.code, local.stderr.as_str())
    );
    assert_eq!(
        redacted(&served, &repo.root()),
        redacted(&local, &repo.root())
    );
    assert_eq!(
        served.envelope()["findings"][0]["kind"],
        json!("complexity.high")
    );
}

#[test]
fn a_stale_socket_file_is_ignored_and_the_run_is_local() {
    let repo = repo();
    std::fs::create_dir_all(repo.path(".cupel")).unwrap();
    drop(std::os::unix::net::UnixListener::bind(repo.path(".cupel/serve.sock")).unwrap());
    let outcome = repo.run(&CHECK);
    assert_eq!((outcome.code, outcome.stderr.as_str()), (0, ""));
    assert_eq!(outcome.envelope()["checks"][0]["status"], json!("fail"));
}

#[test]
fn a_client_of_another_build_is_refused_runs_locally_and_says_why() {
    let repo = repo();
    let daemon = Daemon::start(&repo, &[]);
    // A copy of the binary with a later modification time: another build of the same version.
    let other = tempfile::tempdir().unwrap();
    let copy = other.path().join("cupel");
    std::fs::copy(assert_cmd::cargo::cargo_bin("cupel"), &copy).unwrap();
    let later = std::time::SystemTime::now() + Duration::from_secs(60);
    std::fs::File::options()
        .write(true)
        .open(&copy)
        .unwrap()
        .set_modified(later)
        .unwrap();
    let outcome = repo.run_with(&mut assert_cmd::Command::new(&copy), &CHECK);
    assert_eq!(daemon.status().served, 0);
    assert_eq!(outcome.code, 0, "{}", outcome.stderr);
    assert!(
        outcome.stderr.starts_with("cupel: the daemon at ")
            && outcome.stderr.contains("runs another build of cupel")
            && outcome
                .stderr
                .ends_with("`cupel serve --stop`, then `cupel serve`.; ran without the daemon.\n"),
        "{}",
        outcome.stderr
    );
    let local = cold(&repo, &CHECK);
    assert_eq!(
        redacted(&outcome, &repo.root()),
        redacted(&local, &repo.root())
    );
    let quiet = repo.run_with(
        &mut assert_cmd::Command::new(&copy),
        &[&CHECK[..], &["--quiet"]].concat(),
    );
    assert_eq!(quiet.stderr, "");
}

#[test]
fn concurrent_clients_are_each_answered_by_the_daemon() {
    let repo = repo();
    let daemon = Daemon::start(&repo, &[]);
    let root = repo.root();
    let clients: Vec<_> = (0..4)
        .map(|_| {
            let root = root.display().to_string();
            thread::spawn(move || {
                run_with(
                    &mut cupel(),
                    [&CHECK[..], &["--cwd", root.as_str()]].concat(),
                )
            })
        })
        .collect();
    let outcomes: Vec<Outcome> = clients.into_iter().map(|c| c.join().unwrap()).collect();
    assert_eq!(daemon.status().served, 4);
    let local = redacted(&cold(&repo, &CHECK), &root);
    for outcome in &outcomes {
        assert_eq!(outcome.code, 0);
        assert_eq!(redacted(outcome, &root), local);
    }
}

/// The event lines of `stderr` with their volatile values masked.
fn events(stderr: &str) -> Vec<Value> {
    stderr
        .lines()
        .map(|line| {
            let mut event: Value = serde_json::from_str(line).unwrap();
            for key in ["run", "duration_ms", "est_ms"] {
                if event.get(key).is_some() {
                    event[key] = json!("<masked>");
                }
            }
            event
        })
        .collect()
}

#[test]
fn events_reach_the_client_in_the_order_a_cold_run_writes_them() {
    let repo = repo();
    let daemon = Daemon::start(&repo, &[]);
    let args = [&CHECK[..], &["--events", "stderr"]].concat();
    let served = repo.run(&args);
    assert_eq!(daemon.status().served, 1);
    let local = cold(&repo, &args);
    let kinds: Vec<Value> = events(&served.stderr)
        .iter()
        .map(|event| event["ev"].clone())
        .collect();
    assert_eq!(kinds.last(), Some(&json!("run.end")));
    assert!(kinds.len() >= 4, "{kinds:?}");
    assert_eq!(events(&served.stderr), events(&local.stderr));
}

#[test]
fn serve_stop_stops_the_daemon_and_reports_what_it_served() {
    let repo = repo();
    let daemon = Daemon::start(&repo, &["--quiet"]);
    repo.run(&CHECK);
    let (stopped, served) = daemon.stop(&repo);
    assert_eq!(stopped.code, 0, "{}", stopped.stdout);
    let finding = stopped.finding();
    assert_eq!(
        (finding["kind"].clone(), finding["title"].clone()),
        (
            json!("serve.stopped"),
            json!("Stopped the daemon at .cupel/serve.sock")
        )
    );
    assert_eq!((served.code, served.stderr.as_str()), (0, ""));
    let finding = served.finding();
    assert_eq!(
        finding["title"],
        json!("The daemon at .cupel/serve.sock stopped after serving 1 request")
    );
    assert!(!repo.path(".cupel/serve.sock").exists());
    let again = repo.run(&["serve", "--stop"]);
    assert_eq!(
        (again.code, again.finding()["kind"].clone()),
        (0, json!("serve.not-running"))
    );
}

#[test]
fn serve_announces_itself_and_refuses_to_start_beside_a_running_daemon() {
    let repo = repo();
    let daemon = Daemon::start(&repo, &[]);
    let second = repo.run(&["serve", "--timeout", "10s"]);
    assert_eq!(second.code, 3, "{}", second.stdout);
    let finding = second.finding();
    assert_eq!(finding["kind"], json!("serve.running"));
    assert_eq!(finding["evidence"]["socket"], json!(".cupel/serve.sock"));
    let (_, served) = daemon.stop(&repo);
    let announced = format!(
        "cupel: serving {} at .cupel/serve.sock (pid ",
        repo.root().display()
    );
    assert!(served.stderr.starts_with(&announced), "{}", served.stderr);
}

#[test]
fn serve_mcp_is_pending() {
    let repo = repo();
    let outcome = repo.run(&["serve", "--mcp"]);
    assert_eq!(outcome.code, 2);
    assert_eq!(outcome.finding()["kind"], json!("cli.not-implemented"));
}

#[test]
fn a_file_changed_under_watch_is_reparsed_and_the_next_run_matches_a_cold_one() {
    let repo = repo();
    let daemon = Daemon::start(&repo, &["--watch"]);
    assert!(daemon.status().watching);
    let first = repo.run(&CHECK);
    assert_eq!(first.code, 0);
    // `branchy` gains a branch, an edit inside the file that a reparse starts from.
    repo.write("src/lib.rs", &lib(14));
    assert!(
        eventually(|| daemon.status().reparsed >= 1),
        "the watcher never reparsed src/lib.rs"
    );
    let served = repo.run(&CHECK);
    assert_eq!(daemon.status().served, 2);
    let local = cold(&repo, &CHECK);
    assert_eq!(
        redacted(&served, &repo.root()),
        redacted(&local, &repo.root())
    );
    assert_ne!(
        redacted(&served, &repo.root())["findings"],
        redacted(&first, &repo.root())["findings"]
    );
}

#[test]
fn doctor_reports_a_running_daemon_and_a_stale_socket() {
    let repo = repo();
    let none = cold(&repo, &["doctor", "--format", "json"]);
    let kinds = |outcome: &Outcome| support::finding_fields(&outcome.envelope(), "kind");
    assert!(
        !kinds(&none)
            .iter()
            .any(|kind| kind.starts_with("doctor.daemon"))
    );

    let daemon = Daemon::start(&repo, &["--watch"]);
    let pid = daemon.status().pid;
    let running = cold(&repo, &["doctor", "--format", "json"]);
    let envelope = running.envelope();
    let finding = envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["kind"] == json!("doctor.daemon-running"))
        .unwrap_or_else(|| panic!("no daemon finding: {envelope}"))
        .clone();
    assert_eq!(finding["severity"], json!("info"));
    assert_eq!(
        finding["evidence"],
        json!({"socket": ".cupel/serve.sock", "version": env!("CARGO_PKG_VERSION"), "pid": pid, "watching": true})
    );
    drop(daemon);

    std::fs::create_dir_all(repo.path(".cupel")).unwrap();
    drop(std::os::unix::net::UnixListener::bind(repo.path(".cupel/serve.sock")).unwrap());
    let stale = cold(&repo, &["doctor", "--format", "json"]);
    assert!(
        kinds(&stale).contains(&"doctor.daemon-stale".to_owned()),
        "{}",
        stale.stdout
    );
    assert_eq!(stale.code, 0);
}
