// ABOUTME: `tool.porcupine` (ADR-119): register and key-value histories checked by Cupel's Go runner of Porcupine, a
// ABOUTME: pinned module run with `go run`; each violation excerpted as its longest linearizable prefix and stuck operations.
use std::path::Path;
use std::time::Duration;

use cupel_core::{
    Error, History, HistoryOp, HistoryStatus, HistoryViolation, OpOutcome, Process, ProduceCtx,
    Result, output_tail,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::history::{Op, Outcome, Recorded, canonical};

/// The runner and its module, which pins Porcupine v1.3.1, written into the module directory
/// under the cache before each run.
const PROGRAM: &str = include_str!("porcupine.go");
const GO_MOD: &str =
    "module cupel.dev/porcupine\n\ngo 1.21\n\nrequire github.com/anishathalye/porcupine v1.3.1\n";
const GO_SUM: &str = "github.com/anishathalye/porcupine v1.3.1 h1:fBZ4/NGNPnIDdd6xNtrNk9/GiEQ0L4FO5+scINN+t0E=\n\
                      github.com/anishathalye/porcupine v1.3.1/go.mod h1:WM0SsFjWNl2Y4BqHr/E/ll2yY1GY1jqn+W7Z/84Zoog=\n";

/// The models the runner checks: a single register, and a key-value store checked key by key.
const MODELS: [&str; 2] = ["register", "kv"];

/// One operation as the runner reads it: its function with canonical JSON values, and the lines
/// that bound it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct RunnerOp {
    f: String,
    read: String,
    write: String,
    from: String,
    to: String,
    call: u32,
    #[serde(rename = "return")]
    ret: u32,
}

/// A part of a history checked on its own: the whole of a register history, one key of a
/// key-value one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partition {
    pub key: Option<Value>,
    ops: Vec<RunnerOp>,
    shown: Vec<HistoryOp>,
}

/// A history to check, or why it cannot be.
pub struct Checkable {
    pub name: String,
    pub recorded: std::result::Result<Recorded, String>,
}

#[derive(Serialize)]
struct Request<'a> {
    timeout_ms: u128,
    histories: Vec<Vec<&'a [RunnerOp]>>,
}

#[derive(Debug, Deserialize)]
struct Response {
    histories: Vec<Vec<Checked>>,
}

/// One partition's result as the runner prints it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Checked {
    pub result: String,
    pub longest: Vec<usize>,
    pub states: Vec<String>,
}

/// The parts of `recorded` the runner checks, or why it cannot check them.
pub fn partitions(recorded: &Recorded) -> std::result::Result<Vec<Partition>, String> {
    let model = recorded.model.as_str();
    if !MODELS.contains(&model) {
        return Err(format!(
            "tool.porcupine checks register and kv histories, not `{model}`"
        ));
    }
    let mut partitions: Vec<Partition> = Vec::new();
    for op in &recorded.ops {
        let Some(runner) = runner_op(op, recorded.last_line)? else {
            continue;
        };
        let key = if model == "kv" {
            Some(
                op.key
                    .clone()
                    .ok_or_else(|| format!("line {}: a kv operation names its key", op.call))?,
            )
        } else {
            None
        };
        let index = partitions
            .iter()
            .position(|partition| {
                partition.key.as_ref().map(canonical) == key.as_ref().map(canonical)
            })
            .unwrap_or_else(|| {
                partitions.push(Partition {
                    key: key.clone(),
                    ops: Vec::new(),
                    shown: Vec::new(),
                });
                partitions.len() - 1
            });
        partitions[index].ops.push(runner);
        partitions[index].shown.push(shown(op));
    }
    Ok(partitions)
}

/// The operations of `recorded` that may have taken effect.
pub fn operations(recorded: &Recorded) -> u64 {
    recorded
        .ops
        .iter()
        .filter(|op| op.outcome != Outcome::Fail)
        .count() as u64
}

