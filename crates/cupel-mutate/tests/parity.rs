// ABOUTME: Schemata parity on the Rust fixtures (design §11, ADR-082): the engine's outcomes as schemata equal its
// ABOUTME: outcomes as file copies, and a file that defeats the rewrite falls back to file copies with the reason.
mod support;

use std::fs;
use std::path::{Path, PathBuf};

use cupel_core::{MutantOutcome, MutationOutcomes, MutationStrategy};
use support::{Run, fixture, produce, summary};

/// Whether cargo is here; prints the skip line when it is not.
fn cargo() -> bool {
    support::installed("cargo", "the Rust schemata parity test")
}

/// A `--config` file in `dir`: the fixture's `cupel.toml` at `root` with `[lang.rust] mutation`
/// set to `strategy` (the fixture's own file stays as its committed coverage report expects).
fn config(root: &Path, dir: &Path, strategy: &str) -> PathBuf {
    let toml = fs::read_to_string(root.join("cupel.toml")).unwrap();
    let path = dir.join(format!("{strategy}.toml"));
    fs::write(
        &path,
        format!("{toml}mutation = {{ strategy = \"{strategy}\" }}\n"),
    )
    .unwrap();
    path
}

/// The outcomes in a copy of `repos/<name>`, changed by `change`, under `strategy`.
fn outcomes(name: &str, change: fn(&Path), strategy: &str) -> MutationOutcomes {
    let dir = fixture(name);
    change(dir.path());
    let configs = tempfile::tempdir().unwrap();
    let run = Run {
        config: Some(config(dir.path(), configs.path(), strategy)),
        ..Run::default()
    };
    produce(dir.path(), "rust", &run).unwrap()
}

/// Leaves a fixture as it is.
fn unchanged(_: &Path) {}

/// Appends `code` to the fixture's `src/lib.rs`.
fn append(root: &Path, code: &str) {
    let lib = root.join("src/lib.rs");
    let text = fs::read_to_string(&lib).unwrap();
    fs::write(&lib, format!("{text}{code}")).unwrap();
}

/// What a mutant's outcome is, independent of how it was built.
fn outcome_set(outcomes: &MutationOutcomes) -> Vec<String> {
    outcomes
        .mutants
        .iter()
        .map(|m| {
            format!(
                "{}:{}:{} {}→{} {} tests={:?} killing={:?} equivalent={}",
                m.symbol.file,
                m.span.line,
                m.span.col,
                m.from,
                m.to,
                m.outcome,
                m.tests,
                m.killing_tests,
                m.equivalent.is_some()
            )
        })
        .collect()
}

/// Each run mutant's strategy and fallback.
fn strategies(outcomes: &MutationOutcomes) -> Vec<(Option<MutationStrategy>, Option<&str>)> {
    outcomes
        .mutants
        .iter()
        .filter(|m| m.outcome != MutantOutcome::Uncovered)
        .map(|m| (m.strategy, m.fallback.as_deref()))
        .collect()
}

/// Asserts the outcomes of `name` agree under both strategies and that every run mutant was
/// built the way its strategy says; the schemata outcomes.
fn parity(name: &str, change: fn(&Path)) -> MutationOutcomes {
    let schemata = outcomes(name, change, "schemata");
    let copies = outcomes(name, change, "file-copy");
    assert_eq!(outcome_set(&schemata), outcome_set(&copies), "{name}");
    assert!(
        strategies(&copies)
            .iter()
            .all(|built| *built == (Some(MutationStrategy::FileCopy), None))
    );
    schemata
}

#[test]
fn every_rust_fixture_with_a_suite_has_the_same_outcomes_as_schemata_and_as_file_copies() {
    if !cargo() {
        return;
    }
    let mutation = parity("tests.mutation/rust", unchanged);
    let coverage = parity("coverage/rust", unchanged);
    assert_eq!(
        summary(&mutation),
        [
            "5 >=→> survived",
            "5 &&→|| killed",
            "6 -→+ killed",
            "8 *→/ survived (equivalent?)",
            "8 1→0 killed",
            "13 -→+ uncovered",
        ]
    );
    assert_eq!(
        summary(&coverage),
        [
            "5 0→1 killed",
            "7 +=→-= killed",
            "14 >=→> survived",
            "17 >=→> survived",
            "25 <=→< uncovered",
            "25 0→1 uncovered",
            "26 0→1 uncovered",
            "28 >→>= uncovered",
            "29 0→1 uncovered",
            "31 &&→|| uncovered",
            "31 <→<= uncovered",
            "35 /→* uncovered",
            "37 /→* uncovered",
        ]
    );
    assert_eq!(
        (strategies(&mutation), strategies(&coverage)),
        (
            vec![(Some(MutationStrategy::Schemata), None); 5],
            vec![(Some(MutationStrategy::Schemata), None); 4]
        )
    );
}

/// Whether the fixture's coverage can be measured again after a change; prints the skip line
/// when it cannot.
fn coverage_tools() -> bool {
    cargo() && support::installed("cargo-llvm-cov", "the Rust schemata fallback test")
}

/// A greeting `String + &str`, whose `+` → `-` no `String` supports, and a test that covers it.
fn greeting(root: &Path) {
    append(
        root,
        "\n/// A greeting.\npub fn greet(name: &str) -> String {\n    String::from(\"hi \") + name\n}\n",
    );
    fs::write(
        root.join("tests/greet.rs"),
        "#[test]\nfn greets() {\n    assert_eq!(discount::greet(\"al\"), \"hi al\");\n}\n",
    )
    .unwrap();
}

#[test]
fn a_mutant_that_does_not_type_check_leaves_the_schemata_and_runs_as_a_file_copy() {
    if !coverage_tools() {
        return;
    }
    let schemata = parity("tests.mutation/rust", greeting);
    let greet: Vec<String> = schemata
        .mutants
        .iter()
        .filter(|m| m.span.line == 24)
        .map(|m| {
            format!(
                "{}→{} {} {:?} {:?}",
                m.from, m.to, m.outcome, m.strategy, m.fallback
            )
        })
        .collect();
    assert_eq!(
        greet,
        ["+→- unviable Some(FileCopy) Some(\"did not compile under schemata (line 24)\")"]
    );
    let others = schemata
        .mutants
        .iter()
        .filter(|m| m.span.line != 24 && m.outcome != MutantOutcome::Uncovered);
    assert!(others.clone().count() == 5);
    assert!(
        others
            .into_iter()
            .all(|m| m.strategy == Some(MutationStrategy::Schemata))
    );
}

/// A function of the reader's name, so no schemata build of the file can compile.
fn taken_name(root: &Path) {
    append(
        root,
        "\n#[allow(dead_code)]\nfn cupel_mutant_id() -> u32 {\n    7\n}\n",
    );
}

#[test]
fn a_schemata_build_that_cannot_compile_runs_the_whole_file_as_file_copies() {
    if !coverage_tools() {
        return;
    }
    let schemata = outcomes("tests.mutation/rust", taken_name, "schemata");
    assert_eq!(
        summary(&schemata),
        [
            "5 >=→> survived",
            "5 &&→|| killed",
            "6 -→+ killed",
            "8 *→/ survived (equivalent?)",
            "8 1→0 killed",
            "13 -→+ uncovered",
        ]
    );
    let reason = "the schemata build of `src/lib.rs` failed: error[E0428]: the name \
                  `cupel_mutant_id` is defined multiple times";
    assert_eq!(
        strategies(&schemata),
        [(Some(MutationStrategy::FileCopy), Some(reason)); 5]
    );
}
