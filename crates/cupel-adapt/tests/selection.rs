// ABOUTME: The `Runner` surface of the runner producers (ADR-077 §3): listings parsed from committed captures,
// ABOUTME: the commands that run a selection, and real listed and narrowed runs, skipped when a tool is absent.
use std::time::Duration;

use cupel_adapt::report::listing;
use cupel_core::syntax::{ProjectInfo, ProjectRoot};
use cupel_core::{
    Availability, Cancellation, LangId, ProduceCtx, Produced, ProducerId, ProducerRegistry, Runner,
    RunnerCommand, TestOutcome, TestSelection,
};
use cupel_fixtures::Capture;
use support::{lang, produce_ctx, registry};

mod support;

fn tests(ids: &[&str]) -> TestSelection {
    TestSelection::Tests(ids.iter().map(|id| (*id).to_owned()).collect())
}

fn files(paths: &[&str]) -> TestSelection {
    TestSelection::Files(paths.iter().map(|path| (*path).to_owned()).collect())
}

fn runner<'a>(registry: &'a ProducerRegistry, id: &str) -> &'a dyn Runner {
    registry.runner(&ProducerId::from(id)).unwrap()
}

#[test]
fn every_runner_producer_is_registered_as_a_runner() {
    let registry = registry();
    for lang in ["rust", "go", "python", "javascript", "typescript"] {
        for producer in registry.producers_for(cupel_core::FactKind::TestResults, &self::lang(lang))
        {
            assert!(
                registry.runner(&producer.id()).is_some(),
                "{}",
                producer.id()
            );
        }
    }
}

#[test]
fn listings_are_read_from_real_output_in_the_runners_id_schemes() {
    let read = |tool: &str, file: &str, parse: fn(&str) -> Vec<String>| {
        parse(&Capture::of(&format!("per-test/{tool}"), "list").file(file))
    };
    assert_eq!(
        read("cargo-nextest", "list.json", listing::nextest),
        ["calc tests::adds", "calc tests::scales"]
    );
    assert_eq!(
        read("go-test", "stdout.jsonl", listing::go),
        ["example.com/calc/TestAdd", "example.com/calc/TestScale"]
    );
    assert_eq!(
        read("pytest", "stdout.txt", listing::pytest),
        [
            "tests/test_calc.py::test_add",
            "tests/test_calc.py::test_scale"
        ]
    );
}

/// The commands `id` gives for `selection` in a copy of `per-test/<repo>`.
fn commands(id: &str, repo: &str, lang: &str, selection: &TestSelection) -> Option<Vec<String>> {
    let dir = tempfile::tempdir().unwrap();
    cupel_fixtures::copy_repo(&format!("per-test/{repo}"), dir.path());
    let registry = registry();
    let ctx = produce_ctx(dir.path(), lang, Cancellation::new());
    runner(&registry, id)
        .commands(&ctx, selection)
        .map(|commands| {
            commands
                .into_iter()
                .map(|command| format!("{}: {}", command.dir, command.command))
                .collect()
        })
}

#[test]
fn a_selection_becomes_the_table_command_with_the_arguments_that_narrow_it() {
    assert_eq!(
        commands(
            "tool.cargo-nextest",
            "rust",
            "rust",
            &tests(&["calc tests::adds", "calc tests::scales"])
        ),
        Some(vec![
            ".: cargo nextest run --workspace -E '(binary_id(=calc) & test(=tests::adds)) | \
             (binary_id(=calc) & test(=tests::scales))'"
                .to_owned()
        ])
    );
    assert_eq!(
        commands(
            "tool.go-test",
            "go",
            "go",
            &tests(&["example.com/calc/TestAdd"])
        ),
        Some(vec![".: go test -json ./... -run '^(TestAdd)$'".to_owned()])
    );
    assert_eq!(
        commands(
            "tool.pytest",
            "python",
            "python",
            &tests(&["tests/test_calc.py::test_add"])
        ),
        Some(vec![
            ".: python -m pytest tests/test_calc.py::test_add".to_owned()
        ])
    );
    assert_eq!(
        commands(
            "tool.jest",
            "javascript",
            "javascript",
            &files(&["add.test.js"])
        ),
        Some(vec![".: npx jest add.test.js".to_owned()])
    );
    assert_eq!(
        commands("tool.pytest", "python", "python", &TestSelection::All),
        Some(vec![".: python -m pytest".to_owned()])
    );
}

#[test]
fn a_runner_that_cannot_narrow_to_a_selection_gives_no_command() {
    assert_eq!(
        commands(
            "tool.cargo-test",
            "rust",
            "rust",
            &tests(&["calc/src/lib.rs::tests::adds"])
        ),
        None
    );
    assert_eq!(
        commands(
            "tool.jest",
            "javascript",
            "javascript",
            &tests(&["add.test.js > adds"])
        ),
        None
    );
}