/// `op` as the runner checks it; `None` for one that cannot constrain the register: a failed
/// operation, which did not happen, or a read of unknown outcome, which observed nothing.
fn runner_op(op: &Op, last_line: u32) -> std::result::Result<Option<RunnerOp>, String> {
    if op.outcome == Outcome::Fail || (op.f == "read" && op.unknown()) {
        return Ok(None);
    }
    let mut runner = RunnerOp {
        f: op.f.clone(),
        read: String::new(),
        write: String::new(),
        from: String::new(),
        to: String::new(),
        call: op.call,
        // An operation of unknown outcome returns after every other, so it may take effect
        // anywhere after its call, or never.
        ret: op.ret.filter(|_| !op.unknown()).unwrap_or(last_line + 1),
    };
    match op.f.as_str() {
        "read" => runner.read = canonical(op.completed.as_ref().unwrap_or(&Value::Null)),
        "write" => runner.write = canonical(&op.invoked),
        "cas" => match op.invoked.as_array().map(Vec::as_slice) {
            Some([from, to]) => {
                runner.from = canonical(from);
                runner.to = canonical(to);
            }
            _ => {
                return Err(format!(
                    "line {}: a cas operation's value is [expected, new]",
                    op.call
                ));
            }
        },
        other => {
            return Err(format!(
                "line {}: `{other}` is not a register operation (read, write, cas)",
                op.call
            ));
        }
    }
    Ok(Some(runner))
}

/// How a violation's excerpt shows `op`.
fn shown(op: &Op) -> HistoryOp {
    HistoryOp {
        process: op.process.clone(),
        f: op.f.clone(),
        key: op.key.clone(),
        value: if op.f == "read" {
            op.completed.clone().unwrap_or(Value::Null)
        } else {
            op.invoked.clone()
        },
        outcome: if op.unknown() {
            OpOutcome::Info
        } else {
            OpOutcome::Ok
        },
        line: op.call,
    }
}

/// How long the runner may spend on each part of a history at `depth`.
pub fn timeout(depth: cupel_core::CostClass) -> Duration {
    match depth {
        cupel_core::CostClass::Exhaustive => Duration::from_secs(300),
        _ => Duration::from_secs(30),
    }
}

/// Checks every history of `histories` the runner can check, running it once in `module`,
/// its module directory under the cache.
///
/// # Errors
///
/// When the runner cannot be written or run, or prints what Cupel cannot read.
pub fn check(
    ctx: &ProduceCtx,
    module: &Path,
    histories: Vec<Checkable>,
    timeout: Duration,
) -> Result<Vec<History>> {
    let parted: Vec<(Checkable, std::result::Result<Vec<Partition>, String>)> = histories
        .into_iter()
        .map(|history| {
            let parts = history
                .recorded
                .as_ref()
                .map_err(Clone::clone)
                .and_then(partitions);
            (history, parts)
        })
        .collect();
    let request = Request {
        timeout_ms: timeout.as_millis(),
        histories: parted
            .iter()
            .filter_map(|(_, parts)| parts.as_ref().ok())
            .map(|parts| parts.iter().map(|part| part.ops.as_slice()).collect())
            .collect(),
    };
    let mut checked = run(ctx, module, &request)?.into_iter();
    Ok(parted
        .into_iter()
        .map(|(history, parts)| {
            let operations = history.recorded.as_ref().map_or(0, operations);
            let results = parts
                .as_ref()
                .ok()
                .map(|_| checked.next().unwrap_or_default());
            judged(history, parts, results, operations, timeout)
        })
        .collect())
}

/// The fact's entry for one history, from its parts and the runner's result for each.
fn judged(
    history: Checkable,
    parts: std::result::Result<Vec<Partition>, String>,
    results: Option<Vec<Checked>>,
    operations: u64,
    timeout: Duration,
) -> History {
    let model = history
        .recorded
        .as_ref()
        .ok()
        .map(|recorded| recorded.model.clone());
    let mut entry = History {
        name: history.name,
        model,
        operations,
        status: HistoryStatus::Linearizable,
        detail: None,
        violations: Vec::new(),
    };
    let parts = match parts {
        Ok(parts) => parts,
        Err(problem) => {
            entry.status = HistoryStatus::Invalid;
            entry.detail = Some(problem);
            return entry;
        }
    };
    let results = results.unwrap_or_default();
    for (part, result) in parts.iter().zip(&results) {
        match result.result.as_str() {
            "Illegal" => entry.violations.push(violation(part, result)),
            "Ok" => {}
            // `Unknown`, or no result: the runner ran out of time on the part.
            _ => {
                entry.detail = Some(format!(
                    "the checker ran out of its {} s",
                    timeout.as_secs()
                ));
            }
        }
    }
    if !entry.violations.is_empty() {
        entry.status = HistoryStatus::Violated;
        entry.detail = None;
    } else if entry.detail.is_some() {
        entry.status = HistoryStatus::Unknown;
    }
    entry
}

