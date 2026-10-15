// ABOUTME: The coverage report parsers against real reports committed under `cupel-fixtures/captures/coverage/`:
// ABOUTME: every tool's report of the coverage fixture, and two formats of one run agreeing line for line.
use std::path::Path;

use cupel_adapt::coverage::{
    Format, parse, parse_cobertura, parse_coverprofile, parse_istanbul, parse_jacoco, parse_lcov,
    placed,
};
use cupel_core::coverage::CoverageMap;
use cupel_fixtures::Capture;

/// The capture of `format` by `tool`, and the named file in it.
fn report(format: &str, tool: &str, file: &str) -> (Capture, String) {
    let capture = Capture::of(&format!("coverage/{format}"), tool);
    let text = capture.file(file);
    (capture, text)
}

/// `map` with paths relative to where the capture ran.
fn relative(map: CoverageMap, capture: &Capture) -> CoverageMap {
    placed(map, &[capture.ran_in.as_path()], ".")
}

/// Each file's `(path, lines, lines that ran, branches or None, branches taken)`.
fn summary(map: &CoverageMap) -> Vec<(String, usize, usize, Option<usize>, usize)> {
    map.files
        .iter()
        .map(|file| {
            let branches = file.branches.as_ref();
            (
                file.file.clone(),
                file.lines.len(),
                file.lines.iter().filter(|hits| hits.hits > 0).count(),
                branches.map(Vec::len),
                branches.map_or(0, |b| b.iter().filter(|hits| hits.hits > 0).count()),
            )
        })
        .collect()
}

fn row(
    file: &str,
    lines: usize,
    ran: usize,
    branches: Option<usize>,
    taken: usize,
) -> (String, usize, usize, Option<usize>, usize) {
    (file.to_owned(), lines, ran, branches, taken)
}

/// The lines of `file` that never ran.
fn missed(map: &CoverageMap, file: &str) -> Vec<u32> {
    map.files
        .iter()
        .filter(|coverage| coverage.file == file)
        .flat_map(|coverage| &coverage.lines)
        .filter(|hits| hits.hits == 0)
        .map(|hits| hits.line)
        .collect()
}

#[test]
fn cargo_llvm_cov_lcov_has_absolute_paths_counts_and_no_branches() {
    let (capture, text) = report("lcov", "cargo-llvm-cov", "lcov.info");
    let map = parse_lcov(&text).unwrap();
    assert!(Path::new(&map.files[0].file).is_absolute());
    let map = relative(map, &capture);
    assert_eq!(summary(&map), [row("src/lib.rs", 31, 14, None, 0)]);
    assert_eq!(
        map.files[0].lines[2].hits, 4,
        "line 6 ran once per value and once more"
    );
    assert_eq!(
        missed(&map, "src/lib.rs"),
        [
            19, 20, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38
        ]
    );
    assert!(
        !map.branches_measured(),
        "BRF:0 without BRDA measures nothing"
    );
}

#[test]
fn pytest_cov_lcov_has_relative_paths_and_branches_with_never_evaluated_as_zero() {
    let (_, text) = report("lcov", "pytest-cov", "lcov.info");
    let map = parse_lcov(&text).unwrap();
    assert_eq!(
        summary(&map),
        [
            row("grade.py", 21, 11, Some(14), 5),
            row("tests/__init__.py", 0, 0, None, 0),
            row("tests/test_grade.py", 6, 6, None, 0),
        ]
    );
    assert_eq!(
        missed(&map, "grade.py"),
        [18, 23, 24, 25, 26, 27, 28, 29, 30, 31]
    );
    let classify = map.branch_hits("grade.py", 16, 16).unwrap();
    let hits: Vec<(u32, u64)> = classify.iter().map(|b| (b.branch, b.hits)).collect();
    assert_eq!(hits, [(0, 1), (1, 0)]);
}

#[test]
fn cobertura_from_the_same_run_agrees_with_its_lcov_line_for_line() {
    let (capture, xml) = report("cobertura", "pytest-cov", "coverage.xml");
    let cobertura = relative(parse_cobertura(&xml).unwrap(), &capture);
    let (_, lcov) = report("lcov", "pytest-cov", "lcov.info");
    let lcov = parse_lcov(&lcov).unwrap();
    assert_eq!(summary(&cobertura), summary(&lcov));
    assert_eq!(cobertura.files[0].lines, lcov.files[0].lines);
    let taken = |map: &CoverageMap| -> Vec<(u32, u64)> {
        map.files[0]
            .branches
            .iter()
            .flatten()
            .map(|hits| (hits.line, hits.hits))
            .collect()
    };
    let mut expected = taken(&lcov);
    expected.sort_by_key(|&(line, hits)| (line, std::cmp::Reverse(hits)));
    assert_eq!(
        taken(&cobertura),
        expected,
        "Cobertura lists covered branches first"
    );
}

