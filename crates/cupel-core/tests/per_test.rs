// ABOUTME: Tests for the `PerTestMap` payload (ADR-077): how tests' covered lines are normalized and how the
// ABOUTME: inverted index answers which tests execute a range of lines of a file.
use cupel_core::{CoveredLines, FactKind, PerTestMap, ProducerId, TestCoverage, schema};

fn coverage(id: &str, covered: &[(&str, &[u32])]) -> TestCoverage {
    TestCoverage::new(
        id,
        None,
        covered
            .iter()
            .map(|(file, lines)| CoveredLines {
                file: (*file).to_owned(),
                lines: lines.to_vec(),
            })
            .collect(),
    )
}

fn map() -> PerTestMap {
    PerTestMap::new(
        ProducerId::from("tool.cargo-nextest"),
        vec![
            coverage("calc tests::scales", &[("src/lib.rs", &[11, 9, 10, 10])]),
            coverage(
                "calc tests::adds",
                &[("src/lib.rs", &[4, 5, 6]), ("src/util.rs", &[2])],
            ),
            coverage("calc tests::nothing", &[]),
        ],
    )
}

#[test]
fn tests_are_sorted_by_id_and_lines_ascend_without_repeats() {
    let map = map();
    let ids: Vec<&str> = map.tests.iter().map(|test| test.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "calc tests::adds",
            "calc tests::nothing",
            "calc tests::scales"
        ]
    );
    assert_eq!(map.tests[2].covered[0].lines, [9, 10, 11]);
}

#[test]
fn files_of_one_test_are_sorted_and_merged() {
    let test = coverage("t", &[("b.rs", &[3]), ("a.rs", &[2]), ("b.rs", &[1, 3])]);
    let files: Vec<(&str, &[u32])> = test
        .covered
        .iter()
        .map(|file| (file.file.as_str(), file.lines.as_slice()))
        .collect();
    assert_eq!(files, [("a.rs", &[2][..]), ("b.rs", &[1, 3][..])]);
}

#[test]
fn the_index_names_the_tests_that_execute_a_range_with_the_first_line_each_executes() {
    let map = map();
    let index = map.index();
    let ids = |file: &str, start: u32, end: u32| -> Vec<(String, u32)> {
        index
            .covering(file, start, end)
            .into_iter()
            .map(|(test, line)| (test.id.clone(), line))
            .collect()
    };
    assert_eq!(
        ids("src/lib.rs", 4, 6),
        [("calc tests::adds".to_owned(), 4)]
    );
    assert_eq!(
        ids("src/lib.rs", 10, 20),
        [("calc tests::scales".to_owned(), 10)]
    );
    assert_eq!(
        ids("src/lib.rs", 1, u32::MAX),
        [
            ("calc tests::adds".to_owned(), 4),
            ("calc tests::scales".to_owned(), 9)
        ]
    );
    assert!(ids("src/lib.rs", 7, 8).is_empty());
    assert!(ids("src/other.rs", 1, 100).is_empty());
}

#[test]
fn the_payload_has_a_schema_and_version_one() {
    assert_eq!(schema::fact_version(FactKind::PerTestMap), Some(1));
}
