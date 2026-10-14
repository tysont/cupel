// ABOUTME: Test listings (ADR-077): the IDs a runner would run, read from `cargo nextest list`, `go test -list`,
// ABOUTME: and `pytest --collect-only` output, in the same ID schemes their `TestResults` use (ADR-075).

/// The tests nextest would run, `<binary-id> <test path>`, from `cargo nextest list
/// --message-format json`, sorted; ignored tests are left out, as nextest runs none.
pub fn nextest(stdout: &str) -> Vec<String> {
    let Ok(list) = serde_json::from_str::<serde_json::Value>(stdout) else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    for suite in list["rust-suites"]
        .as_object()
        .into_iter()
        .flat_map(|suites| suites.values())
    {
        let binary = suite["binary-id"].as_str().unwrap_or_default();
        for (name, case) in suite["testcases"].as_object().into_iter().flatten() {
            if case["ignored"] != serde_json::Value::Bool(true) {
                ids.push(format!("{binary} {name}"));
            }
        }
    }
    ids.sort();
    ids
}

/// The tests `go test -list ^Test -json` printed, `<package>/<Test>`.
pub fn go(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|event| event["Action"] == "output")
        .filter_map(|event| {
            let package = event["Package"].as_str()?;
            let name = event["Output"].as_str()?.trim();
            let test = name.starts_with("Test") && !name.contains(char::is_whitespace);
            test.then(|| format!("{package}/{name}"))
        })
        .collect()
}

/// The node IDs `pytest --collect-only -q` prints before its summary.
pub fn pytest(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .take_while(|line| !line.trim().is_empty())
        .filter(|line| line.contains("::"))
        .map(|line| line.trim().to_owned())
        .collect()
}
