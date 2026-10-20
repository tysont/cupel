// ABOUTME: How native checks join the registry (ADR-074): each module submits itself, `lib.rs` names a check
// ABOUTME: only on its `mod` line, `register` lists checks in ID order; which metrics are scores (ADR-131).
mod support;

/// `lib.rs` as written, so the test sees exactly what a new check would have to edit. Comments
/// may name checks; code may not.
const LIB: &str = include_str!("../src/lib.rs");

/// The module a check lives in: its ID with `.` and `-` as `_` (`static.dead-code` is
/// `static_dead_code`).
fn module_of(id: &str) -> String {
    id.replace(['.', '-'], "_")
}

/// Each registered check with the lines of code in `lib.rs` that name its module.
fn mentions_in_lib() -> Vec<(String, Vec<String>)> {
    support::registry()
        .iter()
        .map(|check| {
            let module = module_of(check.manifest().id.as_str());
            let lines = LIB
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .filter(|line| line.contains(module.as_str()))
                .map(str::to_owned)
                .collect();
            (module, lines)
        })
        .collect()
}

#[test]
fn lib_names_each_check_only_on_its_mod_line() {
    let mentions = mentions_in_lib();
    let only_mod_lines: Vec<(String, Vec<String>)> = mentions
        .iter()
        .map(|(module, _)| (module.clone(), vec![format!("mod {module};")]))
        .collect();
    assert!(!mentions.is_empty(), "no checks are registered");
    assert_eq!(
        mentions, only_mod_lines,
        "a check joins the registry from its own module, so lib.rs names it only in its `mod` line"
    );
}

#[test]
fn register_lists_checks_in_id_order() {
    let ids: Vec<String> = support::registry()
        .iter()
        .map(|check| check.manifest().id.to_string())
        .collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted);
}

#[test]
fn the_metrics_compared_as_weighted_means_across_matched_symbols_are_crap_and_mock_density() {
    let registry = support::registry();
    let scores: Vec<(String, String)> = registry
        .iter()
        .flat_map(|check| {
            let id = check.manifest().id.as_str().to_owned();
            check
                .manifest()
                .metrics
                .iter()
                .filter(|metric| check.averages(&metric.name))
                .map(move |metric| (id.clone(), metric.name.clone()))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(
        scores,
        [
            ("static.crap".to_owned(), "crap".to_owned()),
            ("tests.quality".to_owned(), "mock_density".to_owned()),
        ]
    );
}