/// The excerpt of a part no linearization explains: the last three operations of the longest
/// linearizable prefix, and the operations that could come next in real time.
fn violation(part: &Partition, checked: &Checked) -> HistoryViolation {
    let longest = &checked.longest;
    let remaining: Vec<usize> = (0..part.ops.len())
        .filter(|index| !longest.contains(index))
        .collect();
    let first_return = remaining.iter().map(|&index| part.ops[index].ret).min();
    let stuck = remaining
        .into_iter()
        .filter(|&index| Some(part.ops[index].call) <= first_return)
        .map(|index| part.shown[index].clone())
        .collect();
    let tail = longest.len().saturating_sub(3);
    HistoryViolation {
        key: part.key.clone(),
        anomaly: "non-linearizable".to_owned(),
        states: checked
            .states
            .iter()
            .map(|state| serde_json::from_str(state).unwrap_or(Value::Null))
            .collect(),
        linearized: longest[tail..]
            .iter()
            .filter_map(|&index| part.shown.get(index).cloned())
            .collect(),
        stuck,
    }
}

/// Writes the runner's module into `module` and runs it on `request`; its result per history.
/// The runner's input and output stay there as `request.json` and `response.json`.
fn run(ctx: &ProduceCtx, module: &Path, request: &Request<'_>) -> Result<Vec<Vec<Checked>>> {
    if request.histories.is_empty() {
        return Ok(Vec::new());
    }
    for (file, text) in [("go.mod", GO_MOD), ("go.sum", GO_SUM), ("main.go", PROGRAM)] {
        ctx.writer.write(&module.join(file), text.as_bytes())?;
    }
    let input = serde_json::to_vec(request).map_err(|error| failed(&error.to_string()))?;
    ctx.writer.write(&module.join("request.json"), &input)?;
    let captured = Process::new("go", module)
        .args(["run", "."])
        .env("GOTOOLCHAIN", "local")
        .env("GOFLAGS", "-mod=readonly")
        .input(input)
        .run(&ctx.cancel)?;
    if captured.exit_code != Some(0) {
        let output = format!("{}\n{}", captured.stdout, captured.stderr);
        return Err(failed(
            &output_tail(&output).unwrap_or_else(|| format!("exited {:?}", captured.exit_code)),
        ));
    }
    ctx.writer
        .write(&module.join("response.json"), captured.stdout.as_bytes())?;
    let response: Response = serde_json::from_str(&captured.stdout)
        .map_err(|error| failed(&format!("unreadable output: {error}")))?;
    Ok(response.histories)
}

fn failed(message: &str) -> Error {
    Error::Process {
        program: "go".to_owned(),
        message: format!("the Porcupine runner failed: {message}"),
    }
}

#[cfg(test)]
mod tests {
    use cupel_fixtures::Capture;
    use serde_json::json;

    use super::super::history::read;
    use super::super::history::written::{event as e, text};
    use super::*;

    fn recorded(model: &str, events: &[Value]) -> Recorded {
        read(&text(model, events)).unwrap()
    }

    fn stale_read() -> Recorded {
        recorded(
            "register",
            &[
                e(0, "invoke", "write", &json!(1)),
                e(0, "ok", "write", &json!(1)),
                e(0, "invoke", "write", &json!(2)),
                e(0, "ok", "write", &json!(2)),
                e(1, "invoke", "read", &Value::Null),
                e(1, "ok", "read", &json!(1)),
            ],
        )
    }

