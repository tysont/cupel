// ABOUTME: The Rust joint producer (ADR-084): one `cargo llvm-cov nextest` run whose JUnit report is the baseline, whose
// ABOUTME: per-process profiles are the per-test map (ADR-083), and whose profiles together are `cargo llvm-cov`'s report.
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Instant;

use cupel_core::{FactKind, SuitePhase};

use super::{Suite, SuiteRun, SuiteSpec, probe};
use crate::cargo_nextest::{PROFILE, filterset};
use crate::coverage::{parse_lcov, placed};
use crate::per_test::Site;
use crate::per_test::llvm_cov::{Instrumented, LLVM_COV, elapsed_ms, warm};
use crate::report::junit;
use crate::runner::finished;

const ID: &str = "tool.llvm-cov-nextest";

static SPEC: SuiteSpec = SuiteSpec {
    id: ID,
    lang: "rust",
    makes: &[
        (FactKind::TestResults, "tool.cargo-nextest"),
        (FactKind::CoverageMap, "tool.cargo-llvm-cov"),
        (FactKind::PerTestMap, "tool.llvm-cov-per-test"),
    ],
    per_test: Some(&LLVM_COV),
    programs: &["cargo-llvm-cov", "cargo-nextest"],
    outputs: &["target"],
    fuses: |words| words == ["cargo", "llvm-cov", "--lcov", "--output-path", "{out}"],
    toolchain,
    run,
    warm,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register_joint(Suite(&SPEC)),
});

/// The root's compiler as `rustc -vV` reports it: its release and host (ADR-149 §4).
fn toolchain(root: &Path) -> Option<String> {
    let version = probe(root, "rustc", &["-vV"])?;
    let field = |name: &str| {
        version
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .map(str::trim)
    };
    Some(format!(
        "rustc {} {}",
        field("release: ")?,
        field("host: ")?
    ))
}

/// The suite under `cargo llvm-cov nextest` with nextest's `cupel` `JUnit` profile, every test
/// process's profiles kept apart (ADR-083), then the per-test map and the whole report from
/// them; every test but `kept` (ADR-149 §1), through nextest's filterset.
fn run(site: &Site<'_>, kept: &BTreeSet<String>) -> cupel_core::Result<SuiteRun> {
    let config = site.report("nextest.toml");
    site.ctx.writer.write(&config, PROFILE.as_bytes())?;
    let junit = site.path.join("target/nextest/cupel/junit.xml");
    let _ = std::fs::remove_file(&junit);
    let mut extra = vec![
        "--profile".to_owned(),
        "cupel".to_owned(),
        "--tool-config-file".to_owned(),
        format!("cupel:{}", config.display()),
    ];
    if !kept.is_empty() {
        let ids: Vec<String> = kept.iter().cloned().collect();
        extra.extend(["-E".to_owned(), format!("not ({})", filterset(&ids))]);
    }
    let instrumented = Instrumented::run(site, &extra)?;
    let found = std::fs::read_to_string(&junit)
        .ok()
        .and_then(|report| junit::nextest(&report));
    let invocation = finished(
        site.dir,
        instrumented.command.clone(),
        &instrumented.captured,
        found.as_deref(),
        &[0],
    );
    let mut run = SuiteRun {
        invocation,
        tests: found.unwrap_or_default(),
        coverage: None,
        per_test: None,
        phases: instrumented.phases.clone(),
    };
    if instrumented.ran() {
        let started = Instant::now();
        run.per_test = Some(instrumented.export(site)?);
        run.phases.push((SuitePhase::Export, elapsed_ms(started)));
        let started = Instant::now();
        let report = instrumented.report(site)?;
        run.phases.push((SuitePhase::Report, elapsed_ms(started)));
        run.coverage = parse_lcov(&report)
            .ok()
            .map(|map| placed(map, &[&site.path], "."));
    }
    instrumented.finish();
    Ok(run)
}
