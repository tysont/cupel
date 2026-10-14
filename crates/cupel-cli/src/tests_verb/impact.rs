// ABOUTME: Which tests a scope impacts (ADR-077 §5): the lines in scope per file, widened to the symbols a change
// ABOUTME: touches, then the tests a per-test map says execute them, or the test files that import them (ADR-006).
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use cupel_core::syntax::{
    ImportTarget, ModuleGraph, SymbolDef, Symbols, TestConventions, TestFrames,
};
use cupel_core::{ImpactedTest, LangId, PerTestMap, TestCoverage};

/// Line ranges per file (inclusive): the lines in scope, or the lines a range changed.
pub type Regions = BTreeMap<String, Vec<(u32, u32)>>;

/// Every line of a file.
const WHOLE: (u32, u32) = (1, u32::MAX);

/// Every line of each of `files`.
pub fn whole(files: &[String]) -> Regions {
    files
        .iter()
        .map(|file| (file.clone(), vec![WHOLE]))
        .collect()
}

/// The lines a range changed in each of `files` (`hunks`; a file without hunks, such as an
/// added one, counts whole), each widened to the spans of the symbols it touches.
pub fn widened(files: &[String], hunks: &Regions, symbols: Option<&Symbols>) -> Regions {
    files
        .iter()
        .map(|file| {
            let Some(changed) = hunks.get(file) else {
                return (file.clone(), vec![WHOLE]);
            };
            let mut ranges = changed.clone();
            for symbol in symbols_of(symbols, file) {
                let span = (symbol.span.line, symbol.span.end_line);
                if changed.iter().any(|range| overlaps(*range, span)) {
                    ranges.push(span);
                }
            }
            (file.clone(), ranges)
        })
        .collect()
}

/// The spans of the symbols of `symbols` whose keys `selected` holds, by file.
pub fn spans(
    symbols: Option<&Symbols>,
    selected: &BTreeSet<cupel_core::syntax::SymbolKey>,
) -> Regions {
    let mut regions = Regions::new();
    for symbol in symbols.iter().flat_map(|symbols| &symbols.symbols) {
        if selected.contains(&symbol.key()) {
            regions
                .entry(symbol.file.clone())
                .or_default()
                .push((symbol.span.line, symbol.span.end_line));
        }
    }
    regions
}

/// How many of `symbols` the regions touch.
pub fn touched(regions: &Regions, symbols: Option<&Symbols>) -> u64 {
    let count = regions
        .iter()
        .flat_map(|(file, ranges)| {
            symbols_of(symbols, file).filter(|symbol| {
                let span = (symbol.span.line, symbol.span.end_line);
                ranges.iter().any(|range| overlaps(*range, span))
            })
        })
        .count();
    count as u64
}

/// The tests `map` says execute a line of `regions`, and the tests whose own definition
/// (`frames`) the regions touch, each with why, in ID order.
pub fn by_coverage(
    map: &PerTestMap,
    regions: &Regions,
    symbols: Option<&Symbols>,
    frames: Option<&TestFrames>,
    lang: &LangId,
) -> Vec<ImpactedTest> {
    let index = map.index();
    let mut found: BTreeMap<String, ImpactedTest> = BTreeMap::new();
    let mut add = |test: &TestCoverage, reason: String| {
        found
            .entry(test.id.clone())
            .or_insert_with(|| ImpactedTest {
                id: test.id.clone(),
                file: test.file.clone().or_else(|| file_of_case(frames, &test.id)),
                reason,
                lang: lang.clone(),
            });
    };
    for (file, ranges) in regions {
        for (start, end) in ranges {
            for (test, line) in index.covering(file, *start, *end) {
                add(test, format!("covers {}", place(symbols, file, line)));
            }
        }
    }
    for test_file in frames.iter().flat_map(|frames| &frames.files) {
        let Some(ranges) = regions.get(&test_file.file) else {
            continue;
        };
        for case in &test_file.cases {
            let span = (case.span.line, case.span.end_line);
            if !ranges.iter().any(|range| overlaps(*range, span)) {
                continue;
            }
            let named = map.tests.iter().filter(|test| {
                case_name(&test.id) == case.name
                    && test
                        .file
                        .as_ref()
                        .is_none_or(|file| *file == test_file.file)
            });
            for test in named {
                add(
                    test,
                    format!("the test changed ({}:{})", test_file.file, case.span.line),
                );
            }
        }
    }
    found.into_values().collect()
}

