// ABOUTME: Tests for the MCP server over in-memory pipes with scripted tools: the handshake, listing and calling
// ABOUTME: tools, progress and log notifications, protocol errors, cancellation, end of input, and the stop token.
use std::io::{BufRead, BufReader, PipeReader, PipeWriter, Write};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cupel_core::Cancellation;
use cupel_serve::Served;
use cupel_serve::mcp::{self, Call, Called, Server, Tool, Tools};
use serde_json::{Value, json};

/// Tools scripted by name: `echo` returns its arguments, in order, as its document; `events`
/// writes two event lines and a stderr line first; `wait` waits on its token; `broken` fails.
#[derive(Default)]
struct Script {
    /// Each `wait` call's outcome, once it ends.
    waited: Mutex<Vec<&'static str>>,
}

impl Tools for Script {
    fn tools(&self) -> Vec<Tool> {
        ["echo", "events", "wait", "broken"]
            .into_iter()
            .map(|name| Tool {
                name: name.to_owned(),
                title: format!("The {name} tool"),
                description: format!("Runs {name}."),
                input_schema: json!({"type": "object"}),
            })
            .collect()
    }

    fn call(&self, name: &str, mut call: Call) -> Called {
        let pairs: Vec<Value> = call
            .arguments
            .0
            .iter()
            .map(|(key, value)| json!([key, value]))
            .collect();
        let document = match name {
            "events" => {
                call.events
                    .write_all(b"{\"ev\":\"plan\",\"checks\":1}\n{\"ev\":\"run.end\"}\n")
                    .unwrap();
                call.stderr.write_all(b"cupel: a note\n").unwrap();
                json!({"status": "pass"})
            }
            "wait" => {
                let outcome = if call.cancel.wait(Duration::from_secs(30)) {
                    "finished"
                } else {
                    "cancelled"
                };
                self.waited.lock().unwrap().push(outcome);
                json!({"status": outcome})
            }
            _ => json!({"status": "pass", "arguments": pairs}),
        };
        Called {
            command: vec![name.to_owned()],
            document: if name == "broken" {
                "not json".to_owned()
            } else {
                serde_json::to_string_pretty(&document).unwrap()
            },
            is_error: name == "broken",
        }
    }
}

/// A server on a thread, fed through one pipe and read through another.
struct Session {
    input: Option<PipeWriter>,
    output: BufReader<PipeReader>,
    stop: Cancellation,
    server: Option<JoinHandle<Served>>,
    tools: Arc<Script>,
}

impl Session {
    fn start() -> Self {
        let (input_reader, input) = std::io::pipe().unwrap();
        let (output, output_writer) = std::io::pipe().unwrap();
        let stop = Cancellation::new();
        let tools = Arc::new(Script::default());
        let (served_tools, served_stop) = (Arc::clone(&tools), stop.clone());
        let server = thread::spawn(move || {
            let info = Server {
                name: "cupel".to_owned(),
                version: "0.1.0".to_owned(),
            };
            mcp::serve(
                &info,
                served_tools,
                Box::new(input_reader),
                Box::new(output_writer),
                &served_stop,
            )
        });
        Self {
            input: Some(input),
            output: BufReader::new(output),
            stop,
            server: Some(server),
            tools,
        }
    }

    fn send(&mut self, message: &Value) {
        self.send_line(&message.to_string());
    }

    fn send_line(&mut self, line: &str) {
        let input = self.input.as_mut().expect("input is open");
        writeln!(input, "{line}").unwrap();
        input.flush().unwrap();
    }

