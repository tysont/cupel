// ABOUTME: The Rust joint producer (ADR-084): one `cargo llvm-cov nextest` run whose JUnit report is the baseline, whose
// ABOUTME: per-process profiles are the per-test map (ADR-083), and whose profiles together are `cargo llvm-cov`'s report.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use cupel_core::{FactKind, SuitePhase};

use super::{Rerun, Suite, SuiteRun, SuiteSpec, probe};
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
    warm: Some(warm),
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
/// them; every test but the kept ones (ADR-149 §1), through nextest's filterset, in only the
/// binaries that can hold a test it runs (ADR-155 §5).
fn run(site: &Site<'_>, rerun: &Rerun) -> cupel_core::Result<SuiteRun> {
    let kept = &rerun.kept;
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
        let not_kept = format!("not ({})", filterset(&ids));
        let expression = match listed(&site.path, rerun) {
            Some(binaries) => format!("({binaries}) & {not_kept}"),
            None => not_kept,
        };
        extra.extend(["-E".to_owned(), expression]);
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

/// The binaries an incremental run must list, as a filterset of nextest's binary-level
/// predicates, which spare every other binary its listing (ADR-155 §5): each stale test's
/// binary, and the targets each changed Rust file under `root` is compiled into. `None`, every
/// binary, when a changed Rust file lies in no package.
fn listed(root: &Path, rerun: &Rerun) -> Option<String> {
    let mut binaries: BTreeSet<String> = rerun
        .stale
        .iter()
        .map(|id| {
            let binary = id.split_once(' ').map_or(id.as_str(), |(binary, _)| binary);
            format!("binary_id(={binary})")
        })
        .collect();
    for file in rerun.changed.iter().filter(|file| file.ends_with(".rs")) {
        binaries.insert(targets_of(root, file)?);
    }
    (!binaries.is_empty()).then(|| binaries.into_iter().collect::<Vec<_>>().join(" | "))
}

/// The targets the Rust file `file` (relative to `root`) is compiled into, as a filterset, by
/// the nearest directory above it whose `Cargo.toml` names a package: its library and binaries
/// for a file below `src/`, its integration tests below `tests/`, else any of its targets.
fn targets_of(root: &Path, file: &str) -> Option<String> {
    let path = root.join(file);
    let (dir, name) = path
        .ancestors()
        .skip(1)
        .take_while(|dir| dir.starts_with(root))
        .find_map(|dir| Some((dir.to_path_buf(), package_name(dir)?)))?;
    let inside: PathBuf = path.strip_prefix(&dir).ok()?.to_path_buf();
    let package = format!("package(={name})");
    Some(match inside.components().next()?.as_os_str().to_str()? {
        "src" => format!("({package} & (kind(lib) | kind(bin) | kind(proc-macro)))"),
        "tests" => format!("({package} & kind(test))"),
        _ => package,
    })
}

/// The `[package]` name of the manifest in `dir`, when it has one.
fn package_name(dir: &Path) -> Option<String> {
    let manifest: toml::Table = std::fs::read_to_string(dir.join("Cargo.toml"))
        .ok()?
        .parse()
        .ok()?;
    Some(manifest.get("package")?.get("name")?.as_str()?.to_owned())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{Rerun, listed};

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    /// A workspace whose root manifest names no package, with the packages `calc` (in `calc/`)
    /// and `calc-cli` (in `cli/`).
    fn workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"calc\", \"cli\"]\n",
        )
        .unwrap();
        for (path, name) in [("calc", "calc"), ("cli", "calc-cli")] {
            std::fs::create_dir_all(root.join(path).join("src")).unwrap();
            std::fs::write(
                root.join(path).join("Cargo.toml"),
                format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n"),
            )
            .unwrap();
        }
        dir
    }

    #[test]
    fn a_run_lists_the_stale_tests_binaries_and_the_targets_of_each_changed_rust_file() {
        let dir = workspace();
        let rerun = Rerun {
            kept: set(&["calc tests::adds"]),
            stale: set(&[
                "calc tests::scales",
                "calc-cli::cli runs",
                "calc-cli::cli prints",
            ]),
            changed: set(&[
                "calc/src/lib.rs",
                "cli/tests/cli.rs",
                "cli/benches/b.rs",
                "README.md",
            ]),
        };
        assert_eq!(
            listed(dir.path(), &rerun),
            Some(
                [
                    "(package(=calc) & (kind(lib) | kind(bin) | kind(proc-macro)))",
                    "(package(=calc-cli) & kind(test))",
                    "binary_id(=calc)",
                    "binary_id(=calc-cli::cli)",
                    "package(=calc-cli)",
                ]
                .join(" | ")
            )
        );
    }

    #[test]
    fn a_changed_rust_file_in_no_package_lists_every_binary() {
        let dir = workspace();
        let rerun = Rerun {
            kept: set(&["calc tests::adds"]),
            stale: set(&["calc tests::scales"]),
            changed: set(&["tools/gen.rs"]),
        };
        assert_eq!(listed(dir.path(), &rerun), None);
    }
}
