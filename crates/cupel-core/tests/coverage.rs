// ABOUTME: The `CoverageMap` payload (ADR-067, ADR-077): building one merges and sorts files, lines, and
// ABOUTME: branches; symbol line coverage stays in [0, 1]; branch coverage exists only where it was measured.
use cupel_core::coverage::{BranchHits, CoverageMap, FileCoverage};
use proptest::prelude::*;

fn file(name: &str, lines: &[(u32, u64)], branches: Option<&[(u32, u32, u64)]>) -> FileCoverage {
    FileCoverage::from_hits(name, lines, branches)
}

fn branches(hits: &[(u32, u32, u64)]) -> Vec<BranchHits> {
    file("", &[], Some(hits)).branches.unwrap()
}

#[test]
fn building_a_map_merges_records_of_one_file_adding_hits_and_sorts_everything() {
    let map = CoverageMap::new(vec![
        file("b.rs", &[(3, 1)], None),
        file("a.rs", &[(9, 0), (2, 1)], Some(&[(9, 1, 0)])),
        file("a.rs", &[(2, 2), (5, 0)], Some(&[(9, 1, 3), (9, 0, 1)])),
    ]);
    assert_eq!(
        map,
        CoverageMap {
            files: vec![
                file(
                    "a.rs",
                    &[(2, 3), (5, 0), (9, 0)],
                    Some(&[(9, 0, 1), (9, 1, 3)])
                ),
                file("b.rs", &[(3, 1)], None),
            ]
        }
    );
}

#[test]
fn a_file_measured_for_branches_in_any_record_keeps_its_branches() {
    let map = CoverageMap::new(vec![
        file("a.py", &[(1, 1)], None),
        file("a.py", &[(1, 1)], Some(&[(1, 0, 1)])),
    ]);
    assert_eq!(map.files[0].branches, Some(branches(&[(1, 0, 1)])));
    assert!(map.branches_measured());
    let unmeasured = CoverageMap::new(vec![file("a.go", &[(1, 1)], None)]);
    assert!(!unmeasured.branches_measured());
}

#[test]
fn branch_hits_are_the_branches_on_lines_of_the_range_and_none_when_unmeasured() {
    let map = CoverageMap::new(vec![
        file(
            "a.ts",
            &[(1, 1), (4, 1)],
            Some(&[(2, 0, 1), (2, 1, 0), (9, 0, 0)]),
        ),
        file("b.go", &[(1, 1)], None),
    ]);
    let found = map
        .branch_hits("a.ts", 1, 4)
        .expect("a.ts measures branches");
    assert_eq!(
        found
            .iter()
            .map(|hits| (hits.line, hits.branch, hits.hits))
            .collect::<Vec<_>>(),
        [(2, 0, 1), (2, 1, 0)]
    );
    assert_eq!(map.branch_hits("b.go", 1, 4), None);
    assert_eq!(map.branch_hits("missing.rs", 1, 4), None);
}

#[test]
fn line_hits_are_the_instrumented_lines_of_the_range() {
    let map = CoverageMap::new(vec![file("a.rs", &[(1, 1), (3, 0), (8, 2)], None)]);
    let found: Vec<(u32, u64)> = map
        .line_hits("a.rs", 2, 8)
        .iter()
        .map(|hits| (hits.line, hits.hits))
        .collect();
    assert_eq!(found, [(3, 0), (8, 2)]);
    assert_eq!(map.line_coverage("a.rs", 2, 8), Some(0.5));
    assert_eq!(map.line_coverage("a.rs", 4, 7), None);
}

proptest! {
    #[test]
    fn symbol_line_coverage_is_a_share_of_its_instrumented_lines(
        records in proptest::collection::vec(
            proptest::collection::vec((1u32..60, 0u64..5), 0..20),
            1..4,
        ),
        start in 1u32..60,
        length in 0u32..30,
    ) {
        let map = CoverageMap::new(
            records.iter().map(|hits| file("f.rs", hits, None)).collect(),
        );
        let end = start + length;
        let instrumented = map.line_hits("f.rs", start, end);
        match map.line_coverage("f.rs", start, end) {
            None => prop_assert!(instrumented.is_empty()),
            Some(share) => {
                prop_assert!((0.0..=1.0).contains(&share));
                let ran = instrumented.iter().filter(|hits| hits.hits > 0).count();
                prop_assert!(ran == 0 || share > 0.0);
                prop_assert!(ran < instrumented.len() || (share - 1.0).abs() < f64::EPSILON);
            }
        }
        let lines: Vec<u32> = map.files[0].lines.iter().map(|hits| hits.line).collect();
        prop_assert!(lines.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
