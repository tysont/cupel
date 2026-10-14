// ABOUTME: Parses the verbose text `python -m unittest -v -b` prints: one `name (id) ... result` line per test,
// ABOUTME: then a FAIL or ERROR block per failure with its traceback and buffered output.
use std::collections::BTreeMap;

use cupel_core::{TestOutcome, TestResult, output_tail};

/// The rule line under a failure block's header and above the run summary.
const RULE: &str = "----------------------------------------------------------------------";
/// The line above each failure block.
const BLOCK: &str = "======================================================================";

/// The tests in `text`, the merged output of `python -m unittest discover -v -b`, with IDs the
/// dotted `<module>.<Class>.<method>` and files derived from the module.
pub fn parse(text: &str) -> Vec<TestResult> {
    let blocks = failure_blocks(text);
    let mut tests = Vec::new();
    let mut pending: Option<(String, String)> = None;
    for line in text.lines() {
        if line == BLOCK {
            break;
        }
        // A test with a docstring prints `name (id)` alone, then the docstring's first line
        // and the result.
        let Some((head, result)) = line.rsplit_once(" ... ") else {
            pending = header(line);
            continue;
        };
        let Some(outcome) = outcome(result) else {
            continue;
        };
        let Some((name, id)) = header(head).or_else(|| pending.take()) else {
            continue;
        };
        tests.push(TestResult {
            file: file_of(&id),
            output: blocks.get(&id).and_then(|block| output_tail(block)),
            id,
            name,
            outcome,
            duration_ms: None,
        });
    }
    tests
}

/// `test_x (tests.test_red.RedTest.test_x)` → the name and the dotted ID (Python before 3.11
/// prints only the class in parentheses, so the name is appended).
fn header(line: &str) -> Option<(String, String)> {
    let (name, rest) = line.split_once(" (")?;
    let dotted = rest.strip_suffix(')')?;
    if name.contains(' ') || dotted.contains(' ') {
        return None;
    }
    let id = if dotted.ends_with(&format!(".{name}")) {
        dotted.to_owned()
    } else {
        format!("{dotted}.{name}")
    };
    Some((name.to_owned(), id))
}

fn outcome(result: &str) -> Option<TestOutcome> {
    Some(match result {
        "ok" | "expected failure" => TestOutcome::Passed,
        "FAIL" | "unexpected success" => TestOutcome::Failed,
        "ERROR" => TestOutcome::Errored,
        skipped if skipped.starts_with("skipped") => TestOutcome::Skipped,
        _ => return None,
    })
}

/// `tests.test_red.RedTest.test_x` → `tests/test_red.py`; `None` for the loader's stand-in for
/// a module that failed to import.
fn file_of(id: &str) -> Option<String> {
    if id.starts_with("unittest.") {
        return None;
    }
    let parts: Vec<&str> = id.split('.').collect();
    (parts.len() > 2).then(|| format!("{}.py", parts[..parts.len() - 2].join("/")))
}

/// Each FAIL or ERROR block's body, by the dotted ID in its header.
fn failure_blocks(text: &str) -> BTreeMap<String, String> {
    let mut blocks = BTreeMap::new();
    for block in text.split(BLOCK).skip(1) {
        let mut lines = block.trim_start_matches('\n').lines();
        let Some((_, id)) = lines
            .next()
            .and_then(|line| line.split_once(": "))
            .and_then(|(_, head)| header(head))
        else {
            continue;
        };
        let body: Vec<&str> = lines
            .skip_while(|line| *line == RULE)
            .take_while(|line| *line != RULE)
            .collect();
        blocks.insert(id, body.join("\n"));
    }
    blocks
}
