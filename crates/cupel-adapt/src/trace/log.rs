// ABOUTME: The trace logging convention (ADR-116): one JSON object per line, a step's logged state and optionally the
// ABOUTME: spec action it took and where it was logged, read into steps every trace validator checks.
use serde_json::{Map, Value};

/// One step an implementation logged.
#[derive(Debug, Clone, PartialEq)]
pub struct Logged {
    /// The spec action the step took, when the implementation named it.
    pub action: Option<String>,
    /// The values of the spec's variables after the step, by variable; variables not logged
    /// are left to the spec.
    pub state: Map<String, Value>,
    /// Where the step was logged, as the implementation named the file.
    pub file: Option<String>,
    pub line: Option<u32>,
}

/// The keys a step may have.
const KEYS: [&str; 4] = ["state", "action", "file", "line"];

/// The steps of a trace file's `text`, one per non-empty line; why it is not a trace otherwise.
pub fn read(text: &str) -> Result<Vec<Logged>, String> {
    let steps: Vec<Logged> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| step(line).map_err(|problem| format!("line {}: {problem}", index + 1)))
        .collect::<Result<_, _>>()?;
    if steps.is_empty() {
        return Err("the trace logs no step".to_owned());
    }
    Ok(steps)
}

fn step(line: &str) -> Result<Logged, String> {
    let value: Value = serde_json::from_str(line).map_err(|error| format!("not JSON: {error}"))?;
    let Value::Object(object) = value else {
        return Err("a step is a JSON object".to_owned());
    };
    if let Some(key) = object.keys().find(|key| !KEYS.contains(&key.as_str())) {
        return Err(format!(
            "unknown key `{key}`; a step has `state` and may have `action`, `file`, and `line`"
        ));
    }
    let state = match object.get("state") {
        Some(Value::Object(state)) => state.clone(),
        _ => return Err("a step's `state` is a JSON object of the spec's variables".to_owned()),
    };
    let text = |key: &str| -> Result<Option<String>, String> {
        match object.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(text)) => Ok(Some(text.clone())),
            Some(_) => Err(format!("`{key}` is a string")),
        }
    };
    let line = match object.get("line") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_u64()
                .and_then(|line| u32::try_from(line).ok())
                .filter(|line| *line > 0)
                .ok_or("`line` is a positive integer")?,
        ),
    };
    Ok(Logged {
        action: text("action")?,
        state,
        file: text("file")?,
        line,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn each_line_is_a_step_with_its_state_and_what_else_was_logged() {
        let steps = read(
            "{\"state\": {\"count\": 0}}\n\n\
             {\"action\": \"Inc\", \"state\": {\"count\": 1}, \"file\": \"src/lib.rs\", \"line\": 9}\n",
        )
        .unwrap();
        assert_eq!(
            steps,
            [
                Logged {
                    action: None,
                    state: json!({"count": 0}).as_object().unwrap().clone(),
                    file: None,
                    line: None,
                },
                Logged {
                    action: Some("Inc".to_owned()),
                    state: json!({"count": 1}).as_object().unwrap().clone(),
                    file: Some("src/lib.rs".to_owned()),
                    line: Some(9),
                },
            ]
        );
    }

    #[test]
    fn a_line_that_is_not_a_step_names_its_line_and_the_problem() {
        let problems = [
            "",
            "{\"state\": {}}\nnot json\n",
            "[1]",
            "{\"count\": 1}",
            "{\"state\": 1}",
            "{\"state\": {}, \"action\": 2}",
            "{\"state\": {}, \"line\": 0}",
        ]
        .map(|text| read(text).unwrap_err());
        assert_eq!(problems[0], "the trace logs no step");
        assert!(
            problems[1].starts_with("line 2: not JSON: "),
            "{}",
            problems[1]
        );
        assert_eq!(
            problems[2..],
            [
                "line 1: a step is a JSON object",
                "line 1: unknown key `count`; a step has `state` and may have `action`, `file`, \
                 and `line`",
                "line 1: a step's `state` is a JSON object of the spec's variables",
                "line 1: `action` is a string",
                "line 1: `line` is a positive integer",
            ]
        );
    }
}
