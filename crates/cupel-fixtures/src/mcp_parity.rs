// ABOUTME: MCP parity (ADR-121): with MCP_PARITY=1, every `run_cupel` of a verb `serve --mcp` serves also runs as a
// ABOUTME: `tools/call` to a fresh server on an identical copy, and fails unless envelope and stderr match the cold run's.
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use cupel_serve::mcp::Client;
use serde_json::{Value, json};
use similar::TextDiff;

use crate::parity::{Twin, redacted};

/// How long one message from the server may take.
const PATIENCE: Duration = Duration::from_secs(600);

/// Whether this test process proves MCP parity: `MCP_PARITY=1` (`make parity`).
pub fn enabled() -> bool {
    std::env::var("MCP_PARITY").as_deref() == Ok("1")
}

/// The tool that runs `args`' verb, if one does (design §8.10).
pub fn tool(args: &[&str]) -> Option<&'static str> {
    match *args.first()? {
        "check" => Some("check"),
        "survey" => Some("survey"),
        "explain" => Some("explain"),
        "describe" => Some("describe"),
        "tests" if args.contains(&"--impacted") => Some("tests_impacted"),
        _ => None,
    }
}

impl Twin {
    /// Calls `tool` with the arguments that spell `args` on a `cupel serve --mcp` in the copy and
    /// compares the envelope's exit code, the redacted envelope, and the logged stderr with the
    /// cold run's. Returns whether it compared: not when `args` uses a flag or a value no tool
    /// takes (`--format text`), or the tool's command line differs from `args` (no `--format`,
    /// which every call adds, so the findings' `repro` would differ).
    ///
    /// # Panics
    ///
    /// When they differ, with a unified diff; when the server does not answer.
    pub fn assert_same_through_mcp(
        &self,
        tool: &str,
        args: &[&str],
        cold: (Option<i32>, &str, &str),
    ) -> bool {
        let args: Vec<String> = args.iter().map(|arg| self.moved(arg)).collect();
        let mut server = Server::start(&self.root);
        let schema = server.schema(tool);
        let Some(arguments) = arguments(tool, &args, &schema) else {
            return false;
        };
        if !arguments.iter().any(|(name, _)| name == "dry_run") {
            let warm = std::iter::once(("dry_run".to_owned(), json!(true)));
            server.call(tool, &warm.chain(arguments.clone()).collect::<Vec<_>>());
        }
        let (result, logged) = server.call(tool, &arguments);
        if result["_meta"]["cupel.dev/command"] != json!(args) {
            return false;
        }
        let document = result["content"][0]["text"].as_str().unwrap_or_default();
        let served_code = serde_json::from_str::<Value>(document)
            .ok()
            .and_then(|envelope| envelope["exit_code"].as_i64());
        let (code, stdout, stderr) = cold;
        let text = |code: Option<i64>, stdout: &str, stderr: &str, root: &Path| {
            format!(
                "exit {code:?}\n--- stdout\n{}--- stderr\n{}",
                redacted(stdout, root),
                redacted(stderr, root)
            )
        };
        let cold_text = text(code.map(i64::from), stdout, stderr, &self.original);
        let served_text = text(served_code, document, &logged, &self.root);
        if cold_text != served_text {
            let diff = TextDiff::from_lines(&cold_text, &served_text)
                .unified_diff()
                .header("cold", "mcp")
                .to_string();
            panic!("cupel {args:?} differs through serve --mcp (MCP_PARITY=1):\n{diff}");
        }
        true
    }
}

/// Arguments in the order a client writes them, which is the order of the flags in the command
/// line a call runs.
type Arguments = Vec<(String, Value)>;

