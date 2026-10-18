// ABOUTME: The MCP server behind `cupel serve --mcp` (ADR-121): JSON-RPC 2.0 over stdio, one message per line,
// ABOUTME: protocol 2025-11-25 or 2025-06-18; each tool call on its own thread, events as progress, stderr as log.
//! The Model Context Protocol transport, independent of what the tools are.
//!
//! [`serve`] reads requests from its input and answers on its output: `initialize`, `ping`,
//! `tools/list`, `tools/call`, and `logging/setLevel`; `notifications/cancelled` cancels a call.
//! A [`Tools`] implementation (`cupel-cli`'s runs the verbs) lists the tools and runs each call,
//! writing event lines and standard error as it goes; event lines become
//! `notifications/progress` under the request's `progressToken` and standard error lines become
//! `notifications/message` at level `info`.

use std::collections::HashMap;
use std::io::{BufReader, Read, Write};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cupel_core::Cancellation;
use serde::de::{Deserializer, MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::lines::Lines;
pub use crate::mcp_client::Client;
use crate::protocol::{MAX_MESSAGE, read_line_within, write_message};
use crate::server::Served;

/// The protocol versions this server speaks, latest first; `initialize` answers the client's
/// when it is one of them, else the first.
pub const VERSIONS: [&str; 2] = ["2025-11-25", "2025-06-18"];

/// How often the loop looks at its stop token while no message arrives.
const POLL: Duration = Duration::from_millis(200);

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// The log levels of `logging/setLevel`, least severe first.
const LEVELS: [&str; 8] = [
    "debug",
    "info",
    "notice",
    "warning",
    "error",
    "critical",
    "alert",
    "emergency",
];

/// The level standard error lines are logged at: `info`.
const STDERR_LEVEL: usize = 1;

/// The `_meta` key of the command line a call ran.
const COMMAND_KEY: &str = "cupel.dev/command";

/// The `_meta` key of the event a progress notification carries.
const EVENT_KEY: &str = "cupel.dev/event";

/// What `initialize` says the server is.
#[derive(Debug, Clone)]
pub struct Server {
    pub name: String,
    pub version: String,
}

/// A tool as `tools/list` lists it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub name: String,
    pub title: String,
    pub description: String,
    /// A JSON Schema (2020-12) object.
    pub input_schema: Value,
}

/// A call's arguments in the order the client wrote them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Arguments(pub Vec<(String, Value)>);

impl<'de> Deserialize<'de> for Arguments {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct InOrder;

        impl<'de> Visitor<'de> for InOrder {
            type Value = Arguments;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("an object of arguments")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Arguments, M::Error> {
                let mut pairs = Vec::new();
                while let Some(pair) = map.next_entry()? {
                    pairs.push(pair);
                }
                Ok(Arguments(pairs))
            }
        }

        deserializer.deserialize_map(InOrder)
    }
}

/// One call as a tool receives it.
pub struct Call {
    pub arguments: Arguments,
    /// Where the run's events go, one JSON line each.
    pub events: Box<dyn Write + Send>,
    /// Where the run's standard error goes.
    pub stderr: Box<dyn Write + Send>,
    /// Fires when the client cancels the call or the server stops.
    pub cancel: Cancellation,
}

/// What a call produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Called {
    /// The command line it ran, after the program name.
    pub command: Vec<String>,
    /// The JSON document it returns; text content always, structured content when an object.
    pub document: String,
    /// Whether the result is a tool execution error.
    pub is_error: bool,
}

/// What the server offers.
pub trait Tools: Send + Sync {
    fn tools(&self) -> Vec<Tool>;

    /// Runs the tool named `name`, one of [`tools`](Self::tools).
    fn call(&self, name: &str, call: Call) -> Called;
}

/// `tools/call`'s parameters, read from the request's own text so the arguments keep their
/// order.
#[derive(Deserialize)]
struct CallRequest {
    params: CallParams,
}

