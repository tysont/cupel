// ABOUTME: The engine's plan (ADR-080 §5–7, §10): the mutants depth and focus select, coverage-gated, each with the
// ABOUTME: tests a per-test map selects for it and the cache key its outcome is stored under.
use std::collections::HashMap;

use cupel_core::coverage::CoverageMap;
use cupel_core::syntax::{
    MutationSites, NormTree, OperatorSet, Site, SymbolDef, SymbolKey, Symbols, TestConventions,
    TestFrames,
};
use cupel_core::{
    CostClass, Focus, LineIndex, Mutant, MutantOutcome, PerTestMap, Scope, SymbolHash,
};

use crate::equivalent;
use crate::mutant::replacements;

/// What planning reads.
pub struct Inputs<'a> {
    pub sites: &'a MutationSites,
    pub symbols: &'a Symbols,
    pub coverage: &'a CoverageMap,
    pub norm: Option<&'a NormTree>,
    /// Which files are tests, and the test cases in them: a test is never mutated.
    pub conventions: Option<&'a TestConventions>,
    pub frames: Option<&'a TestFrames>,
    /// The per-test map, when its runner is the one that runs the mutants.
    pub per_test: Option<&'a PerTestMap>,
    pub focus: &'a Focus,
    pub scope: &'a Scope,
    pub operators: OperatorSet,
    /// The baseline's suite hash: the suite hash of a mutant that runs the whole suite.
    pub suite_hash: &'a str,
    /// Identifies the engine, the runner, and its version.
    pub toolchain_hash: &'a str,
}

/// One mutant to account for.
#[derive(Debug, Clone)]
pub struct Planned {
    /// Its outcome is `uncovered` until it is run or reused.
    pub mutant: Mutant,
    /// The mutated source of its file.
    pub source: String,
    /// The tests to run against it; `None` for the whole suite.
    pub tests: Option<Vec<String>>,
    /// Whether a test runs its site, so it is run (coverage gating).
    pub covered: bool,
}

impl Planned {
    pub fn file(&self) -> &str {
        &self.mutant.symbol.file
    }
}

/// Every mutant `inputs` select, reading each file's source with `read`.
pub fn plan(inputs: &Inputs<'_>, read: &dyn Fn(&str) -> Option<String>) -> Vec<Planned> {
    let symbols: HashMap<SymbolKey, &SymbolDef> = inputs
        .symbols
        .symbols
        .iter()
        .map(|symbol| (symbol.key(), symbol))
        .collect();
    let hashes: HashMap<&SymbolKey, &SymbolHash> = inputs
        .norm
        .map(|norm| {
            norm.symbols
                .iter()
                .map(|symbol| (&symbol.symbol, &symbol.hash))
                .collect()
        })
        .unwrap_or_default();
    let planner = Planner {
        inputs,
        index: inputs.per_test.map(PerTestMap::index),
        read,
    };
    let mut sources: HashMap<String, Option<String>> = HashMap::new();
    let mut found = Vec::new();
    for entry in &inputs.sites.symbols {
        let Some(symbol) = symbols.get(&entry.symbol).filter(|s| in_focus(inputs, s)) else {
            continue;
        };
        let source = sources
            .entry(symbol.file.clone())
            .or_insert_with(|| read(&symbol.file));
        let Some(source) = source.as_deref() else {
            continue;
        };
        let at = Symbol {
            def: symbol,
            hash: hashes.get(&entry.symbol).map(|hash| (*hash).clone()),
            source_hash: source_hash(source, symbol),
        };
        for site in entry
            .sites
            .iter()
            .filter(|site| wanted(inputs, site) && executable(inputs.coverage, symbol, site))
        {
            found.extend(planner.mutants(&at, site, source));
        }
    }
    found
}

/// A symbol being planned: its definition, structural hash, and source text hash.
struct Symbol<'a> {
    def: &'a SymbolDef,
    hash: Option<SymbolHash>,
    source_hash: String,
}

/// Whether `symbol` is test code: in a test file, or a test case of `TestFrames` (a Rust test
/// beside the code it tests).
fn is_test(inputs: &Inputs<'_>, symbol: &SymbolDef) -> bool {
    inputs
        .conventions
        .is_some_and(|conventions| conventions.is_test(&symbol.file))
        || inputs.frames.is_some_and(|frames| {
            frames
                .files
                .iter()
                .filter(|file| file.file == symbol.file)
                .flat_map(|file| &file.cases)
                .any(|case| {
                    case.span.line <= symbol.span.line && symbol.span.line <= case.span.end_line
                })
        })
}