/// The arguments of `tool` that spell `args` after its command words, by the properties of
/// `schema`: a switch is `true`, a repeated flag an array of the words up to the next flag, a
/// positional (`x-cupel-positional`) the words before the first flag. `None` when a flag is not
/// a property, or appears twice.
fn arguments(tool: &str, args: &[String], schema: &Value) -> Option<Arguments> {
    let properties = schema["properties"].as_object()?;
    let skipped = if tool == "tests_impacted" { 2 } else { 1 };
    let mut words = args.iter().skip(skipped).map(String::as_str).peekable();
    let mut arguments = Arguments::new();
    let positionals: Vec<&str> =
        std::iter::from_fn(|| words.next_if(|word| !word.starts_with("--"))).collect();
    if !positionals.is_empty() {
        let (name, property) = properties
            .iter()
            .find(|(_, property)| property["x-cupel-positional"] == true)?;
        let value = match positionals.as_slice() {
            _ if property["type"] == "array" => json!(positionals),
            [one] => json!(one),
            _ => return None,
        };
        arguments.push((name.clone(), value));
    }
    while let Some(flag) = words.next() {
        let (flag, inline) = flag
            .split_once('=')
            .map_or((flag, None), |(f, v)| (f, Some(v)));
        let name = flag.strip_prefix("--")?.replace('-', "_");
        let property = properties.get(&name)?;
        let value = match property["type"].as_str()? {
            "boolean" => json!(true),
            "array" => {
                let items: Vec<&str> = inline
                    .into_iter()
                    .chain(std::iter::from_fn(|| {
                        words.next_if(|word| !word.starts_with("--"))
                    }))
                    .collect();
                json!(items)
            }
            kind => {
                let word = inline.or_else(|| words.next())?;
                match kind {
                    "integer" => json!(word.parse::<u64>().ok()?),
                    _ => json!(word),
                }
            }
        };
        if arguments.iter().any(|(taken, _)| *taken == name) {
            return None;
        }
        arguments.push((name, value));
    }
    Some(arguments)
}

/// A `cupel serve --mcp` started for one comparison and killed after it.
struct Server {
    child: Child,
    client: Client,
    next_id: u64,
}

impl Server {
    /// Starts a server in `root` with the environment of a cold `run_cupel`: its own temporary
    /// directory, the twin's beside `root`.
    fn start(root: &Path) -> Self {
        let mut command = Command::new(assert_cmd::cargo::cargo_bin("cupel"));
        command
            .args(["serve", "--mcp", "--quiet"])
            .current_dir(root)
            .env("TMPDIR", root.parent().unwrap_or(root))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("CUPEL_") {
                command.env_remove(key);
            }
        }
        command.env("CUPEL_NO_DAEMON", "1");
        let mut child = command.spawn().expect("cupel serve --mcp starts");
        let input = child.stdin.take().expect("stdin is piped");
        let output = child.stdout.take().expect("stdout is piped");
        Self {
            child,
            client: Client::new(input, output, PATIENCE),
            next_id: 0,
        }
    }

    /// Sends a request with `params`, JSON text, and returns every message up to and including
    /// its response.
    fn ask(&mut self, method: &str, params: &str) -> Vec<Value> {
        self.next_id += 1;
        self.client.ask(&json!(self.next_id), method, params)
    }

    /// The input schema of `tool`.
    fn schema(&mut self, tool: &str) -> Value {
        let listed = self.ask("tools/list", "{}");
        let tools = listed.last().map(|response| &response["result"]["tools"]);
        tools
            .and_then(Value::as_array)
            .and_then(|tools| tools.iter().find(|listed| listed["name"] == tool))
            .map_or_else(
                || panic!("serve --mcp lists no tool {tool}"),
                |listed| listed["inputSchema"].clone(),
            )
    }

    /// Calls `tool` with `arguments`: its result and what it logged, one line each.
    fn call(&mut self, tool: &str, arguments: &[(String, Value)]) -> (Value, String) {
        let pairs: Vec<String> = arguments
            .iter()
            .map(|(name, value)| format!("{}:{value}", json!(name)))
            .collect();
        let params = format!(
            r#"{{"name":{},"arguments":{{{}}}}}"#,
            json!(tool),
            pairs.join(",")
        );
        let mut messages = self.ask("tools/call", &params);
        let response = messages.pop().expect("a response");
        let mut logged = String::new();
        for message in &messages {
            if message["method"] == "notifications/message"
                && let Some(line) = message["params"]["data"].as_str()
            {
                logged.push_str(line);
                logged.push('\n');
            }
        }
        (response["result"].clone(), logged)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
