// ABOUTME: The client side of `serve --mcp` for programs that drive the server themselves (the parity harness,
// ABOUTME: tests): requests written one line each, replies read on a thread of their own, each wait bounded.
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use serde_json::Value;

/// A connection to an MCP server's input and output.
pub struct Client {
    input: Option<Box<dyn Write + Send>>,
    lines: Receiver<String>,
    /// How long one message may take to arrive.
    patience: Duration,
    received: Vec<String>,
}

impl Client {
    /// Speaks to a server reading `input` and writing `output`, waiting up to `patience` for
    /// each message.
    pub fn new(
        input: impl Write + Send + 'static,
        output: impl Read + Send + 'static,
        patience: Duration,
    ) -> Self {
        let (sender, lines) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(output).lines().map_while(Result::ok) {
                if sender.send(line).is_err() {
                    return;
                }
            }
        });
        Self {
            input: Some(Box::new(input)),
            lines,
            patience,
            received: Vec::new(),
        }
    }

    /// Writes `line`, a message, and a newline.
    ///
    /// # Panics
    ///
    /// When the input is closed or cannot be written.
    pub fn send(&mut self, line: &str) {
        let input = self.input.as_mut().expect("the input is open");
        writeln!(input, "{line}").expect("the server reads its input");
        input.flush().expect("the server reads its input");
    }

    /// The next message, as JSON.
    ///
    /// # Panics
    ///
    /// When none arrives in time, or it is not JSON.
    pub fn receive(&mut self) -> Value {
        let line = self
            .lines
            .recv_timeout(self.patience)
            .expect("the MCP server answers in time");
        let message = serde_json::from_str(&line)
            .unwrap_or_else(|_| panic!("the MCP server wrote a line that is not JSON: {line}"));
        self.received.push(line);
        message
    }

    /// Sends request `id` for `method` with `params` (JSON text, so a caller can order an
    /// object's keys) and returns every message up to and including its response.
    pub fn ask(&mut self, id: &Value, method: &str, params: &str) -> Vec<Value> {
        let method = Value::from(method);
        self.send(&format!(
            r#"{{"jsonrpc":"2.0","id":{id},"method":{method},"params":{params}}}"#
        ));
        let mut messages = Vec::new();
        loop {
            let message = self.receive();
            let done = message["id"] == *id && message.get("method").is_none();
            messages.push(message);
            if done {
                return messages;
            }
        }
    }

    /// Every line read so far, in order.
    pub fn received(&self) -> &[String] {
        &self.received
    }

    /// The lines that arrived but were not read yet.
    pub fn unread(&self) -> Vec<String> {
        self.lines.try_iter().collect()
    }

    /// Closes the server's input, which ends it.
    pub fn close(&mut self) {
        drop(self.input.take());
    }
}
