// ABOUTME: The Rust `PerTestMap` producer (ADR-077): lists tests with `cargo nextest`, builds instrumented once, then
// ABOUTME: runs each test alone under `cargo llvm-cov nextest` and reads the executed lines of its LCOV report.
use cupel_core::{LangId, Langs, PerTestMap, Result, TestCoverage};

use super::{PerTest, PerTestSpec, Site, covered, executed_lines};
use crate::cargo_nextest::filterset;
use crate::coverage::{parse_lcov, placed};
use crate::report::listing;

const ID: &str = "tool.llvm-cov-per-test";

static LLVM_COV: PerTestSpec = PerTestSpec {
    id: ID,
    languages: || Langs::Only(vec![LangId::from("rust")]),
    runner: "tool.cargo-nextest",
    programs: &["cargo-llvm-cov", "cargo-nextest"],
    version: &["cargo", "llvm-cov", "--version"],
    measure,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<PerTestMap, _>(PerTest(&LLVM_COV)),
});

const ENV: &[(&str, &str)] = &[("CARGO_TERM_COLOR", "never")];

fn words(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

fn measure(site: &Site<'_>) -> Result<Vec<TestCoverage>> {
    let listed = site.succeed(
        "cargo",
        &words(&["nextest", "list", "--workspace", "--message-format", "json"]),
        ENV,
    )?;
    site.succeed("cargo", &words(&["llvm-cov", "clean", "--workspace"]), ENV)?;
    let mut tests = Vec::new();
    for (index, id) in listing::nextest(&listed.stdout).into_iter().enumerate() {
        site.succeed(
            "cargo",
            &words(&["llvm-cov", "clean", "--profraw-only"]),
            ENV,
        )?;
        let report = site.report(&format!("{index}.lcov"));
        let _ = std::fs::remove_file(&report);
        let mut args = words(&[
            "llvm-cov",
            "nextest",
            "--no-clean",
            "--lcov",
            "--output-path",
        ]);
        args.push(report.to_string_lossy().into_owned());
        args.extend(words(&["--workspace", "--no-fail-fast", "-E"]));
        args.push(filterset(std::slice::from_ref(&id)));
        // A failing test still measures what it ran; a run that wrote no report measured nothing.
        site.run("cargo", &args, ENV)?;
        let lines = std::fs::read_to_string(&report)
            .ok()
            .and_then(|text| parse_lcov(&text).ok())
            .map(|map| executed_lines(&placed(map, &[&site.path], ".")))
            .unwrap_or_default();
        tests.push(TestCoverage::new(
            &id,
            None,
            covered(lines, |file| Some(file.to_owned())),
        ));
    }
    Ok(tests)
}
