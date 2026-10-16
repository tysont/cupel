// ABOUTME: The Rust reference check plugin (ADR-090): reports each file in scope that no test in the per-test map
// ABOUTME: executes, as `ref.untested-file`; speaks the plugin protocol (design §8.6) with `serde_json` alone.
use std::collections::BTreeSet;
use std::io::Read;
use std::process::ExitCode;

use serde_json::{Value, json};

const CHECK: &str = "ref.untested-rust";
const KIND: &str = "ref.untested-file";

fn manifest() -> Value {
    json!({
        "kind": "check",
        "id": CHECK,
        "version": 1,
        "tier": 2,
        "cost": "quick",
        "needs": ["per-test-map"],
        "wants": [],
        "findings": [{
            "kind": KIND,
            "action": "write_test",
            "summary": "A file in scope that no test executes.",
            "evidence": {
                "type": "object",
                "properties": {"file": {"type": "string"}},
                "required": ["file"],
                "additionalProperties": false
            }
        }],
        "metrics": [{"name": "untested", "summary": "Files in scope no test executes.", "per": "scope"}],
        "stability": "experimental",
        "summary": "Files in scope that no test executes, from the per-test map."
    })
}

/// `fnd_` and six hex digits of 32-bit FNV-1a over the check, kind, and file joined by NUL.
fn finding_id(file: &str) -> String {
    let digest = format!("{CHECK}\0{KIND}\0{file}")
        .bytes()
        .fold(0x811c_9dc5_u32, |digest, byte| {
            (digest ^ u32::from(byte)).wrapping_mul(0x0100_0193)
        });
    format!("fnd_{digest:08x}")[..10].to_owned()
}

fn finding(file: &str) -> Value {
    let id = finding_id(file);
    json!({
        "ev": "finding",
        "id": id,
        "check": CHECK,
        "kind": KIND,
        "severity": "warn",
        "confidence": 1.0,
        "tier": 2,
        "symbol": null,
        "location": {"file": file, "line": 1, "col": null, "end_line": 1, "end_col": null},
        "title": format!("No test executes {file}"),
        "detail": null,
        "evidence": {"file": file},
        "action": {
            "kind": "write_test",
            "priority": 1,
            "summary": format!("Write a test that executes {file}."),
            "hints": {}
        },
        "repro": format!("cupel check --only {CHECK}"),
        "suppress": format!("cupel suppress {id} --reason '…'"),
        "new_since_baseline": false,
        "suppressed": false
    })
}

fn files(request: &Value) -> Vec<String> {
    request["scope"]["files"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn run(request: &Value) -> Result<(), String> {
    let path = request["facts"]["per-test-map"]
        .as_str()
        .ok_or("the request names no per-test map")?;
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let per_test: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let covered: BTreeSet<&str> = per_test["tests"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|test| test["covered"].as_array().into_iter().flatten())
        .filter(|lines| {
            lines["lines"]
                .as_array()
                .is_some_and(|lines| !lines.is_empty())
        })
        .filter_map(|lines| lines["file"].as_str())
        .collect();
    let files = files(request);
    let untested: Vec<&String> = files
        .iter()
        .filter(|file| !covered.contains(file.as_str()))
        .collect();
    for file in &untested {
        emit(&finding(file));
    }
    let tested = files.len() - untested.len();
    #[allow(clippy::cast_precision_loss, reason = "file counts are small")]
    let value = if files.is_empty() {
        1.0
    } else {
        tested as f64 / files.len() as f64
    };
    emit(&json!({
        "ev": "result",
        "status": if untested.is_empty() { "pass" } else { "fail" },
        "score": {"value": value, "unit": "tested_files", "n": files.len()},
        "metrics": {"untested": untested.len()},
        "notes": []
    }));
    Ok(())
}

fn emit(message: &Value) {
    println!("{message}");
}

fn request() -> Result<Value, String> {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_default();
    let done = match command.as_str() {
        "describe" => {
            emit(&manifest());
            Ok(())
        }
        "plan" => request().map(|request| {
            let targets = files(&request).len();
            emit(&json!({"targets": targets, "est_ms": targets}));
        }),
        "run" => request().and_then(|request| run(&request)),
        other => Err(format!("usage: describe|plan|run, not `{other}`")),
    };
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(problem) => {
            eprintln!("{problem}");
            ExitCode::FAILURE
        }
    }
}
