// ABOUTME: The Python `PerTestMap` producer (ADR-077): one pytest run with pytest-cov's test contexts, its data and
// ABOUTME: JSON report under `.cupel/`; each executed line's contexts name the tests that ran it.
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use cupel_core::{LangId, Langs, PerTestMap, Result, TestCoverage};

use super::cache::own_file;
use super::{Measured, PerTest, PerTestSpec, Runs, Site, covered, failed};
use crate::report::listing;

const ID: &str = "tool.pytest-cov-per-test";

static PYTEST_COV: PerTestSpec = PerTestSpec {
    id: ID,
    languages: || Langs::Only(vec![LangId::from("python")]),
    runner: "tool.pytest",
    programs: &["pytest"],
    version: &["pytest", "--version"],
    runs: Runs::Suite,
    outputs: &["__pycache__"],
    whole: &[
        "conftest.py",
        "pyproject.toml",
        "setup.cfg",
        "setup.py",
        "pytest.ini",
        "tox.ini",
    ],
    sources: &["py"],
    owns: |_: &Path, test, file| own_file(test, file),
    measure,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<PerTestMap, _>(PerTest(&PYTEST_COV)),
});

/// Arguments that keep pytest from writing into the tree.
const QUIET: &[&str] = &["-p", "no:cacheprovider"];
const ENV: &[(&str, &str)] = &[("PYTHONDONTWRITEBYTECODE", "1")];

/// One run over every test but `keep`: all of them when `keep` is empty, else the listed tests
/// not in it (none, without running, when that is every listed test).
fn measure(site: &Site<'_>, keep: &BTreeSet<String>) -> Result<Measured> {
    if keep.is_empty() {
        return Ok(Measured {
            tests: run(site, &[])?,
            listed: None,
        });
    }
    let mut list: Vec<String> = QUIET.iter().map(|arg| (*arg).to_owned()).collect();
    list.extend(["--collect-only".to_owned(), "-q".to_owned()]);
    let listed: BTreeSet<String> = listing::pytest(&site.succeed("pytest", &list, ENV)?.stdout)
        .into_iter()
        .collect();
    let selected: Vec<String> = listed.difference(keep).cloned().collect();
    let tests = if selected.is_empty() {
        Vec::new()
    } else {
        run(site, &selected)?
    };
    Ok(Measured {
        tests,
        listed: Some(listed),
    })
}

/// One pytest run with test contexts over `selected` node IDs (every test when empty).
fn run(site: &Site<'_>, selected: &[String]) -> Result<Vec<TestCoverage>> {
    let config = site.report("coveragerc");
    let report = site.report("coverage.json");
    let rc = format!(
        "[run]\ndata_file = {}\nrelative_files = True\n\n[json]\nshow_contexts = True\n",
        site.report(".coverage").display()
    );
    site.ctx.writer.write(&config, rc.as_bytes())?;
    let _ = std::fs::remove_file(&report);
    let mut args: Vec<String> = QUIET.iter().map(|arg| (*arg).to_owned()).collect();
    args.extend([
        "--cov=.".to_owned(),
        "--cov-context=test".to_owned(),
        format!("--cov-config={}", config.display()),
        format!("--cov-report=json:{}", report.display()),
    ]);
    args.extend(selected.iter().cloned());
    // Failing tests still measure what they ran; only a run that wrote no report failed.
    let captured = site.run("pytest", &args, ENV)?;
    let Ok(text) = std::fs::read_to_string(&report) else {
        return Err(failed("pytest", &captured));
    };
    Ok(tests_of(&text)
        .into_iter()
        .map(|(id, lines)| {
            let file = id.split("::").next().map(str::to_owned);
            TestCoverage::new(&id, file, covered(lines, |path| site.file(path)))
        })
        .collect())
}

/// The executed lines per test of a coverage.py JSON report with `show_contexts`: each line's
/// contexts are `<node id>|<phase>`, and the empty context (code run outside any test) is
/// dropped.
pub fn tests_of(text: &str) -> BTreeMap<String, BTreeMap<String, Vec<u32>>> {
    let mut by_test: BTreeMap<String, BTreeMap<String, Vec<u32>>> = BTreeMap::new();
    let Ok(report) = serde_json::from_str::<serde_json::Value>(text) else {
        return by_test;
    };
    for (file, data) in report["files"].as_object().into_iter().flatten() {
        for (line, contexts) in data["contexts"].as_object().into_iter().flatten() {
            let Ok(line) = line.parse::<u32>() else {
                continue;
            };
            for context in contexts
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c.as_str())
            {
                let node = context.rsplit_once('|').map_or(context, |(node, _)| node);
                if !node.is_empty() {
                    by_test
                        .entry(node.to_owned())
                        .or_default()
                        .entry(file.clone())
                        .or_default()
                        .push(line);
                }
            }
        }
    }
    by_test
}

#[cfg(test)]
mod tests {
    use cupel_fixtures::Capture;

    use super::tests_of;

    /// A file and the lines a test ran in it.
    type FileLines = (String, Vec<u32>);

    #[test]
    fn each_tests_context_names_the_lines_it_ran() {
        let report = Capture::of("per-test/pytest-cov", "contexts").file("coverage.json");
        let tests: Vec<(String, Vec<FileLines>)> = tests_of(&report)
            .into_iter()
            .map(|(test, files)| (test, files.into_iter().collect()))
            .collect();
        let calc = |line: u32, test_line: u32| {
            vec![
                ("calc.py".to_owned(), vec![line]),
                ("tests/test_calc.py".to_owned(), vec![test_line]),
            ]
        };
        assert_eq!(
            tests,
            [
                ("tests/test_calc.py::test_add".to_owned(), calc(6, 6)),
                ("tests/test_calc.py::test_scale".to_owned(), calc(11, 10)),
            ]
        );
    }
}