#[test]
fn lcov_and_istanbul_json_of_one_jest_or_vitest_run_agree() {
    for (tool, file, lines, ran) in [
        ("jest", "grade.js", 19, 9),
        ("vitest", "src/grade.ts", 18, 8),
    ] {
        let (capture, json) = report("istanbul", tool, "coverage-final.json");
        let istanbul = relative(parse_istanbul(&json).unwrap(), &capture);
        let lcov = parse_lcov(&report("lcov", tool, "lcov.info").1).unwrap();
        assert_eq!(
            summary(&lcov),
            [row(file, lines, ran, Some(14), 3)],
            "{tool}"
        );
        assert_eq!(istanbul, lcov, "{tool}");
    }
}

#[test]
fn c8_lcov_counts_every_line_v8_saw() {
    let (_, text) = report("lcov", "c8", "lcov.info");
    let map = parse_lcov(&text).unwrap();
    assert_eq!(summary(&map), [row("grade.js", 38, 22, Some(6), 5)]);
}

#[test]
fn go_coverprofile_names_files_by_import_path_and_never_measures_branches() {
    let (_, text) = report("coverprofile", "go-test", "coverage.out");
    let map = parse_coverprofile(&text).unwrap();
    assert_eq!(
        summary(&map),
        [row("example.com/grade/grade.go", 28, 13, None, 0)]
    );
    assert_eq!(
        missed(&map, "example.com/grade/grade.go"),
        [21, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38]
    );
}

#[test]
fn jacoco_names_files_by_package_and_has_branches_without_counts() {
    let (_, text) = report("jacoco", "jacoco-cli", "jacoco.xml");
    let map = parse_jacoco(&text).unwrap();
    assert_eq!(summary(&map), [row("grade/Grade.java", 19, 8, Some(16), 5)]);
    assert!(
        map.files[0].lines.iter().all(|hits| hits.hits <= 1),
        "JaCoCo has no counts"
    );
}

#[test]
fn the_format_is_recognized_from_the_text() {
    let cases = [
        ("lcov", "cargo-llvm-cov", "lcov.info", Format::Lcov),
        ("lcov", "jest", "lcov.info", Format::Lcov),
        ("cobertura", "pytest-cov", "coverage.xml", Format::Cobertura),
        (
            "istanbul",
            "vitest",
            "coverage-final.json",
            Format::Istanbul,
        ),
        (
            "coverprofile",
            "go-test",
            "coverage.out",
            Format::Coverprofile,
        ),
        ("jacoco", "jacoco-cli", "jacoco.xml", Format::Jacoco),
    ];
    for (format, tool, file, expected) in cases {
        let (_, text) = report(format, tool, file);
        assert_eq!(Format::of(&text), Some(expected), "{format}/{tool}");
        assert_eq!(parse(&text), expected.parse(&text), "{format}/{tool}");
    }
    assert_eq!(Format::of("hello"), None);
    assert!(parse("hello").is_err());
}

#[test]
fn malformed_reports_are_errors_naming_their_format() {
    let cases = [
        parse_lcov("DA:1,1\n"),
        parse_lcov("SF:a\nDA:x,1\n"),
        parse_coverprofile("example.com/a.go:1.1,2.2 1 1\n"),
        parse_coverprofile("mode: set\nnonsense\n"),
        parse_istanbul("[1, 2]"),
        parse_cobertura("<report/>"),
        parse_jacoco("<coverage/>"),
    ];
    let formats: Vec<&str> = cases
        .iter()
        .map(|case| case.as_ref().unwrap_err().format)
        .collect();
    assert_eq!(
        formats,
        [
            "LCOV",
            "LCOV",
            "coverprofile",
            "coverprofile",
            "Istanbul JSON",
            "Cobertura XML",
            "JaCoCo XML"
        ]
    );
}

#[test]
fn placing_a_report_keeps_files_below_where_it_ran_under_the_project_root() {
    let map = CoverageMap::new(
        [
            "/work/web/src/a.ts",
            "/elsewhere/dep.ts",
            "./src/b.ts",
            "src/c.ts",
            "../outside.ts",
        ]
        .iter()
        .map(|file| cupel_core::coverage::FileCoverage {
            file: (*file).to_owned(),
            lines: Vec::new(),
            branches: None,
        })
        .collect(),
    );
    let placed = placed(map, &[Path::new("/work/web")], "web");
    let files: Vec<&str> = placed.files.iter().map(|file| file.file.as_str()).collect();
    assert_eq!(files, ["web/src/a.ts", "web/src/b.ts", "web/src/c.ts"]);
}
