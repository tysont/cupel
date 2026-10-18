// ABOUTME: Reads a recorded history (ADR-119): a JSON-lines file whose first line declares the model and whose other
// ABOUTME: lines are invocations and completions per process, paired into operations with the lines that bound them.
use serde_json::{Map, Value};

/// A history file, read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    /// The model the first line declares.
    pub model: String,
    /// Every invocation, in the order invoked.
    pub ops: Vec<Op>,
    /// The last line of the file, after which an operation of unknown outcome returns.
    pub last_line: u32,
}

/// One invocation and how it completed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Op {
    pub process: Value,
    pub f: String,
    pub key: Option<Value>,
    /// The value it was invoked with.
    pub invoked: Value,
    /// The value it completed with, for an `ok` completion.
    pub completed: Option<Value>,
    pub outcome: Outcome,
    /// The line that invoked it.
    pub call: u32,
    /// The line that completed it; `None` when its outcome is unknown.
    pub ret: Option<u32>,
}

/// How an invocation completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// It took effect (`ok`).
    Ok,
    /// It did not take effect (`fail`).
    Fail,
    /// It may or may not have (`info`, or no completion before the history ends).
    Info,
}

/// Reads the history `text`; why it cannot be read, with its line, otherwise.
pub fn read(text: &str) -> Result<Recorded, String> {
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(index, line)| (u32::try_from(index + 1).unwrap_or(u32::MAX), line))
        .filter(|(_, line)| !line.trim().is_empty());
    let model = lines
        .next()
        .and_then(|(_, line)| serde_json::from_str::<Value>(line).ok())
        .and_then(|header| header.get("model")?.as_str().map(str::to_owned))
        .ok_or("line 1: the first line declares the model, as {\"model\": \"register\"}")?;
    let mut recorded = Recorded {
        model,
        ops: Vec::new(),
        last_line: 1,
    };
    for (line, text) in lines {
        recorded.last_line = line;
        let event = event(text).map_err(|problem| format!("line {line}: {problem}"))?;
        recorded
            .take(event, line)
            .map_err(|problem| format!("line {line}: {problem}"))?;
    }
    Ok(recorded)
}

/// One line of a history.
struct Event {
    process: Value,
    kind: String,
    f: String,
    key: Option<Value>,
    value: Value,
}

fn event(text: &str) -> Result<Event, String> {
    let Ok(Value::Object(mut fields)) = serde_json::from_str::<Value>(text) else {
        return Err("not a JSON object".to_owned());
    };
    let mut string = |name: &str| match fields.remove(name) {
        Some(Value::String(text)) => Ok(text),
        _ => Err(format!("`{name}` is a string")),
    };
    let kind = string("type")?;
    let f = string("f")?;
    let process = match fields.remove("process") {
        Some(Value::Null) | None => return Err("`process` names the process".to_owned()),
        Some(process) => process,
    };
    if !["invoke", "ok", "fail", "info"].contains(&kind.as_str()) {
        return Err(format!("`type` is invoke, ok, fail, or info, not `{kind}`"));
    }
    Ok(Event {
        process,
        kind,
        f,
        key: fields.remove("key").filter(|key| !key.is_null()),
        value: fields.remove("value").unwrap_or(Value::Null),
    })
}

impl Recorded {
    /// Adds the event on `line`: an invocation opens an operation of its process, a completion
    /// closes the process's open one.
    fn take(&mut self, event: Event, line: u32) -> Result<(), String> {
        let process = canonical(&event.process);
        let open = self
            .ops
            .iter_mut()
            .rev()
            .find(|op| canonical(&op.process) == process && op.ret.is_none());
        if event.kind == "invoke" {
            if let Some(open) = open {
                return Err(format!(
                    "process {process} invokes `{}` before its `{}` of line {} completes",
                    event.f, open.f, open.call
                ));
            }
            self.ops.push(Op {
                process: event.process,
                f: event.f,
                key: event.key,
                invoked: event.value,
                completed: None,
                outcome: Outcome::Info,
                call: line,
                ret: None,
            });
            return Ok(());
        }
        let Some(open) = open else {
            return Err(format!(
                "process {process} completes `{}` with no open invocation",
                event.f
            ));
        };
        if open.f != event.f {
            return Err(format!(
                "process {process} completes `{}`, but invoked `{}` on line {}",
                event.f, open.f, open.call
            ));
        }
        open.key = open.key.take().or(event.key);
        match event.kind.as_str() {
            "ok" => {
                open.outcome = Outcome::Ok;
                open.completed = Some(event.value);
                open.ret = Some(line);
            }
            "fail" => {
                open.outcome = Outcome::Fail;
                open.ret = Some(line);
            }
            // An `info` completion leaves the outcome unknown; the process may not invoke again.
            _ => open.ret = Some(line),
        }
        Ok(())
    }
}

impl Op {
    /// Whether its outcome is unknown: `info`, or never completed.
    pub fn unknown(&self) -> bool {
        self.outcome == Outcome::Info
    }
}