/// Whether the plan covers `symbol`: code, not a test; in scope; among `--symbols` when given;
/// and at `standard` with a range, holding a changed line (ADR-080 §5).
fn in_focus(inputs: &Inputs<'_>, symbol: &SymbolDef) -> bool {
    let key = symbol.key();
    let focus = inputs.focus;
    !is_test(inputs, symbol)
        && inputs.scope.includes(&key)
        && focus
            .symbols
            .as_ref()
            .is_none_or(|keys| keys.contains(&key))
        && (focus.depth != CostClass::Standard
            || focus.changed.as_ref().is_none_or(|changed| {
                changed.get(&symbol.file).is_some_and(|hunks| {
                    hunks.iter().any(|(start, end)| {
                        *start <= symbol.span.end_line && symbol.span.line <= *end
                    })
                })
            }))
}

/// Whether `site` is on code the build runs: a file the coverage map instruments but whose
/// instrumented lines miss the site's (a `cfg` or build tag left it out, or a constant the
/// compiler evaluates) holds no mutant a test can reach. A file the map does not hold may be
/// one no test loads, so its sites stay.
fn executable(coverage: &CoverageMap, symbol: &SymbolDef, site: &Site) -> bool {
    !coverage.instruments(&symbol.file)
        || !coverage
            .line_hits(&symbol.file, site.span.line, site.span.end_line)
            .is_empty()
}

/// Whether the configured operator set mutates `site`.
fn wanted(inputs: &Inputs<'_>, site: &Site) -> bool {
    inputs.operators == OperatorSet::Extended || site.set == OperatorSet::Default
}

/// What planning one site reads: the inputs, the per-test index, and the source reader.
struct Planner<'a, 'r> {
    inputs: &'a Inputs<'a>,
    index: Option<LineIndex<'a>>,
    read: &'r dyn Fn(&str) -> Option<String>,
}

/// How a site's mutants run: whether a test runs the site, the tests selected for it (`None`
/// for the whole suite), and the suite hash of that selection.
struct Selection {
    covered: bool,
    tests: Option<Vec<String>>,
    suite_hash: String,
}

impl Planner<'_, '_> {
    /// The mutants of `site` in `symbol`, whose file's text is `source`.
    fn mutants(&self, symbol: &Symbol<'_>, site: &Site, source: &str) -> Vec<Planned> {
        let selection = self.selection(&symbol.def.file, site);
        let equivalent = equivalent::reason(source, site.operator, &site.span);
        replacements(source, site)
            .into_iter()
            .map(|(to, mutated)| {
                let key = selection.covered.then(|| {
                    digest(&[
                        &symbol.source_hash,
                        site.site_hash.as_str(),
                        &to,
                        &selection.suite_hash,
                        self.inputs.toolchain_hash,
                    ])
                });
                Planned {
                    mutant: Mutant {
                        symbol: symbol.def.key(),
                        symbol_hash: symbol.hash.clone(),
                        site_hash: site.site_hash.clone(),
                        operator: site.operator,
                        from: site.from.clone(),
                        to,
                        span: site.span,
                        outcome: MutantOutcome::Uncovered,
                        tests: Vec::new(),
                        killing_tests: Vec::new(),
                        duration_ms: None,
                        reused: false,
                        equivalent: equivalent.clone(),
                        cache_key: key,
                        strategy: None,
                        fallback: None,
                    },
                    source: mutated,
                    tests: selection.tests.clone(),
                    covered: selection.covered,
                }
            })
            .collect()
    }

    /// How `site` of `file` is run: covered when the coverage map records a hit on one of its
    /// lines; its tests those the per-test map says run its lines (the whole suite when there
    /// is no map, or the map names none).
    fn selection(&self, file: &str, site: &Site) -> Selection {
        let (line, end) = (site.span.line, site.span.end_line);
        let covered = self
            .inputs
            .coverage
            .line_hits(file, line, end)
            .iter()
            .any(|hits| hits.hits > 0);
        let tests = self.index.as_ref().and_then(|index| {
            let mut ids: Vec<String> = index
                .covering(file, line, end)
                .into_iter()
                .map(|(test, _)| test.id.clone())
                .collect();
            ids.sort();
            (!ids.is_empty()).then_some(ids)
        });
        let suite_hash = tests.as_ref().map_or_else(
            || self.inputs.suite_hash.to_owned(),
            |ids| tests_hash(ids, self.inputs.per_test, self.read),
        );
        Selection {
            covered,
            tests,
            suite_hash,
        }
    }
}

/// BLAKE3 of the symbol's source text: any edit to it reruns its mutants.
fn source_hash(source: &str, symbol: &SymbolDef) -> String {
    let lines: Vec<&str> = source
        .lines()
        .skip(symbol.span.line.saturating_sub(1) as usize)
        .take((symbol.span.end_line + 1).saturating_sub(symbol.span.line) as usize)
        .collect();
    digest(&[&lines.join("\n")])
}

