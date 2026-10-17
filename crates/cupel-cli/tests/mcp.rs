// ABOUTME: Integration tests for `cupel serve --mcp` (ADR-121) with the real binary over stdio: the handshake, the
// ABOUTME: tools, envelopes equal to cold runs, events as progress, usage errors, a clean stdout, and the daemon.
mod support;

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use cupel_core::Envelope;
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
    input: Option<ChildStdin>,
    lines: Receiver<String>,
    /// Every line it wrote to stdout so far.
    written: Vec<String>,
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
        let input = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let (sender, lines) = mpsc::channel();
        thread::spawn(move || {
            for line in stdout.lines() {
                let Ok(line) = line else { return };
                if sender.send(line).is_err() {
                    return;
                }
            }
        });
        Self {
            child,
            input,
            lines,
            written: Vec::new(),
        }
    }

    fn send(&mut self, message: &Value) {
        let input = self.input.as_mut().expect("stdin is open");
        writeln!(input, "{message}").unwrap();
        input.flush().unwrap();
    }

    fn next(&mut self) -> Value {
        let line = self
            .lines
            .recv_timeout(PATIENCE)
            .expect("the server answers in time");
        self.written.push(line.clone());
        serde_json::from_str(&line).unwrap_or_else(|_| panic!("a JSON line: {line}"))
    }

    /// Sends request `id` and returns every message up to and including its response.
    fn ask(&mut self, id: u64, method: &str, params: &Value) -> Vec<Value> {
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let mut messages = Vec::new();
        loop {
            let message = self.next();
            let done = message["id"] == json!(id);
            messages.push(message);
            if done {
                return messages;
            }
        }
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
        drop(self.input.take());
        let deadline = Instant::now() + Duration::from_secs(60);
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "the server never exited");
            thread::sleep(Duration::from_millis(20));
        };
        let rest: Vec<String> = self.lines.try_iter().collect();
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

#[test]
fn serve_mcp_initializes_and_lists_the_tools() {
    let repo = repo();
    let mut mcp = Mcp::start(&repo.root(), &[]);
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
    for tool in tools {
        assert_eq!(tool["inputSchema"]["additionalProperties"], false, "{tool}");
    }
}

#[test]
fn a_check_call_returns_the_envelope_cupel_check_prints_for_the_same_arguments() {
    let repo = repo();
    let root = repo.root();
    let mut mcp = Mcp::start(&root, &[]);
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

#[test]
fn the_other_tools_answer_as_their_verbs_do() {
    let repo = repo();
    let root = repo.root();
    let mut mcp = Mcp::start(&root, &[]);
    let cases: [(&str, Value, &[&str]); 4] = [
        (
            "describe",
            json!({"subject": ["check"]}),
            &["describe", "check", "--format", "json"],
        ),
        (
            "tests_impacted",
            json!({}),
            &["tests", "--impacted", "--format", "json"],
        ),
        (
            "survey",
            json!({"only": ["static.complexity"]}),
            &["survey", "--only", "static.complexity", "--format", "json"],
        ),
        (
            "explain",
            json!({"target": "static.complexity"}),
            &["explain", "static.complexity", "--format", "json"],
        ),
    ];
    for (id, (tool, arguments, command)) in (2..).zip(cases) {
        let result = mcp.call(id, tool, &arguments);
        assert_eq!(result["_meta"]["cupel.dev/command"], json!(command));
        let local = cold(&root, command);
        assert_eq!(
            redacted(&structured(&result), &root),
            redacted(&local.envelope(), &root),
            "{tool}"
        );
        assert_eq!(result["isError"], json!(local.code >= 2 && local.code != 5));
    }
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
    let repo = repo();
    let root = repo.root();
    let mut mcp = Mcp::start(&root, &[]);
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

#[test]
fn a_bad_argument_is_a_usage_envelope_marked_as_an_error() {
    let repo = repo();
    let mut mcp = Mcp::start(&repo.root(), &[]);
    let mistyped = mcp.call(2, "check", &json!({"only": "static.complexity"}));
    assert_eq!(mistyped["isError"], true);
    let envelope = structured(&mistyped);
    assert_eq!(envelope["exit_code"], 2);
    assert_eq!(envelope["status"], "error");
    assert_eq!(envelope["findings"][0]["kind"], "cli.usage");
    assert_eq!(envelope["findings"][0]["evidence"]["argument"], "only");
    for (id, arguments, argument) in [
        (3, json!({"format": "text"}), "format"),
        (4, json!({"events": "stderr"}), "events"),
        // A value of the right type that the verb rejects is the verb's own usage error.
        (5, json!({"timeout": "soon"}), "--timeout"),
    ] {
        let refused = mcp.call(id, "check", &arguments);
        assert_eq!(refused["isError"], true);
        let envelope = structured(&refused);
        assert_eq!(envelope["findings"][0]["evidence"]["argument"], argument);
    }
    let unknown = mcp
        .ask(6, "tools/call", &json!({"name": "serve"}))
        .pop()
        .unwrap();
    assert_eq!(unknown["error"]["code"], -32602);
}

#[test]
fn stdout_carries_only_protocol_messages_and_the_serve_envelope_goes_to_stderr() {
    let repo = repo();
    let mut mcp = Mcp::start(&repo.root(), &["--quiet"]);
    mcp.initialize();
    mcp.call(2, "check", &json!({"only": ["static.complexity"]}));
    let written = mcp.written.clone();
    let (code, rest, stderr) = mcp.close();
    assert_eq!(code, 0, "{stderr}");
    assert!(rest.is_empty(), "{rest:?}");
    for line in written {
        let message: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(message["jsonrpc"], "2.0", "{line}");
    }
    let envelope: Value = serde_json::from_str(&stderr).expect("stderr is the serve envelope");
    assert_eq!(envelope["findings"][0]["kind"], "serve.mcp-stopped");
    assert_eq!(envelope["findings"][0]["evidence"]["requests"], 1);
}

#[test]
fn serve_mcp_runs_beside_a_socket_daemon_and_takes_no_socket_flags() {
    let repo = repo();
    let root = repo.root();
    let mut daemon = binary(&root)
        .args(["serve", "--quiet", "--timeout", "300s"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let socket = root.join(".cupel/serve.sock");
    let deadline = Instant::now() + Duration::from_secs(30);
    while !socket.exists() {
        assert!(Instant::now() < deadline, "the daemon never listened");
        thread::sleep(Duration::from_millis(20));
    }
    let mut mcp = Mcp::start(&root, &["--watch"]);
    let result = mcp.call(2, "check", &json!({"only": ["static.complexity"]}));
    assert_eq!(structured(&result)["exit_code"], 0);
    drop(mcp);
    let stopped = cold(&root, &["serve", "--stop", "--format", "json"]);
    assert_eq!(stopped.finding()["evidence"]["requests"], json!(null));
    let _ = daemon.wait();
    for flag in ["--socket", "--stop"] {
        let mut args = vec!["serve", "--mcp", flag];
        if flag == "--socket" {
            args.push("x.sock");
        }
        args.extend(["--format", "json"]);
        let refused = cold(&root, &args);
        assert_eq!(refused.code, 2, "{flag}: {}", refused.stdout);
        assert_eq!(refused.finding()["kind"], "cli.usage");
    }
}
