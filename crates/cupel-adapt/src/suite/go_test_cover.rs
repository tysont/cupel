// ABOUTME: The Go joint producer (ADR-084): one `go test -json -coverprofile` run whose events are the baseline and whose
// ABOUTME: profile is the coverage; Go's per-test map stays one run per test, refreshed by its own producer.
use cupel_core::{FactKind, Result};

use super::{Suite, SuiteRun, SuiteSpec};
use crate::coverage::{module_paths, parse_coverprofile, placed};
use crate::go_test::packages;
use crate::per_test::Site;
use crate::report::test2json;
use crate::runner::finished;

const ID: &str = "tool.go-test-cover";

static SPEC: SuiteSpec = SuiteSpec {
    id: ID,
    lang: "go",
    makes: &[
        (FactKind::TestResults, "tool.go-test"),
        (FactKind::CoverageMap, "tool.go-coverprofile"),
    ],
    per_test: None,
    programs: &["go"],
    outputs: &[],
    fuses,
    run,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register_joint(Suite(&SPEC)),
});

/// The toolchain table's coverprofile command, `go test -coverprofile={out} <packages>`, with
/// package patterns and no other flag.
fn fuses(words: &[&str]) -> bool {
    words.len() > 3
        && words[..3] == ["go", "test", "-coverprofile={out}"]
        && words[3..].iter().all(|word| !word.starts_with('-'))
}

/// One `go test -json -coverprofile` run over the root's packages.
fn run(site: &Site<'_>) -> Result<SuiteRun> {
    let profile = site.report("cover.out");
    let _ = std::fs::remove_file(&profile);
    let args: Vec<String> = ["test".to_owned(), "-json".to_owned()]
        .into_iter()
        .chain([format!("-coverprofile={}", profile.display())])
        .chain(packages(&site.packages))
        .collect();
    let captured = site.run("go", &args, &[])?;
    let found = test2json::parse(&captured.stdout, &captured.stderr);
    let command = std::iter::once("go".to_owned()).chain(args).collect();
    let invocation = finished(site.dir, command, &captured, Some(&found), &[0]);
    let coverage = std::fs::read_to_string(&profile)
        .ok()
        .and_then(|text| parse_coverprofile(&text).ok())
        .map(|map| placed(module_paths(map, &site.path), &[&site.path], "."));
    Ok(SuiteRun {
        invocation,
        tests: found,
        coverage,
        per_test: None,
    })
}

#[cfg(test)]
mod tests {
    use super::fuses;

    #[test]
    fn the_default_command_with_package_patterns_is_fused_and_any_flag_is_not() {
        let fused = |command: &str| fuses(&command.split_whitespace().collect::<Vec<_>>());
        assert_eq!(
            [
                "go test -coverprofile={out} ./...",
                "go test -coverprofile={out} ./cmd/... ./pkg/...",
                "go test -coverprofile={out} -coverpkg=./... ./...",
                "go test -coverprofile={out}",
            ]
            .map(fused),
            [true, true, false, false]
        );
    }
}
