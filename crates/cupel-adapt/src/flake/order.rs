// ABOUTME: The order `go test -json` ran its tests in: test2json's `run` events, in the order they appear,
// ABOUTME: with the IDs the test2json parser gives (`<package>/<Test>`), so reruns can compare orders.
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Event {
    action: String,
    #[serde(default)]
    package: String,
    #[serde(default)]
    test: Option<String>,
}

/// Each test's ID in the order `stdout`, the events of `go test -json`, started them.
pub fn go_test_order(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Event>(line).ok())
        .filter(|event| event.action == "run")
        .filter_map(|event| Some(format!("{}/{}", event.package, event.test?)))
        .collect()
}