/// The suite hash of a selection of `ids`: the IDs and the content of every test file the
/// per-test map names for them.
fn tests_hash(
    ids: &[String],
    map: Option<&PerTestMap>,
    read: &dyn Fn(&str) -> Option<String>,
) -> String {
    let mut files: Vec<&str> = map
        .into_iter()
        .flat_map(|map| &map.tests)
        .filter(|test| ids.binary_search(&test.id).is_ok())
        .filter_map(|test| test.file.as_deref())
        .collect();
    files.sort_unstable();
    files.dedup();
    let contents: Vec<String> = files
        .iter()
        .map(|file| format!("{file}\n{}", read(file).unwrap_or_default()))
        .collect();
    let mut parts: Vec<&str> = ids.iter().map(String::as_str).collect();
    parts.extend(contents.iter().map(String::as_str));
    digest(&parts)
}

/// BLAKE3 of `parts`, NUL-separated, as hex.
pub fn digest(parts: &[&str]) -> String {
    let mut hasher = blake3::Hasher::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update(&[0]);
    }
    hasher.finalize().to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use cupel_core::coverage::{FileCoverage, LineHits};
    use cupel_core::syntax::{MutationOperator, Site, Span, SymbolKind, SymbolSites, Visibility};
    use cupel_core::{Focus, LangId};

    use super::*;

    const SOURCE: &str = "fn f(a: i64) -> bool {\n    let ran = a > 1;\n    #[cfg(feature = \"off\")]\n    let off = a > 2;\n    ran\n}\n";

    fn symbol(file: &str) -> SymbolDef {
        SymbolDef {
            name: "f".to_owned(),
            namespace: String::new(),
            display: "f".to_owned(),
            kind: SymbolKind::Function,
            visibility: Visibility::Private,
            lang: LangId::from("rust"),
            file: file.to_owned(),
            span: Span {
                line: 1,
                col: 1,
                end_line: 6,
                end_col: 2,
            },
            body: None,
            parameters: 1,
        }
    }

    /// The `>` of `a > 1` (line 2) or `a > 2` (line 4).
    fn site(line: u32) -> Site {
        Site {
            operator: MutationOperator::Relational,
            set: OperatorSet::Default,
            from: ">".to_owned(),
            to: vec![">=".to_owned()],
            site_hash: format!("{line:016x}").parse().unwrap(),
            span: Span {
                line,
                col: 17,
                end_line: line,
                end_col: 18,
            },
        }
    }

    /// The lines of `file`'s mutants with `coverage`: line 2 instrumented and never run.
    fn planned_lines(file: &str, coverage: &CoverageMap) -> Vec<(u32, bool)> {
        let symbols = Symbols {
            symbols: vec![symbol(file)],
        };
        let sites = MutationSites {
            symbols: vec![SymbolSites {
                symbol: symbol(file).key(),
                sites: vec![site(2), site(4)],
            }],
        };
        let inputs = Inputs {
            sites: &sites,
            symbols: &symbols,
            coverage,
            norm: None,
            conventions: None,
            frames: None,
            per_test: None,
            focus: &Focus::default(),
            scope: &Scope::of_files(vec![file.to_owned()]),
            operators: OperatorSet::Default,
            suite_hash: "suite",
            toolchain_hash: "toolchain",
        };
        plan(&inputs, &|_| Some(SOURCE.to_owned()))
            .iter()
            .map(|planned| (planned.mutant.span.line, planned.covered))
            .collect()
    }

    #[test]
    fn a_site_on_a_line_the_coverage_map_does_not_instrument_is_not_a_mutant() {
        // The coverage tool instruments the file's executable lines; line 4 is not one in this
        // build (a `cfg` left it out), so no test can reach its mutant.
        let coverage = CoverageMap::new(vec![FileCoverage {
            file: "src/lib.rs".to_owned(),
            lines: vec![
                LineHits { line: 1, hits: 1 },
                LineHits { line: 2, hits: 0 },
                LineHits { line: 5, hits: 1 },
            ],
            branches: None,
        }]);
        assert_eq!(planned_lines("src/lib.rs", &coverage), [(2, false)]);
    }

    #[test]
    fn every_site_of_a_file_the_coverage_map_does_not_hold_is_an_uncovered_mutant() {
        // A file no test loads is absent from some tools' reports (V8, coverage.py): its
        // mutants are uncovered, not left out.
        let coverage = CoverageMap::new(Vec::new());
        assert_eq!(
            planned_lines("src/lib.rs", &coverage),
            [(2, false), (4, false)]
        );
    }
}
