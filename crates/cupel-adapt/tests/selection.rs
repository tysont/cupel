// ABOUTME: The `Runner` surface of the runner producers (ADR-077 §3): listings parsed from committed captures,
// ABOUTME: the commands that run a selection, and real listed and narrowed runs, skipped when a tool is absent.
use std::time::Duration;

use cupel_adapt::report::listing;
use cupel_core::syntax::{ProjectInfo, ProjectRoot};
use cupel_core::{
    Availability, Cancellation, LangId, ProduceCtx, Produced, ProducerId, ProducerRegistry,
    RunOptions, RunOrder, Runner, RunnerCommand, TestOutcome, TestSelection,
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

/// The `TestResults` producers of every language that are not registered as runners.
fn not_runners(registry: &ProducerRegistry) -> Vec<String> {
    ["rust", "go", "python", "javascript", "typescript"]
        .into_iter()
        .flat_map(|lang| {
            registry.producers_for(cupel_core::FactKind::TestResults, &self::lang(lang))
        })
        .map(cupel_core::RegisteredProducer::id)
        .filter(|id| registry.runner(id).is_none())
        .map(|id| id.to_string())
        .collect()
}

#[test]
fn every_runner_producer_is_registered_as_a_runner() {
    assert_eq!(not_runners(&registry()), Vec::<String>::new());
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

fn except(ids: &[&str]) -> TestSelection {
    TestSelection::Except(ids.iter().map(|id| (*id).to_owned()).collect())
}

#[test]
fn the_whole_suite_but_some_tests_becomes_the_runners_flag_that_leaves_them_out() {
    assert_eq!(
        commands(
            "tool.cargo-nextest",
            "rust",
            "rust",
            &except(&["calc tests::scales"])
        ),
        Some(vec![
            ".: cargo nextest run --workspace -E 'not ((binary_id(=calc) & test(=tests::scales)))'"
                .to_owned()
        ])
    );
    assert_eq!(
        commands(
            "tool.go-test",
            "go",
            "go",
            &except(&["example.com/calc/TestScale"])
        ),
        Some(vec![
            ".: go test -json ./... -skip '^(TestScale)$'".to_owned()
        ])
    );
    assert_eq!(
        commands(
            "tool.pytest",
            "python",
            "python",
            &except(&[
                "tests/test_calc.py::test_add",
                "tests/test_calc.py::test_scale"
            ])
        ),
        Some(vec![
            ".: python -m pytest --deselect tests/test_calc.py::test_add --deselect \
             tests/test_calc.py::test_scale"
                .to_owned()
        ])
    );
    assert_eq!(
        commands("tool.pytest", "python", "python", &except(&[])),
        Some(vec![".: python -m pytest".to_owned()])
    );
}

#[test]
fn a_runner_that_cannot_leave_a_test_out_gives_no_command_for_one() {
    for (id, repo, test) in [
        ("tool.cargo-test", "rust", "calc/src/lib.rs::tests::adds"),
        ("tool.jest", "javascript", "add.test.js > adds"),
        (
            "tool.unittest",
            "python",
            "tests.test_calc.CalcTest.test_add",
        ),
    ] {
        assert_eq!(commands(id, repo, repo, &except(&[test])), None, "{id}");
    }
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

/// `id` lists two tests in `per-test/<repo>`, `selected` among them, and runs only `selected`.
fn assert_lists_and_runs_one(id: &str, repo: &str, lang: &str, selected: &str) {
    let Some((_dir, ctx)) = installed(id, repo, lang) else {
        return;
    };
    let registry = registry();
    let runner = runner(&registry, id);
    let ids = runner.list(&ctx).unwrap();
    assert_eq!(ids.len(), 2, "{ids:?}");
    assert!(ids.iter().any(|listed| listed == selected), "{ids:?}");
    let results = runner
        .run(&ctx, &tests(&[selected]), &RunOptions::default())
        .unwrap()
        .results;
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
    assert_lists_and_runs_one("tool.cargo-nextest", "rust", "rust", "calc tests::adds");
}

#[test]
fn go_test_lists_and_runs_one_test() {
    assert_lists_and_runs_one("tool.go-test", "go", "go", "example.com/calc/TestAdd");
}

#[test]
fn pytest_lists_and_runs_one_test() {
    assert_lists_and_runs_one(
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
            &RunOptions {
                timeout: Some(Duration::from_millis(1)),
                ..RunOptions::default()
            },
        )
        .unwrap()
        .results;
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
    assert_eq!(
        error.to_string(),
        "invalid runner `tool.jest`: expected a runner that lists its tests: \
         tool.cargo-nextest, tool.go-test, tool.pytest"
    );
}

/// A copy of `tests.flaky/python` and the context of its unittest runner, or `None` (reported as
/// skipped) without `python3`.
fn unittest_fixture() -> Option<(tempfile::TempDir, ProduceCtx)> {
    let dir = tempfile::tempdir().unwrap();
    cupel_fixtures::copy_repo("tests.flaky/python", dir.path());
    let ctx = produce_ctx(dir.path(), "python", Cancellation::new());
    let registry = registry();
    if let Availability::MissingTool { name, .. } =
        runner(&registry, "tool.unittest").availability(&ctx.ctx)
    {
        eprintln!("SKIPPED: {name} not installed; tool.unittest was not run");
        return None;
    }
    Some((dir, ctx))
}

#[test]
fn an_explicit_order_runner_runs_a_selection_in_the_order_given_and_reports_it() {
    let Some((_dir, ctx)) = unittest_fixture() else {
        return;
    };
    let registry = registry();
    let order = [
        "tests.test_ledger.LedgerTest.test_deposit_changes_balance",
        "tests.test_ledger.LedgerTest.test_balance_starts_at_zero",
    ];
    let ran = runner(&registry, "tool.unittest")
        .run(
            &ctx,
            &tests(&order),
            &RunOptions {
                order: RunOrder::Reversed,
                seed: Some(7),
                parallelism: Some(1),
                ..RunOptions::default()
            },
        )
        .unwrap();
    assert_eq!(ran.executed.as_deref(), Some(&order.map(str::to_owned)[..]));
    let outcomes: Vec<TestOutcome> = ran.results.tests.iter().map(|test| test.outcome).collect();
    // Sorted by ID: the balance test ran second, after the deposit, and failed.
    assert_eq!(outcomes, [TestOutcome::Failed, TestOutcome::Passed]);
    let command = &ran.results.invocations[0].command;
    assert!(
        !command.iter().any(|word| word == "discover"),
        "{command:?}"
    );
}

#[test]
fn a_seeded_run_hands_the_seed_to_the_runner() {
    let Some((dir, ctx)) = unittest_fixture() else {
        return;
    };
    std::fs::write(
        dir.path().join("tests").join("test_seed.py"),
        "import os, unittest\n\nclass SeedTest(unittest.TestCase):\n    def test_seed(self):\n        self.assertEqual(os.environ.get('PYTHONHASHSEED'), '11')\n",
    )
    .unwrap();
    let registry = registry();
    let ran = runner(&registry, "tool.unittest")
        .run(
            &ctx,
            &tests(&["tests.test_seed.SeedTest.test_seed"]),
            &RunOptions {
                seed: Some(11),
                ..RunOptions::default()
            },
        )
        .unwrap();
    let outcomes: Vec<TestOutcome> = ran.results.tests.iter().map(|test| test.outcome).collect();
    assert_eq!(outcomes, [TestOutcome::Passed]);
}

/// `id` runs every test of `per-test/<repo>` but `left_out`, which leaves `kept`.
fn assert_leaves_out_one(id: &str, repo: &str, lang: &str, left_out: &str, kept: &str) {
    let Some((_dir, ctx)) = installed(id, repo, lang) else {
        return;
    };
    let registry = registry();
    let results = runner(&registry, id)
        .run(&ctx, &except(&[left_out]), &RunOptions::default())
        .unwrap()
        .results;
    let ran: Vec<(&str, TestOutcome)> = results
        .tests
        .iter()
        .map(|test| (test.id.as_str(), test.outcome))
        .collect();
    assert_eq!(ran, [(kept, TestOutcome::Passed)], "{id}");
    assert!(results.is_green());
}

#[test]
fn cargo_nextest_runs_every_test_but_one() {
    assert_leaves_out_one(
        "tool.cargo-nextest",
        "rust",
        "rust",
        "calc tests::scales",
        "calc tests::adds",
    );
}

#[test]
fn go_test_runs_every_test_but_one() {
    assert_leaves_out_one(
        "tool.go-test",
        "go",
        "go",
        "example.com/calc/TestScale",
        "example.com/calc/TestAdd",
    );
}

#[test]
fn pytest_runs_every_test_but_one() {
    assert_leaves_out_one(
        "tool.pytest",
        "python",
        "python",
        "tests/test_calc.py::test_scale",
        "tests/test_calc.py::test_add",
    );
}