#[test]
fn a_selection_reaches_only_the_roots_that_hold_it_relative_to_each() {
    let dir = tempfile::tempdir().unwrap();
    let mut ctx = ProduceCtx::new(
        cupel_core::Ctx::new(dir.path().to_path_buf()),
        cupel_core::Writer::new(dir.path(), false)
            .unwrap()
            .confined(),
        Cancellation::new(),
    );
    let root = |path: &str| ProjectRoot {
        path: path.to_owned(),
        lang: LangId::from("javascript"),
        build_files: Vec::new(),
        members: Vec::new(),
        packages: Vec::new(),
        skip_dirs: Vec::new(),
    };
    ctx.upstream.insert(
        Produced::new(ProjectInfo {
            roots: vec![root("."), root("web"), root("web/app")],
        })
        .share(),
    );
    let registry = registry();
    let commands = runner(&registry, "tool.jest")
        .commands(
            &ctx,
            &files(&["web/app/a.test.js", "web/b.test.js", "c.test.js"]),
        )
        .unwrap();
    let shown: Vec<(&str, &str)> = commands
        .iter()
        .map(|RunnerCommand { dir, command }| (dir.as_str(), command.as_str()))
        .collect();
    assert_eq!(
        shown,
        [
            (".", "npx jest c.test.js"),
            ("web", "npx jest b.test.js"),
            ("web/app", "npx jest a.test.js")
        ]
    );
}

/// A copy of `per-test/<repo>` and the context of `lang`'s runner `id` there, or `None` (reported
/// as skipped) when the runner's tool is not installed.
fn installed(id: &str, repo: &str, lang: &str) -> Option<(tempfile::TempDir, ProduceCtx)> {
    let dir = tempfile::tempdir().unwrap();
    cupel_fixtures::copy_repo(&format!("per-test/{repo}"), dir.path());
    let ctx = produce_ctx(dir.path(), lang, Cancellation::new());
    let registry = registry();
    if let Availability::MissingTool { name, .. } = runner(&registry, id).availability(&ctx.ctx) {
        eprintln!("SKIPPED: {name} not installed; {id} was not run");
        return None;
    }
    Some((dir, ctx))
}

/// What `id` lists in `per-test/<repo>`, and what running only `selected` reports.
fn listed_and_selected(id: &str, repo: &str, lang: &str, selected: &str) {
    let Some((_dir, ctx)) = installed(id, repo, lang) else {
        return;
    };
    let registry = registry();
    let runner = runner(&registry, id);
    let ids = runner.list(&ctx).unwrap();
    assert_eq!(ids.len(), 2, "{ids:?}");
    assert!(ids.iter().any(|listed| listed == selected), "{ids:?}");
    let results = runner.run(&ctx, &tests(&[selected]), &[], None).unwrap();
    let ran: Vec<(&str, TestOutcome)> = results
        .tests
        .iter()
        .map(|test| (test.id.as_str(), test.outcome))
        .collect();
    assert_eq!(ran, [(selected, TestOutcome::Passed)], "{id}");
    assert!(results.is_green());
}

#[test]
fn cargo_nextest_lists_and_runs_one_test() {
    listed_and_selected("tool.cargo-nextest", "rust", "rust", "calc tests::adds");
}

#[test]
fn go_test_lists_and_runs_one_test() {
    listed_and_selected("tool.go-test", "go", "go", "example.com/calc/TestAdd");
}

#[test]
fn pytest_lists_and_runs_one_test() {
    listed_and_selected(
        "tool.pytest",
        "python",
        "python",
        "tests/test_calc.py::test_add",
    );
}

#[test]
fn a_run_its_own_timeout_ends_is_incomplete_and_leaves_the_run_going() {
    let Some((_dir, ctx)) = installed("tool.pytest", "python", "python") else {
        return;
    };
    let registry = registry();
    let results = runner(&registry, "tool.pytest")
        .run(
            &ctx,
            &TestSelection::All,
            &[],
            Some(Duration::from_millis(1)),
        )
        .unwrap();
    let invocation = &results.invocations[0];
    assert_eq!(
        (
            invocation.completed,
            invocation.exit_code,
            invocation.output.as_deref()
        ),
        (false, None, Some("timed out after 1 ms"))
    );
    assert!(!ctx.cancel.is_cancelled());
}

#[test]
fn a_runner_that_cannot_list_says_which_runners_can() {
    let dir = tempfile::tempdir().unwrap();
    let registry = registry();
    let error = runner(&registry, "tool.jest")
        .list(&produce_ctx(dir.path(), "javascript", Cancellation::new()))
        .unwrap_err();
    assert!(error.to_string().contains("tool.cargo-nextest"), "{error}");
}
