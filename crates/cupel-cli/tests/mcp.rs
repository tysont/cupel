// ABOUTME: Integration tests for `cupel serve --mcp` (ADR-121) with the real binary over stdio: the handshake, the
// ABOUTME: tools, envelopes equal to cold runs, events as progress, usage errors, a clean stdout, and the daemon.
mod support;

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use cupel_core::Envelope;
use cupel_serve::mcp::Client;
use serde_json::{Value, json};
use support::{Outcome, Repo};

/// How long any one message may take to arrive.
const PATIENCE: Duration = Duration::from_secs(120);

/// A repository holding a function of eleven decisions, over `static.complexity`'s limit.
fn repo() -> Repo {
    let repo = Repo::new();
    let chain: Vec<String> = (1..=11).map(|n| format!("if a == {n} {{ {n} }}")).collect();
    let lib = format!(
        "pub fn branchy(a: i32) -> i32 {{\n    {} else {{ 0 }}\n}}\n",
        chain.join(" else ")
    );
    repo.write("src/lib.rs", &lib);
    repo
}

/// The `cupel` binary run in `root` with no `CUPEL_*` variable from the caller.
fn binary(root: &Path) -> Command {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("cupel"));
    command.current_dir(root);
    for (key, _) in std::env::vars() {
        if key.starts_with("CUPEL_") {
            command.env_remove(key);
        }
    }
    command
}

/// A `cupel serve --mcp` process in a repository, spoken to over its stdin and stdout.
struct Mcp {
    child: Child,
    client: Client,
}

impl Mcp {
    fn start(root: &Path, extra: &[&str]) -> Self {
        let mut child = binary(root)
            .args(["serve", "--mcp"])
            .args(extra)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("cupel serve --mcp starts");
        let input = child.stdin.take().unwrap();
        let output = child.stdout.take().unwrap();
        let client = Client::new(input, output, PATIENCE);
        Self { child, client }
    }

    fn send(&mut self, message: &Value) {
        self.client.send(&message.to_string());
    }

    fn ask(&mut self, id: u64, method: &str, params: &Value) -> Vec<Value> {
        self.client.ask(&json!(id), method, &params.to_string())
    }

