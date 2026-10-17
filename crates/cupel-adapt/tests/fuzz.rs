// ABOUTME: The fuzz producers (ADR-107) on the real fuzzers: each language's gen.fuzz fixture crashes on the empty input
// ABOUTME: and passes its other target, inputs and copies stay under the cache, a hang times out, and a missing tool says so.
mod support;

use std::fs;
use std::path::Path;

use cupel_core::{
    Availability, Cancellation, FactKind, FuzzResults, FuzzStatus, Scope, find_program,
};

/// Whether every program in `programs` is installed; says which is not.
fn installed(tool: &str, programs: &[&str]) -> bool {
    match programs
        .iter()
        .find(|program| find_program(program).is_none())
    {
        Some(program) => {
            eprintln!("SKIPPED: {program} not installed; {tool} was not run");
            false
        }
        None => true,
    }
}

/// What the producer `id` makes of the repository at `root` for `lang`, with `cases`.
fn fuzz(id: &str, lang: &str, root: &Path, cases: u64) -> FuzzResults {
    let producer = support::producer(support::shared_registry(), FactKind::FuzzResults, id, lang);
    let mut ctx = support::produce_ctx(root, lang, Cancellation::new());
    ctx.seed = 3;
    ctx.focus.cases = Some(cases);
    assert_eq!(producer.availability(&ctx.ctx), Availability::Ready);
    producer
        .produce(&ctx, &Scope::default(), &mut Vec::new())
        .unwrap()
        .downcast::<FuzzResults>()
        .unwrap()
}

/// A crash as (message, file and line, input).
type Crash = (String, Option<(String, u32)>, String);

/// Each target as (name, status, crash).
fn summary(results: &FuzzResults) -> Vec<(String, FuzzStatus, Option<Crash>)> {
    results
        .targets
        .iter()
        .map(|target| {
            let crash = target.crash.as_ref().map(|crash| {
                (
                    crash.message.clone(),
                    crash.location.as_ref().map(|at| (at.file.clone(), at.line)),
                    crash.input.clone(),
                )
            });
            (target.name.clone(), target.status, crash)
        })
        .collect()
}

/// Every file under `dir` outside `.cupel`, relative to it.
fn tree(dir: &Path) -> Vec<String> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == ".cupel" {
            continue;
        }
        if path.is_dir() {
            files.extend(tree(&path).into_iter().map(|file| format!("{name}/{file}")));
        } else {
            files.push(name);
        }
    }
    files.sort();
    files
}

#[test]
fn cargo_fuzz_crashes_first_on_the_empty_input_and_passes_checked_first() {
    if !installed("tool.cargo-fuzz", &["cargo-fuzz", "cargo"]) {
        return;
    }
    let copy = support::own_fixture("gen.fuzz", "rust");
    let before = tree(copy.path());
    let results = fuzz("tool.cargo-fuzz", "rust", copy.path(), 200);
    assert_eq!(
        (results.seeded, results.cases, results.roots.clone()),
        (true, 200, vec![".".to_owned()])
    );
    let crash = Some((
        "index out of bounds: the len is 0 but the index is 0".to_owned(),
        Some(("src/lib.rs".to_owned(), 5)),
        String::new(),
    ));
    assert_eq!(
        summary(&results),
        [
            ("first".to_owned(), FuzzStatus::Crashed, crash),
            ("checked_first".to_owned(), FuzzStatus::Passed, None),
        ]
    );
    let first = &results.targets[0];
    let reproducer = first
        .crash
        .as_ref()
        .and_then(|crash| crash.reproducer.clone())
        .unwrap();
    assert_eq!(
        reproducer,
        ".cupel/cache/fuzz/rust/root/first/inputs/crash-da39a3ee5e6b4b0d3255bfef95601890afd80709"
    );
    assert!(copy.path().join(&reproducer).is_file());
    assert_eq!(
        first
            .crash
            .as_ref()
            .and_then(|crash| crash.reproduce.clone())
            .as_deref(),
        Some(format!("cargo +nightly fuzz run first {reproducer}").as_str())
    );
    assert_eq!(results.targets[1].executions, Some(200));
    assert_eq!(
        tree(copy.path()),
        before,
        "the fuzz run wrote nothing in the tree"
    );
}

#[test]
fn a_target_that_hangs_on_an_input_times_out() {
    if !installed("tool.cargo-fuzz", &["cargo-fuzz", "cargo"]) {
        return;
    }
    let copy = support::own_fixture("gen.fuzz", "rust");
    let target = copy.path().join("fuzz/fuzz_targets/first.rs");
    let spinning = fs::read_to_string(&target).unwrap().replace(
        "let _ = units::first(data);",
        "while data.is_empty() { std::hint::spin_loop(); }",
    );
    fs::write(&target, spinning).unwrap();
    let results = fuzz("tool.cargo-fuzz", "rust", copy.path(), 10);
    let first = &results.targets[0];
    assert_eq!(first.status, FuzzStatus::TimedOut);
    let crash = first.crash.as_ref().unwrap();
    assert_eq!(
        crash.message,
        "timeout: an input ran past the per-input limit"
    );
    assert!(
        crash
            .reproducer
            .as_deref()
            .is_some_and(|path| path.contains("/inputs/timeout-")),
        "{crash:?}"
    );
}

