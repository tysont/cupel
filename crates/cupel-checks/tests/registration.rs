// ABOUTME: How native checks join the registry (ADR-074): each module submits itself, so `lib.rs`
// ABOUTME: names a check only on its `mod` line, and `register` lists checks in ID order.
mod support;

/// `lib.rs` as written, so the test sees exactly what a new check would have to edit. Comments
/// may name checks; code may not.
const LIB: &str = include_str!("../src/lib.rs");

/// The module a check lives in: its ID with `.` and `-` as `_` (`static.dead-code` is
/// `static_dead_code`).
fn module_of(id: &str) -> String {
    id.replace(['.', '-'], "_")
}

#[test]
fn lib_names_each_check_only_on_its_mod_line() {
    let registry = support::registry();
    let mut checked = 0;
    for check in registry.iter() {
        let module = module_of(check.manifest().id.as_str());
        let mentions: Vec<&str> = LIB
            .lines()
            .filter(|line| !line.trim_start().starts_with("//") && line.contains(module.as_str()))
            .collect();
        assert_eq!(
            mentions,
            [format!("mod {module};")],
            "{}: a check joins the registry from its own module, so lib.rs names it only in `mod {module};`",
            check.manifest().id
        );
        checked += 1;
    }
    assert!(checked > 0, "no checks are registered");
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