    #[test]
    fn a_register_history_is_one_part_of_canonical_runner_operations() {
        let parts = partitions(&stale_read()).unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].key, None);
        let ops: Vec<(&str, &str, &str, u32, u32)> = parts[0]
            .ops
            .iter()
            .map(|op| {
                (
                    op.f.as_str(),
                    op.read.as_str(),
                    op.write.as_str(),
                    op.call,
                    op.ret,
                )
            })
            .collect();
        assert_eq!(
            ops,
            [
                ("write", "", "1", 2, 3),
                ("write", "", "2", 4, 5),
                ("read", "1", "", 6, 7)
            ]
        );
    }

    #[test]
    fn failed_operations_and_reads_of_unknown_outcome_are_not_checked() {
        let history = recorded(
            "register",
            &[
                e(0, "invoke", "write", &json!(1)),
                e(0, "fail", "write", &json!(1)),
                e(1, "invoke", "read", &Value::Null),
                e(1, "info", "read", &Value::Null),
                e(2, "invoke", "cas", &json!([null, {"b": 1, "a": 2}])),
            ],
        );
        let parts = partitions(&history).unwrap();
        assert_eq!(operations(&history), 2);
        assert_eq!(parts[0].ops.len(), 1);
        let cas = &parts[0].ops[0];
        assert_eq!(
            (cas.from.as_str(), cas.to.as_str(), cas.ret),
            ("null", r#"{"a":2,"b":1}"#, 7)
        );
        assert_eq!(parts[0].shown[0].outcome, OpOutcome::Info);
    }

    #[test]
    fn a_key_value_history_is_parted_by_key_in_the_order_keys_first_appear() {
        let keyed = |process, kind, f, key: &str, value| json!({"process": process, "type": kind, "f": f, "key": key, "value": value});
        let history = recorded(
            "kv",
            &[
                keyed(0, "invoke", "write", "y", json!(1)),
                keyed(0, "ok", "write", "y", json!(1)),
                keyed(1, "invoke", "read", "x", Value::Null),
                keyed(1, "ok", "read", "x", Value::Null),
                keyed(0, "invoke", "read", "y", Value::Null),
                keyed(0, "ok", "read", "y", json!(1)),
            ],
        );
        let parts = partitions(&history).unwrap();
        let keys: Vec<(Option<Value>, usize)> = parts
            .iter()
            .map(|part| (part.key.clone(), part.ops.len()))
            .collect();
        assert_eq!(keys, [(Some(json!("y")), 2), (Some(json!("x")), 1)]);
    }

    #[test]
    fn histories_the_runner_cannot_check_say_why() {
        let problem =
            |model: &str, events: &[Value]| partitions(&recorded(model, events)).unwrap_err();
        assert_eq!(
            problem("queue", &[]),
            "tool.porcupine checks register and kv histories, not `queue`"
        );
        assert_eq!(
            problem("register", &[e(0, "invoke", "enqueue", &json!(1))]),
            "line 2: `enqueue` is not a register operation (read, write, cas)"
        );
        assert_eq!(
            problem("register", &[e(0, "invoke", "cas", &json!(1))]),
            "line 2: a cas operation's value is [expected, new]"
        );
        assert_eq!(
            problem("kv", &[e(0, "invoke", "write", &json!(1))]),
            "line 2: a kv operation names its key"
        );
    }

    #[test]
    fn the_runner_s_output_becomes_each_history_s_verdict_and_excerpt() {
        let output = Capture::of("porcupine", "conformance").file("output.json");
        let response: Response = serde_json::from_str(&output).unwrap();
        // The sixth checkable history, by name, is the stale read.
        let stale = response.histories[5][0].clone();
        assert_eq!(stale.result, "Illegal");
        let parts = partitions(&stale_read()).unwrap();
        let checkable = Checkable {
            name: "stale-read.jsonl".to_owned(),
            recorded: Ok(stale_read()),
        };
        let history = judged(
            checkable,
            Ok(parts),
            Some(vec![stale]),
            3,
            Duration::from_secs(30),
        );
        assert_eq!(history.status, HistoryStatus::Violated);
        let excerpt = serde_json::to_value(&history.violations).unwrap();
        assert_eq!(
            excerpt,
            json!([{
                "key": null,
                "anomaly": "non-linearizable",
                "states": [2],
                "linearized": [
                    {"process": 0, "f": "write", "key": null, "value": 1, "outcome": "ok", "line": 2},
                    {"process": 0, "f": "write", "key": null, "value": 2, "outcome": "ok", "line": 4},
                ],
                "stuck": [
                    {"process": 1, "f": "read", "key": null, "value": 1, "outcome": "ok", "line": 6},
                ],
            }])
        );
    }

    #[test]
    fn a_part_the_runner_ran_out_of_time_on_leaves_the_history_unknown() {
        let checkable = Checkable {
            name: "slow.jsonl".to_owned(),
            recorded: Ok(stale_read()),
        };
        let parts = partitions(&stale_read()).unwrap();
        let history = judged(
            checkable,
            Ok(parts),
            Some(vec![Checked::default()]),
            3,
            Duration::from_secs(30),
        );
        assert_eq!(history.status, HistoryStatus::Unknown);
        assert_eq!(
            history.detail.as_deref(),
            Some("the checker ran out of its 30 s")
        );
        let invalid = judged(
            Checkable {
                name: "bad.jsonl".to_owned(),
                recorded: Err("line 1: no model".to_owned()),
            },
            Err("line 1: no model".to_owned()),
            None,
            0,
            Duration::from_secs(30),
        );
        assert_eq!(
            (invalid.status, invalid.model, invalid.detail.as_deref()),
            (HistoryStatus::Invalid, None, Some("line 1: no model"))
        );
    }
}