#[derive(Deserialize)]
struct CallParams {
    name: Option<String>,
    arguments: Option<Arguments>,
    #[serde(rename = "_meta")]
    meta: Option<CallMeta>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CallMeta {
    progress_token: Option<Value>,
}

/// A destination every message goes through, one line each.
type Output = Arc<Mutex<Box<dyn Write + Send>>>;

/// A call in flight: its token, and whether the client cancelled it, so it gets no response.
struct InFlight {
    cancel: Cancellation,
    cancelled: bool,
}

struct Session {
    server: Server,
    tools: Arc<dyn Tools>,
    output: Output,
    /// The index in `LEVELS` below which log messages are not sent.
    level: Arc<AtomicUsize>,
    calls: Mutex<HashMap<String, InFlight>>,
    /// Fires when the server stops; every call's token is under it.
    stopping: Cancellation,
    served: AtomicU64,
}

/// Serves `tools` on `input` and `output` until the input ends or `stop` fires. Calls in
/// flight then are cancelled and answered before it returns.
pub fn serve(
    server: &Server,
    tools: Arc<dyn Tools>,
    input: Box<dyn Read + Send>,
    output: Box<dyn Write + Send>,
    stop: &Cancellation,
) -> Served {
    let session = Arc::new(Session {
        server: server.clone(),
        tools,
        output: Arc::new(Mutex::new(output)),
        level: Arc::new(AtomicUsize::new(0)),
        calls: Mutex::new(HashMap::new()),
        stopping: stop.under(None),
        served: AtomicU64::new(0),
    });
    let lines = read_lines(input);
    let mut workers: Vec<JoinHandle<()>> = Vec::new();
    while !session.stopping.is_cancelled() {
        match lines.recv_timeout(POLL) {
            Ok(Some(line)) => workers.extend(session.handle(&line)),
            Ok(None) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        workers.retain(|worker| !worker.is_finished());
    }
    session.stopping.cancel();
    for worker in workers {
        let _ = worker.join();
    }
    Served {
        requests: session.served.load(Ordering::SeqCst),
    }
}

/// Reads `input` on a thread of its own, one line at a time; `None` once it ends, fails, or
/// holds a line longer than `MAX_MESSAGE`.
fn read_lines(input: Box<dyn Read + Send>) -> Receiver<Option<Vec<u8>>> {
    let (lines, received) = mpsc::channel();
    thread::spawn(move || {
        let mut input = BufReader::new(input);
        while let Ok(Some(line)) = read_line_within(&mut input, MAX_MESSAGE) {
            if lines.send(Some(line)).is_err() {
                return;
            }
        }
        let _ = lines.send(None);
    });
    received
}

impl Session {
    fn send(&self, message: &Value) {
        send(&self.output, message);
    }

    /// Handles one message; a call's worker when it starts one.
    fn handle(self: &Arc<Self>, line: &[u8]) -> Option<JoinHandle<()>> {
        let message = match serde_json::from_slice::<Value>(line) {
            Ok(Value::Object(message)) => message,
            Ok(_) => {
                let text = "a message is one JSON object";
                self.send(&failure(&Value::Null, INVALID_REQUEST, text));
                return None;
            }
            Err(error) => {
                self.send(&failure(&Value::Null, PARSE_ERROR, &error.to_string()));
                return None;
            }
        };
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        match (
            message.get("id"),
            message.get("method").and_then(Value::as_str),
        ) {
            (Some(id), Some("tools/call")) => self.call(id.clone(), line),
            (Some(id), Some(method)) => {
                let answer = match self.answer(method, &params) {
                    Ok(result) => success(id, &result),
                    Err((code, text)) => failure(id, code, &text),
                };
                self.send(&answer);
                None
            }
            (None, Some("notifications/cancelled")) => {
                self.cancelled(params.get("requestId"));
                None
            }
            // Other notifications, and responses to requests this server never sends.
            _ => None,
        }
    }

    /// The result of a request other than `tools/call`.
    fn answer(&self, method: &str, params: &Value) -> Result<Value, (i64, String)> {
        match method {
            "initialize" => Ok(self.initialize(params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": self.tools.tools()})),
            "logging/setLevel" => {
                let level = params.get("level").and_then(Value::as_str);
                let index = LEVELS.iter().position(|known| Some(*known) == level);
                let index = index.ok_or((INVALID_PARAMS, format!("Unknown level: {level:?}")))?;
                self.level.store(index, Ordering::SeqCst);
                Ok(json!({}))
            }
            _ => Err((METHOD_NOT_FOUND, format!("Method not found: {method}"))),
        }
    }

    fn initialize(&self, params: &Value) -> Value {
        let asked = params.get("protocolVersion").and_then(Value::as_str);
        let version = VERSIONS
            .into_iter()
            .find(|version| Some(*version) == asked)
            .unwrap_or(VERSIONS[0]);
        json!({
            "protocolVersion": version,
            "capabilities": {"tools": {}, "logging": {}},
            "serverInfo": {"name": self.server.name, "version": self.server.version},
        })
    }

    /// Starts the call `line` asks for on its own thread, or answers why it cannot.
    fn call(self: &Arc<Self>, id: Value, line: &[u8]) -> Option<JoinHandle<()>> {
        let params = match serde_json::from_slice::<CallRequest>(line) {
            Ok(request) => request.params,
            Err(error) => {
                self.send(&failure(&id, INVALID_PARAMS, &error.to_string()));
                return None;
            }
        };
        let Some(name) = params.name else {
            self.send(&failure(&id, INVALID_PARAMS, "tools/call names no tool"));
            return None;
        };
        if !self.tools.tools().iter().any(|tool| tool.name == name) {
            self.send(&failure(
                &id,
                INVALID_PARAMS,
                &format!("Unknown tool: {name}"),
            ));
            return None;
        }
        let token = params.meta.and_then(|meta| meta.progress_token);
        let cancel = self.stopping.under(None);
        let key = id.to_string();
        let in_flight = InFlight {
            cancel: cancel.clone(),
            cancelled: false,
        };
        self.calls().insert(key.clone(), in_flight);
        let call = Call {
            arguments: params.arguments.unwrap_or_default(),
            events: Box::new(Lines::new(progress(&self.output, token))),
            stderr: Box::new(Lines::new(log(&self.output, &self.level))),
            cancel,
        };
        let session = Arc::clone(self);
        Some(thread::spawn(move || {
            let called = session.tools.call(&name, call);
            session.served.fetch_add(1, Ordering::SeqCst);
            let cancelled = session
                .calls()
                .remove(&key)
                .is_some_and(|in_flight| in_flight.cancelled);
            if !cancelled {
                session.send(&success(&id, &result(&called)));
            }
        }))
    }

    /// The client cancelled request `id`: its call stops and is not answered.
    fn cancelled(&self, id: Option<&Value>) {
        let Some(id) = id else { return };
        if let Some(in_flight) = self.calls().get_mut(&id.to_string()) {
            in_flight.cancelled = true;
            in_flight.cancel.cancel();
        }
    }

    fn calls(&self) -> std::sync::MutexGuard<'_, HashMap<String, InFlight>> {
        self.calls.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// `called` as a `tools/call` result.
fn result(called: &Called) -> Value {
    let document = serde_json::from_str::<Value>(&called.document)
        .ok()
        .filter(Value::is_object);
    let mut result = json!({
        "content": [{"type": "text", "text": called.document}],
        "isError": called.is_error,
        "_meta": {COMMAND_KEY: called.command},
    });
    if let Some(document) = document {
        result["structuredContent"] = document;
    }
    result
}

/// What a call's writers hand on: lines of text, newlines included.
type LineSink = dyn FnMut(&str) -> std::io::Result<()> + Send;

/// Sends each event line as a progress notification under `token`; nothing without one.
fn progress(output: &Output, token: Option<Value>) -> Box<LineSink> {
    let output = Arc::clone(output);
    let mut sent: u64 = 0;
    Box::new(move |lines: &str| {
        let Some(token) = &token else { return Ok(()) };
        for line in lines.lines() {
            sent += 1;
            let mut params = json!({"progressToken": token, "progress": sent, "message": line});
            if let Ok(event) = serde_json::from_str::<Value>(line) {
                params["_meta"] = json!({EVENT_KEY: event});
            }
            send(&output, &notification("notifications/progress", &params));
        }
        Ok(())
    })
}

/// Sends each standard error line as a log message, unless the level asked for is above it.
fn log(output: &Output, level: &Arc<AtomicUsize>) -> Box<LineSink> {
    let (output, level) = (Arc::clone(output), Arc::clone(level));
    Box::new(move |lines: &str| {
        if level.load(Ordering::SeqCst) <= STDERR_LEVEL {
            for line in lines.lines() {
                let params =
                    json!({"level": LEVELS[STDERR_LEVEL], "logger": "cupel", "data": line});
                send(&output, &notification("notifications/message", &params));
            }
        }
        Ok(())
    })
}

fn send(output: &Output, message: &Value) {
    let mut output = output.lock().unwrap_or_else(PoisonError::into_inner);
    // A client that has gone cannot be told anything.
    let _ = write_message(&mut *output, message);
}

fn success(id: &Value, result: &Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn failure(id: &Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn notification(method: &str, params: &Value) -> Value {
    json!({"jsonrpc": "2.0", "method": method, "params": params})
}