    /// The next message the server wrote.
    fn next(&mut self) -> Value {
        let mut line = String::new();
        self.output.read_line(&mut line).unwrap();
        assert!(line.ends_with('\n'), "a whole line: {line:?}");
        serde_json::from_str(&line).unwrap_or_else(|_| panic!("a JSON line: {line:?}"))
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

    /// The response to request `id`.
    fn response(&mut self, id: u64, method: &str, params: &Value) -> Value {
        self.ask(id, method, params).pop().unwrap()
    }

    /// Closes the input and returns what the server reported.
    fn close(mut self) -> Served {
        drop(self.input.take());
        self.server.take().unwrap().join().unwrap()
    }
}

fn call(name: &str, arguments: &Value) -> Value {
    json!({"name": name, "arguments": arguments})
}

#[test]
fn initialize_answers_the_version_asked_for_when_it_is_one_it_speaks() {
    let mut session = Session::start();
    for (id, asked, answered) in [
        (1, "2025-11-25", "2025-11-25"),
        (2, "2025-06-18", "2025-06-18"),
        (3, "2024-11-05", "2025-11-25"),
    ] {
        let params = json!({"protocolVersion": asked, "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}});
        let response = session.response(id, "initialize", &params);
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["result"]["protocolVersion"], answered);
        assert_eq!(
            response["result"]["serverInfo"],
            json!({"name": "cupel", "version": "0.1.0"})
        );
        assert_eq!(
            response["result"]["capabilities"],
            json!({"tools": {}, "logging": {}})
        );
    }
    assert_eq!(mcp::VERSIONS, ["2025-11-25", "2025-06-18"]);
}

#[test]
fn ping_and_set_level_answer_an_empty_result() {
    let mut session = Session::start();
    session.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    assert_eq!(session.response(1, "ping", &json!({}))["result"], json!({}));
    let level = session.response(2, "logging/setLevel", &json!({"level": "error"}));
    assert_eq!(level["result"], json!({}));
}

#[test]
fn tools_list_returns_every_tool_with_its_schema() {
    let mut session = Session::start();
    let response = session.response(1, "tools/list", &json!({}));
    let tools = response["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 4);
    assert_eq!(
        tools[0],
        json!({"name": "echo", "title": "The echo tool", "description": "Runs echo.", "inputSchema": {"type": "object"}})
    );
}

#[test]
fn a_call_returns_the_document_as_structured_and_text_content_with_its_command() {
    let mut session = Session::start();
    // Arguments reach the tool in the order the client wrote them.
    session.send_line(
        r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"echo","arguments":{"only":["b","a"],"changed":true,"depth":"quick"}}}"#,
    );
    let response = session.next();
    assert_eq!(response["id"], 7);
    let result = &response["result"];
    let document = json!({"status": "pass", "arguments": [["only", ["b", "a"]], ["changed", true], ["depth", "quick"]]});
    assert_eq!(result["structuredContent"], document);
    assert_eq!(result["isError"], false);
    assert_eq!(result["_meta"], json!({"cupel.dev/command": ["echo"]}));
    let text = result["content"][0]["text"].as_str().unwrap();
    assert_eq!(result["content"][0]["type"], "text");
    assert_eq!(serde_json::from_str::<Value>(text).unwrap(), document);
}

#[test]
fn a_call_without_arguments_passes_none() {
    let mut session = Session::start();
    let response = session.response(1, "tools/call", &json!({"name": "echo"}));
    assert_eq!(
        response["result"]["structuredContent"]["arguments"],
        json!([])
    );
}

#[test]
fn a_document_that_is_not_json_is_text_content_only() {
    let mut session = Session::start();
    let response = session.response(1, "tools/call", &call("broken", &json!({})));
    let result = &response["result"];
    assert_eq!(result["isError"], true);
    assert_eq!(
        result["content"],
        json!([{"type": "text", "text": "not json"}])
    );
    assert!(result.get("structuredContent").is_none());
}

#[test]
fn events_become_progress_notifications_under_the_requests_token_and_stderr_log_messages() {
    let mut session = Session::start();
    let params = json!({"name": "events", "arguments": {}, "_meta": {"progressToken": "tok"}});
    let messages = session.ask(1, "tools/call", &params);
    assert_eq!(messages.len(), 4, "{messages:#?}");
    let progress: Vec<&Value> = messages
        .iter()
        .filter(|m| m["method"] == "notifications/progress")
        .collect();
    assert_eq!(
        progress[0]["params"],
        json!({
            "progressToken": "tok", "progress": 1,
            "message": "{\"ev\":\"plan\",\"checks\":1}",
            "_meta": {"cupel.dev/event": {"ev": "plan", "checks": 1}}
        })
    );
    assert_eq!(progress[1]["params"]["progress"], 2);
    assert_eq!(progress[1]["params"]["message"], "{\"ev\":\"run.end\"}");
    let log: Vec<&Value> = messages
        .iter()
        .filter(|m| m["method"] == "notifications/message")
        .collect();
    assert_eq!(
        log[0]["params"],
        json!({"level": "info", "logger": "cupel", "data": "cupel: a note"})
    );
    assert_eq!(messages[3]["result"]["structuredContent"]["status"], "pass");
}

#[test]
fn without_a_token_events_send_no_progress_and_a_level_above_info_silences_stderr() {
    let mut session = Session::start();
    session.response(1, "logging/setLevel", &json!({"level": "warning"}));
    let messages = session.ask(2, "tools/call", &call("events", &json!({})));
    assert_eq!(messages.len(), 1, "{messages:#?}");
}

#[test]
fn protocol_errors_carry_json_rpc_codes() {
    let mut session = Session::start();
    let unknown_tool = session.response(1, "tools/call", &call("nope", &json!({})));
    assert_eq!(unknown_tool["error"]["code"], -32602);
    assert_eq!(unknown_tool["error"]["message"], "Unknown tool: nope");
    let not_an_object = session.response(2, "tools/call", &call("echo", &json!([1])));
    assert_eq!(not_an_object["error"]["code"], -32602);
    let no_name = session.response(3, "tools/call", &json!({}));
    assert_eq!(no_name["error"]["code"], -32602);
    // A dual-era client's probe gets "method not found" and falls back to `initialize`.
    let discover = session.response(4, "server/discover", &json!({}));
    assert_eq!(discover["error"]["code"], -32601);
    session.send_line("{not json");
    let parse = session.next();
    assert_eq!(parse["error"]["code"], -32700);
    assert_eq!(parse["id"], Value::Null);
    session.send_line("[1, 2]");
    assert_eq!(session.next()["error"]["code"], -32600);
    // A response from the client and an unknown notification are ignored.
    session.send(&json!({"jsonrpc": "2.0", "id": 9, "result": {}}));
    session.send(&json!({"jsonrpc": "2.0", "method": "notifications/whatever"}));
    assert_eq!(session.response(5, "ping", &json!({}))["result"], json!({}));
}

#[test]
fn a_cancelled_call_stops_and_gets_no_response_while_others_are_answered() {
    let mut session = Session::start();
    session.send(&json!({"jsonrpc": "2.0", "id": "w", "method": "tools/call", "params": call("wait", &json!({}))}));
    assert_eq!(session.response(1, "ping", &json!({}))["result"], json!({}));
    session.send(&json!({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": "w", "reason": "user"}}));
    // The next message is the ping's answer, not the cancelled call's.
    assert_eq!(session.response(2, "ping", &json!({}))["id"], 2);
    let tools = Arc::clone(&session.tools);
    let served = session.close();
    assert_eq!(*tools.waited.lock().unwrap(), ["cancelled"]);
    assert_eq!(served.requests, 1);
}

#[test]
fn closing_the_input_cancels_calls_in_flight_and_returns() {
    let mut session = Session::start();
    session.send(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": call("wait", &json!({}))}));
    // Answered requests are in order, so the call has started once the ping is answered.
    session.response(2, "ping", &json!({}));
    let tools = Arc::clone(&session.tools);
    let served = session.close();
    assert_eq!(*tools.waited.lock().unwrap(), ["cancelled"]);
    assert_eq!(served.requests, 1);
}

#[test]
fn the_stop_token_cancels_calls_answers_them_and_returns() {
    let mut session = Session::start();
    session.send(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": call("wait", &json!({}))}));
    session.response(2, "ping", &json!({}));
    session.stop.cancel();
    let answered = session.next();
    assert_eq!(answered["id"], 1);
    assert_eq!(
        answered["result"]["structuredContent"]["status"],
        "cancelled"
    );
    let served = session.server.take().unwrap().join().unwrap();
    assert_eq!(served.requests, 1);
}
