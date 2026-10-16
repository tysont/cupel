// ABOUTME: The Rust reference producer plugin (ADR-092): a `per-test-map` fact for JavaScript and TypeScript read
// ABOUTME: from a committed LCOV file with one `TN:` section per test; speaks the plugin protocol with `serde_json` alone.
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;
use std::process::ExitCode;

use serde_json::{Value, json};

const PRODUCER: &str = "plugin.ref-rust";
const DEFAULT_LCOV: &str = "coverage/per-test.lcov";

/// Each test's covered lines, by file.
type Tests = BTreeMap<String, BTreeMap<String, BTreeSet<u32>>>;

fn manifest() -> Value {
    json!({
        "kind": "producer",
        "id": PRODUCER,
        "version": 1,
        "provides": {"kind": "per-test-map", "version": 2},
        "languages": ["javascript", "typescript"],
        "needs": [],
        "wants": [],
        "cost": "quick",
        "stability": "experimental",
        "summary": "The lines each test executes, from an LCOV file with a TN: section per test."
    })
}

/// Each `TN:` test's covered lines, by file: `DA:<line>,<hits>` records with hits above zero.
fn per_test(text: &str) -> Tests {
    let mut tests = Tests::new();
    let (mut current, mut file) = (None::<String>, None::<String>);
    for record in text.lines() {
        let (key, value) = record.split_once(':').unwrap_or((record, ""));
        match key {
            "TN" => {
                tests.entry(value.to_owned()).or_default();
                current = Some(value.to_owned());
            }
            "SF" => file = Some(value.to_owned()),
            "DA" => {
                let mut fields = value.split(',').map(|field| field.parse::<u64>().ok());
                if let (Some(test), Some(file), Some(Some(line)), Some(Some(hits))) =
                    (&current, &file, fields.next(), fields.next())
                    && hits > 0
                    && let Ok(line) = u32::try_from(line)
                {
                    let files = tests.entry(test.clone()).or_default();
                    files.entry(file.clone()).or_default().insert(line);
                }
            }
            _ => {}
        }
    }
    tests
}

fn payload(tests: &Tests) -> Value {
    let tests: Vec<Value> = tests
        .iter()
        .map(|(id, files)| {
            let covered: Vec<Value> = files
                .iter()
                .map(|(file, lines)| json!({"file": file, "lines": lines}))
                .collect();
            json!({"id": id, "file": null, "covered": covered})
        })
        .collect();
    json!({"runner": "tool.vitest", "measured": tests.len(), "reused": 0, "tests": tests})
}

fn run(request: &Value) -> Result<(), String> {
    let root = request["root"]
        .as_str()
        .ok_or("the request names no root")?;
    let lcov = request["config"]["per_test_lcov"]
        .as_str()
        .unwrap_or(DEFAULT_LCOV);
    let text = std::fs::read_to_string(Path::new(root).join(lcov))
        .map_err(|error| format!("cannot read the per-test LCOV file: {error}"))?;
    let fact = json!({
        "ev": "fact",
        "kind": "per-test-map",
        "version": 2,
        "payload": payload(&per_test(&text))
    });
    println!("{fact}");
    Ok(())
}

fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_default();
    let done = match command.as_str() {
        "describe" => {
            println!("{}", manifest());
            Ok(())
        }
        "run" => {
            let mut text = String::new();
            std::io::stdin()
                .read_to_string(&mut text)
                .map_err(|error| error.to_string())
                .and_then(|_| serde_json::from_str(&text).map_err(|error| error.to_string()))
                .and_then(|request: Value| run(&request))
        }
        other => Err(format!("usage: describe|run, not `{other}`")),
    };
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(problem) => {
            eprintln!("{problem}");
            ExitCode::FAILURE
        }
    }
}