    fn initialize(&mut self) -> Value {
        let params = json!({"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "test", "version": "1"}});
        let response = self.ask(1, "initialize", &params).pop().unwrap();
        self.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        response
    }

    /// The result of calling `tool` with `arguments` as request `id`.
    fn call(&mut self, id: u64, tool: &str, arguments: &Value) -> Value {
        let params = json!({"name": tool, "arguments": arguments});
        let response = self.ask(id, "tools/call", &params).pop().unwrap();
        response["result"].clone()
    }

    /// Closes stdin and returns the exit code, what stdout still held, and stderr.
    fn close(mut self) -> (i32, Vec<String>, String) {
        self.client.close();
        let deadline = Instant::now() + Duration::from_secs(60);
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "the server never exited");
            thread::sleep(Duration::from_millis(20));
        };
        let rest = self.client.unread();
        let mut stderr = String::new();
        let mut pipe = self.child.stderr.take().unwrap();
        std::io::Read::read_to_string(&mut pipe, &mut stderr).unwrap();
        (status.code().unwrap(), rest, stderr)
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// `args` run cold in `root`, with no `--cwd`, as the server's calls run.
fn cold(root: &Path, args: &[&str]) -> Outcome {
    let output = binary(root)
        .args(args)
        .env("CUPEL_NO_DAEMON", "1")
        .stdin(Stdio::null())
        .output()
        .expect("cupel runs");
    Outcome {
        code: output.status.code().expect("exit code"),
        stdout: String::from_utf8(output.stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(output.stderr).expect("stderr is UTF-8"),
    }
}

fn redacted(document: &Value, root: &Path) -> Value {
    let envelope: Envelope = serde_json::from_value(document.clone())
        .unwrap_or_else(|error| panic!("{error}: {document}"));
    cupel_fixtures::redact(&envelope, root)
}

/// The structured content of `result`, after checking its text content is the same document.
fn structured(result: &Value) -> Value {
    let text = result["content"][0]["text"].as_str().expect("text content");
    let document: Value = serde_json::from_str(text).expect("the text is the envelope");
    assert_eq!(result["structuredContent"], document);
    document
}

/// A repository with a server on it.
fn served(extra: &[&str]) -> (Repo, PathBuf, Mcp) {
    let repo = repo();
    let root = repo.root();
    let mcp = Mcp::start(&root, extra);
    (repo, root, mcp)
}

#[test]
fn serve_mcp_initializes_and_lists_the_tools() {
    let (_repo, _root, mut mcp) = served(&[]);
    let initialized = mcp.initialize();
    assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(
        initialized["result"]["serverInfo"],
        json!({"name": "cupel", "version": env!("CARGO_PKG_VERSION")})
    );
    let listed = mcp.ask(2, "tools/list", &json!({})).pop().unwrap();
    let tools = listed["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        ["check", "survey", "explain", "tests_impacted", "describe"]
    );
    let closed: Vec<&Value> = tools
        .iter()
        .map(|tool| &tool["inputSchema"]["additionalProperties"])
        .collect();
    assert_eq!(closed, [&json!(false); 5]);
}

#[test]
fn a_check_call_returns_the_envelope_cupel_check_prints_for_the_same_arguments() {
    let (repo, root, mut mcp) = served(&[]);
    mcp.initialize();
    let result = mcp.call(2, "check", &json!({"only": ["static.complexity"]}));
    let command = ["check", "--only", "static.complexity", "--format", "json"];
    assert_eq!(result["_meta"]["cupel.dev/command"], json!(command));
    assert_eq!(result["isError"], false);
    let served = structured(&result);
    let local = cold(&root, &command);
    assert_eq!(redacted(&served, &root), redacted(&local.envelope(), &root));
    assert_eq!(served["findings"][0]["kind"], "complexity.high");
    // A failed gate is a result, not an error.
    repo.write(
        "cupel.toml",
        "[gate.tree]\n\"static.complexity\" = \"none\"\n",
    );
    let gated = mcp.call(
        3,
        "check",
        &json!({"only": ["static.complexity"], "gate": true}),
    );
    assert_eq!(structured(&gated)["exit_code"], 1);
    assert_eq!(gated["isError"], false);
}

/// Calls `tool` as request `id` and checks it ran `command` and returned what a cold run of it
/// prints, marked as an error exactly when that run's exit code is one.
fn assert_answers_as_cold(mcp: &mut Mcp, root: &Path, call: (u64, &str, &Value), command: &[&str]) {
    let (id, tool, arguments) = call;
    let result = mcp.call(id, tool, arguments);
    assert_eq!(result["_meta"]["cupel.dev/command"], json!(command));
    let local = cold(root, command);
    assert_eq!(
        redacted(&structured(&result), root),
        redacted(&local.envelope(), root),
        "{tool}"
    );
    assert_eq!(result["isError"], json!(matches!(local.code, 2..=4 | 6)));
}

#[test]
fn the_other_tools_answer_as_their_verbs_do() {
    let (_repo, root, mut mcp) = served(&[]);
    assert_answers_as_cold(
        &mut mcp,
        &root,
        (2, "describe", &json!({"subject": ["check"]})),
        &["describe", "check", "--format", "json"],
    );
    assert_answers_as_cold(
        &mut mcp,
        &root,
        (3, "tests_impacted", &json!({})),
        &["tests", "--impacted", "--format", "json"],
    );
    assert_answers_as_cold(
        &mut mcp,
        &root,
        (4, "survey", &json!({"only": ["static.complexity"]})),
        &["survey", "--only", "static.complexity", "--format", "json"],
    );
    assert_answers_as_cold(
        &mut mcp,
        &root,
        (5, "explain", &json!({"target": "static.complexity"})),
        &["explain", "static.complexity", "--format", "json"],
    );
}

/// `event` with its volatile values masked.
fn masked(mut event: Value) -> Value {
    for key in ["run", "duration_ms", "est_ms", "eta_ms"] {
        if event.get(key).is_some() {
            event[key] = json!("<masked>");
        }
    }
    event
}

#[test]
fn events_arrive_as_progress_notifications_in_the_order_a_cold_run_writes_them() {
    let (_repo, root, mut mcp) = served(&[]);
    let params = json!({
        "name": "check",
        "arguments": {"only": ["static.complexity"]},
        "_meta": {"progressToken": 7},
    });
    let messages = mcp.ask(2, "tools/call", &params);
    let progress: Vec<&Value> = messages
        .iter()
        .filter(|message| message["method"] == "notifications/progress")
        .map(|message| &message["params"])
        .collect();
    let counts: Vec<u64> = progress
        .iter()
        .map(|params| params["progress"].as_u64().unwrap())
        .collect();
    assert_eq!(counts, (1..=progress.len() as u64).collect::<Vec<_>>());
    assert!(progress.iter().all(|params| params["progressToken"] == 7));
    let served: Vec<Value> = progress
        .iter()
        .map(|params| masked(params["_meta"]["cupel.dev/event"].clone()))
        .collect();
    let args = ["check", "--only", "static.complexity", "--format", "json"];
    let local = cold(&root, &[&args[..], &["--events", "stderr"]].concat());
    let written: Vec<Value> = local
        .stderr
        .lines()
        .map(|line| masked(serde_json::from_str(line).unwrap()))
        .collect();
    assert_eq!(served, written);
    assert_eq!(served.last().unwrap()["ev"], "run.end");
}

/// The argument the usage envelope of a `check` call with `arguments` names, after checking the
/// call is marked as an error with exit 2.
fn refused_argument(mcp: &mut Mcp, id: u64, arguments: &Value) -> Value {
    let refused = mcp.call(id, "check", arguments);
    assert_eq!(refused["isError"], true);
    let envelope = structured(&refused);
    assert_eq!(
        (&envelope["exit_code"], &envelope["status"]),
        (&json!(2), &json!("error"))
    );
    assert_eq!(envelope["findings"][0]["kind"], "cli.usage");
    envelope["findings"][0]["evidence"]["argument"].clone()
}

#[test]
fn a_bad_argument_is_a_usage_envelope_marked_as_an_error() {
    let (_repo, _root, mut mcp) = served(&[]);
    let mistyped = refused_argument(&mut mcp, 2, &json!({"only": "static.complexity"}));
    assert_eq!(mistyped, "only");
    assert_eq!(
        refused_argument(&mut mcp, 3, &json!({"format": "text"})),
        "format"
    );
    assert_eq!(
        refused_argument(&mut mcp, 4, &json!({"events": "stderr"})),
        "events"
    );
    // A value of the right type that the verb rejects is the verb's own usage error.
    assert_eq!(
        refused_argument(&mut mcp, 5, &json!({"timeout": "soon"})),
        "--timeout"
    );
    let unknown = mcp
        .ask(6, "tools/call", &json!({"name": "serve"}))
        .pop()
        .unwrap();
    assert_eq!(unknown["error"]["code"], -32602);
}

#[test]
fn stdout_carries_only_protocol_messages_and_the_serve_envelope_goes_to_stderr() {
    let (_repo, _root, mut mcp) = served(&["--quiet"]);
    mcp.initialize();
    mcp.call(2, "check", &json!({"only": ["static.complexity"]}));
    let received = mcp.client.received();
    let written: Vec<Value> = received
        .iter()
        .map(|line| serde_json::from_str::<Value>(line).unwrap()["jsonrpc"].clone())
        .collect();
    assert_eq!(written, vec![json!("2.0"); received.len()]);
    let (code, rest, stderr) = mcp.close();
    assert_eq!(code, 0, "{stderr}");
    assert!(rest.is_empty(), "{rest:?}");
    let envelope: Value = serde_json::from_str(&stderr).expect("stderr is the serve envelope");
    assert_eq!(envelope["findings"][0]["kind"], "serve.mcp-stopped");
    assert_eq!(envelope["findings"][0]["evidence"]["requests"], 1);
}

/// Waits up to thirty seconds for `path` to exist.
fn appears(path: &Path) -> bool {
    let deadline = Instant::now() + Duration::from_secs(30);
    std::iter::from_fn(|| {
        thread::sleep(Duration::from_millis(20));
        Some(path.exists())
    })
    .find(|exists| *exists || Instant::now() >= deadline)
    .unwrap_or(false)
}

#[test]
fn serve_mcp_runs_beside_a_socket_daemon_and_takes_no_socket_flags() {
    let (_repo, root, mut mcp) = served(&["--watch"]);
    let mut daemon = binary(&root)
        .args(["serve", "--quiet", "--timeout", "300s"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    assert!(
        appears(&root.join(".cupel/serve.sock")),
        "the daemon listens"
    );
    let result = mcp.call(2, "check", &json!({"only": ["static.complexity"]}));
    assert_eq!(structured(&result)["exit_code"], 0);
    drop(mcp);
    let stopped = cold(&root, &["serve", "--stop", "--format", "json"]);
    assert_eq!(stopped.finding()["evidence"]["requests"], json!(null));
    let _ = daemon.wait();
    let socket = cold(
        &root,
        &["serve", "--mcp", "--socket", "x.sock", "--format", "json"],
    );
    let stop = cold(&root, &["serve", "--mcp", "--stop", "--format", "json"]);
    let kinds = [
        socket.finding()["kind"].clone(),
        stop.finding()["kind"].clone(),
    ];
    assert_eq!(
        (socket.code, stop.code, kinds),
        (2, 2, [json!("cli.usage"), json!("cli.usage")])
    );
}
