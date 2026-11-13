// ABOUTME: The Go `PerTestMap` producer (ADR-077): lists tests with `go test -list`, then runs each alone with
// ABOUTME: `-coverprofile` over every package of the root's modules, mapping import paths to files via `go.mod`.
use std::collections::BTreeSet;
use std::path::Path;

use cupel_core::{LangId, Langs, PerTestMap, Result, TestCoverage};

use super::{Measured, PerTest, PerTestSpec, Runs, Site, covered, executed_lines};
use crate::coverage::parse_coverprofile;
use crate::go_module::Modules;
use crate::go_test::packages;
use crate::report::listing;

const ID: &str = "tool.go-cover-per-test";

static GO_COVER: PerTestSpec = PerTestSpec {
    id: ID,
    languages: || Langs::Only(vec![LangId::from("go")]),
    runner: "tool.go-test",
    programs: &["go"],
    version: &["go", "version"],
    runs: Runs::PerTest,
    outputs: &[],
    whole: &["go.mod", "go.sum", "go.work"],
    sources: &["go"],
    owns,
    measure,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<PerTestMap, _>(PerTest(&GO_COVER)),
});

/// Each listed test but those in `keep`, run alone, measuring every package of the root's
/// modules: a test of one package records the lines it runs in any other (ADR-202).
fn measure(site: &Site<'_>, keep: &BTreeSet<String>) -> Result<Measured> {
    let patterns = packages(&site.packages);
    let mut list = vec![
        "test".to_owned(),
        "-list".to_owned(),
        "^Test".to_owned(),
        "-json".to_owned(),
    ];
    list.extend(patterns.iter().cloned());
    let listed = site.succeed("go", &list, &[])?;
    let modules = Modules::of(&site.path);
    let listed: BTreeSet<String> = listing::go(&listed.stdout).into_iter().collect();
    let mut tests = Vec::new();
    for (index, id) in listed.difference(keep).enumerate() {
        let Some((package, name)) = id.rsplit_once('/') else {
            continue;
        };
        let report = site.report(&format!("{index}.out"));
        let _ = std::fs::remove_file(&report);
        let args = vec![
            "test".to_owned(),
            format!("-coverprofile={}", report.display()),
            format!("-coverpkg={}", patterns.join(",")),
            "-run".to_owned(),
            format!("^({name})$"),
            package.to_owned(),
        ];
        // A failing test still measures what it ran; a run that wrote no profile measured nothing.
        site.run("go", &args, &[])?;
        let lines = std::fs::read_to_string(&report)
            .ok()
            .and_then(|text| parse_coverprofile(&text).ok())
            .map(|map| executed_lines(&map))
            .unwrap_or_default();
        let file = |path: &str| modules.file(path);
        tests.push(TestCoverage::new(id, None, covered(lines, file)));
    }
    Ok(Measured {
        tests,
        listed: Some(listed),
    })
}

/// Whether `file` is one of `test`'s own: a `_test.go` file of its package. A Go profile never
/// covers test files, so a test owns every test file of its package.
fn owns(root: &Path, test: &TestCoverage, file: &str) -> bool {
    let Some(stem) = file.strip_suffix("_test.go") else {
        return false;
    };
    let Some((package, _)) = test.id.rsplit_once('/') else {
        return false;
    };
    let dir = stem.rsplit_once('/').map_or("", |(dir, _)| dir);
    Modules::of(root).package(dir).as_deref() == Some(package)
}