/// `value` as JSON text with every object's keys sorted, so equal values read the same.
pub fn canonical(value: &Value) -> String {
    serde_json::to_string(&sorted(value)).unwrap_or_default()
}

fn sorted(value: &Value) -> Value {
    match value {
        Value::Object(fields) => {
            let mut keys: Vec<&String> = fields.keys().collect();
            keys.sort();
            Value::Object(
                keys.into_iter()
                    .map(|key| (key.clone(), sorted(&fields[key])))
                    .collect::<Map<String, Value>>(),
            )
        }
        Value::Array(items) => Value::Array(items.iter().map(sorted).collect()),
        other => other.clone(),
    }
}

/// Histories written the way a history command writes them, for tests.
#[cfg(test)]
pub mod written {
    use serde_json::{Value, json};

    /// The text of a history of `model` holding `events`, one JSON line each.
    pub fn text(model: &str, events: &[Value]) -> String {
        let mut text = json!({ "model": model }).to_string();
        for event in events {
            text.push('\n');
            text.push_str(&event.to_string());
        }
        text.push('\n');
        text
    }

    /// The event `kind` of `f` by `process`, with `value`.
    pub fn event(process: u64, kind: &str, f: &str, value: &Value) -> Value {
        json!({"process": process, "type": kind, "f": f, "value": value})
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    use super::written::event as e;

    fn lines(events: &[Value]) -> String {
        super::written::text("register", events)
    }

    /// One recorded operation as (function, completion value, outcome, call line, return line).
    type Paired = (String, Option<Value>, Outcome, u32, Option<u32>);

    #[test]
    fn invocations_pair_with_their_process_s_completion() {
        let recorded = read(&lines(&[
            e(0, "invoke", "write", &json!(1)),
            e(1, "invoke", "read", &Value::Null),
            e(0, "ok", "write", &json!(1)),
            e(1, "ok", "read", &json!(1)),
            e(0, "invoke", "write", &json!(2)),
            e(0, "fail", "write", &json!(2)),
        ]))
        .unwrap();
        assert_eq!(recorded.model, "register");
        assert_eq!(recorded.last_line, 7);
        let ops: Vec<Paired> = recorded
            .ops
            .iter()
            .map(|op| {
                (
                    op.f.clone(),
                    op.completed.clone(),
                    op.outcome,
                    op.call,
                    op.ret,
                )
            })
            .collect();
        assert_eq!(
            ops,
            [
                ("write".to_owned(), Some(json!(1)), Outcome::Ok, 2, Some(4)),
                ("read".to_owned(), Some(json!(1)), Outcome::Ok, 3, Some(5)),
                ("write".to_owned(), None, Outcome::Fail, 6, Some(7)),
            ]
        );
    }

    #[test]
    fn an_info_completion_and_a_missing_one_leave_the_outcome_unknown() {
        let recorded = read(&lines(&[
            e(0, "invoke", "write", &json!(1)),
            e(0, "info", "write", &Value::Null),
            e(1, "invoke", "read", &Value::Null),
        ]))
        .unwrap();
        assert!(recorded.ops.iter().all(Op::unknown));
        assert_eq!(recorded.ops[0].ret, Some(3));
        assert_eq!(recorded.ops[1].ret, None);
    }

    #[test]
    fn a_history_that_cannot_be_paired_names_its_line() {
        let read_error = |events: &[Value]| read(&lines(events)).unwrap_err();
        assert_eq!(
            read_error(&[e(1, "ok", "read", &json!(1))]),
            "line 2: process 1 completes `read` with no open invocation"
        );
        assert_eq!(
            read_error(&[
                e(0, "invoke", "write", &json!(1)),
                e(0, "invoke", "read", &Value::Null)
            ]),
            "line 3: process 0 invokes `read` before its `write` of line 2 completes"
        );
        assert_eq!(
            read_error(&[
                e(0, "invoke", "write", &json!(1)),
                e(0, "ok", "read", &json!(1))
            ]),
            "line 3: process 0 completes `read`, but invoked `write` on line 2"
        );
        assert_eq!(
            read_error(&[json!({"process": 0, "type": "done", "f": "read"})]),
            "line 2: `type` is invoke, ok, fail, or info, not `done`"
        );
        assert_eq!(
            read_error(&[json!({"type": "invoke", "f": "read"})]),
            "line 2: `process` names the process"
        );
        assert_eq!(
            read("{\"process\": 0}\n").unwrap_err(),
            "line 1: the first line declares the model, as {\"model\": \"register\"}"
        );
        assert!(read("").is_err());
    }

    #[test]
    fn canonical_text_sorts_object_keys_at_every_depth() {
        assert_eq!(
            canonical(&json!({"b": [{"d": 1, "c": 2}], "a": null})),
            r#"{"a":null,"b":[{"c":2,"d":1}]}"#
        );
    }
}