#[test]
fn a_target_that_does_not_build_failed_with_the_build_output() {
    if !installed("tool.cargo-fuzz", &["cargo-fuzz", "cargo"]) {
        return;
    }
    let copy = support::own_fixture("gen.fuzz", "rust");
    fs::write(
        copy.path().join("fuzz/fuzz_targets/checked_first.rs"),
        "#![no_main]\nlibfuzzer_sys::fuzz_target!(|data: &[u8]| { missing(data); });\n",
    )
    .unwrap();
    let results = fuzz("tool.cargo-fuzz", "rust", copy.path(), 10);
    let broken = &results.targets[1];
    assert_eq!(
        (broken.status, broken.crash.is_none()),
        (FuzzStatus::Failed, true)
    );
    assert!(
        broken
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("missing")),
        "{broken:?}"
    );
}

#[test]
fn go_fuzzing_crashes_on_a_one_byte_string_in_a_copy_and_passes_even() {
    if !installed("tool.go-fuzz", &["go"]) {
        return;
    }
    let copy = support::own_fixture("gen.fuzz", "go");
    let before = tree(copy.path());
    let results = fuzz("tool.go-fuzz", "go", copy.path(), 500);
    assert!(!results.seeded);
    let crash = Some((
        "panic: runtime error: index out of range [1] with length 1".to_owned(),
        Some(("text/first.go".to_owned(), 7)),
        "0".to_owned(),
    ));
    assert_eq!(
        summary(&results),
        [
            (
                "example.com/units/text.FuzzSecondByte".to_owned(),
                FuzzStatus::Crashed,
                crash
            ),
            (
                "example.com/units/text.FuzzEven".to_owned(),
                FuzzStatus::Passed,
                None
            ),
        ]
    );
    let first = &results.targets[0];
    assert_eq!(
        first
            .location
            .as_ref()
            .map(|at| (at.file.as_str(), at.line)),
        Some(("text/first_test.go", 5))
    );
    let crash = first.crash.as_ref().unwrap();
    let reproducer = crash.reproducer.clone().unwrap();
    assert!(copy.path().join(&reproducer).is_file(), "{reproducer}");
    assert!(
        crash.reproduce.as_deref().is_some_and(
            |command| command.starts_with("mkdir -p text/testdata/fuzz/FuzzSecondByte && cp ")
        ),
        "{crash:?}"
    );
    assert_eq!(
        tree(copy.path()),
        before,
        "nothing was written into the package's testdata"
    );
}

#[test]
fn jazzer_crashes_first_on_the_empty_input_and_passes_size() {
    if !installed("tool.jazzer", &["jazzer", "node"]) {
        return;
    }
    let copy = support::own_fixture("gen.fuzz", "javascript");
    let before = tree(copy.path());
    let results = fuzz("tool.jazzer", "javascript", copy.path(), 200);
    let crash = Some((
        "Error: empty input".to_owned(),
        Some(("first.js".to_owned(), 4)),
        String::new(),
    ));
    assert_eq!(
        summary(&results),
        [
            ("first.fuzz".to_owned(), FuzzStatus::Crashed, crash),
            ("size.fuzz".to_owned(), FuzzStatus::Passed, None),
        ]
    );
    assert_eq!(
        tree(copy.path()),
        before,
        "the fuzz run wrote nothing in the tree"
    );
}

/// An atheris fuzz script whose target raises on the empty input.
const ATHERIS_SCRIPT: &str = "import sys\n\nimport atheris\n\n\ndef first(text):\n    return text[0]\n\n\n\
def test_one_input(data):\n    first(data.decode(\"latin-1\"))\n\n\natheris.Setup(sys.argv, test_one_input)\n\
atheris.Fuzz()\n";

#[test]
fn atheris_crashes_a_script_on_the_empty_input_or_says_how_to_install_it() {
    let producer = support::producer(
        support::shared_registry(),
        FactKind::FuzzResults,
        "tool.atheris",
        "python",
    );
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("fuzz_first.py"), ATHERIS_SCRIPT).unwrap();
    let ctx = support::produce_ctx(dir.path(), "python", Cancellation::new());
    if producer.availability(&ctx.ctx) != Availability::Ready {
        eprintln!(
            "SKIPPED: atheris cannot be imported by python3 (it builds only against a clang that \
             ships libFuzzer); tool.atheris was not run"
        );
        assert_eq!(
            producer.availability(&ctx.ctx),
            Availability::MissingTool {
                name: cupel_core::ToolId::from("atheris"),
                install: Some(cupel_core::Install::Command(
                    "python3 -m pip install atheris".to_owned()
                )),
            }
        );
        return;
    }
    let results = fuzz("tool.atheris", "python", dir.path(), 200);
    let first = &results.targets[0];
    assert_eq!(
        (first.name.as_str(), first.status),
        ("fuzz_first.py", FuzzStatus::Crashed)
    );
    let crash = first.crash.as_ref().unwrap();
    assert_eq!(crash.message, "IndexError: string index out of range");
    assert_eq!(
        crash
            .location
            .as_ref()
            .map(|at| (at.file.as_str(), at.line)),
        Some(("fuzz_first.py", 7))
    );
}

#[test]
fn a_root_without_targets_is_searched_and_has_none() {
    if !installed("tool.jazzer", &["jazzer", "node"]) {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("package.json"), "{}\n").unwrap();
    fs::write(dir.path().join("index.js"), "module.exports = 1;\n").unwrap();
    let results = fuzz("tool.jazzer", "javascript", dir.path(), 10);
    assert_eq!(
        (results.roots.clone(), results.targets.len()),
        (vec![".".to_owned()], 0)
    );
}