/// The test files (`conventions`) that are in `regions` or import a file there, directly or
/// through other files of `graph`, each with why, in path order.
pub fn by_imports(
    graph: &ModuleGraph,
    conventions: &TestConventions,
    regions: &Regions,
    lang: &LangId,
) -> Vec<ImpactedTest> {
    let mut importers: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for edge in graph.roots.iter().flat_map(|root| &root.edges) {
        if let ImportTarget::File { path } = &edge.target {
            importers
                .entry(path.as_str())
                .or_default()
                .insert(edge.from.as_str());
        }
    }
    let mut found: BTreeMap<String, ImpactedTest> = BTreeMap::new();
    for file in regions.keys() {
        for (reached, hop) in reaching(&importers, file) {
            if !conventions.is_test(reached) {
                continue;
            }
            let reason = match hop {
                None => "is in scope".to_owned(),
                Some(hop) if hop == reached => format!("imports {file}"),
                Some(hop) => format!("imports {file} through {hop}"),
            };
            found
                .entry(reached.to_owned())
                .or_insert_with(|| ImpactedTest {
                    id: reached.to_owned(),
                    file: Some(reached.to_owned()),
                    reason,
                    lang: lang.clone(),
                });
        }
    }
    found.into_values().collect()
}

/// `file` and every file that imports it through `importers`, each with the importer of `file`
/// its path starts from (`None` for `file` itself), nearest first.
fn reaching<'a>(
    importers: &BTreeMap<&str, BTreeSet<&'a str>>,
    file: &'a str,
) -> Vec<(&'a str, Option<&'a str>)> {
    let mut reached = vec![(file, None)];
    let mut seen: BTreeSet<&str> = BTreeSet::from([file]);
    let mut queue: VecDeque<(&str, Option<&str>)> = VecDeque::from([(file, None)]);
    while let Some((current, hop)) = queue.pop_front() {
        for importer in importers.get(current).into_iter().flatten() {
            if seen.insert(importer) {
                let hop = hop.or(Some(*importer));
                reached.push((importer, hop));
                queue.push_back((importer, hop));
            }
        }
    }
    reached
}

/// `file:line`, after the symbol that holds it when there is one.
fn place(symbols: Option<&Symbols>, file: &str, line: u32) -> String {
    symbols_of(symbols, file)
        .filter(|symbol| symbol.span.line <= line && line <= symbol.span.end_line)
        .min_by_key(|symbol| symbol.span.end_line - symbol.span.line)
        .map_or_else(
            || format!("{file}:{line}"),
            |symbol| format!("{} ({file}:{line})", symbol.display),
        )
}

fn symbols_of<'a>(
    symbols: Option<&'a Symbols>,
    file: &'a str,
) -> impl Iterator<Item = &'a SymbolDef> {
    symbols
        .into_iter()
        .flat_map(|symbols| &symbols.symbols)
        .filter(move |symbol| symbol.file == file)
}

/// The test case name a runner's test ID ends with: its last `::`, `/`, or space-separated part.
fn case_name(id: &str) -> &str {
    id.rsplit(['/', ' ', ':']).next().unwrap_or(id)
}

/// The file of the test case `frames` names as `id`'s case.
fn file_of_case(frames: Option<&TestFrames>, id: &str) -> Option<String> {
    let name = case_name(id);
    frames?
        .files
        .iter()
        .find(|file| file.cases.iter().any(|case| case.name == name))
        .map(|file| file.file.clone())
}

fn overlaps((a_start, a_end): (u32, u32), (b_start, b_end): (u32, u32)) -> bool {
    a_start <= b_end && b_start <= a_end
}
