// ABOUTME: The Gradle runner producer: `gradle test --rerun` with an init script pointing every test task's JUnit
// ABOUTME: reports into a directory under `.cupel/` that each run starts empty; `--tests` narrows (ADR-108).
use std::path::PathBuf;

use cupel_core::{LangId, Langs, RunOptions, RunOrder, RunOrdering, TestResult};

use crate::java_tests::{read_reports, selected_classes};
use crate::runner::{Executed, Ran, Root, Runner, RunnerSpec, Selected, Varying};

const ID: &str = "tool.gradle";

static GRADLE: RunnerSpec = RunnerSpec {
    id: ID,
    languages: || Langs::Only(vec![LangId::from("java")]),
    tool: "gradle",
    node_bin: false,
    version: &["gradle", "--version"],
    command,
    env: &[],
    merged: false,
    read,
    select,
    list: None,
    completed_codes: &[0],
    vary: &Varying {
        ordering: RunOrdering::Seeded,
        args: vary,
        env: |_| Vec::new(),
        // One report per class, read in name order: the reports do not say the order classes ran in.
        executed: Executed::Unknown,
    },
    outputs: &["build", ".gradle"],
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register_runner(Runner(&GRADLE)),
});

/// The init script: each project's test tasks write `JUnit` XML into `<reports>/<project path>`,
/// and a `cupelSeed` project property shuffles `JUnit` Jupiter's method order with that seed.
const INIT: &str = r"// Written by Cupel (ADR-108): test reports go where Cupel reads them.
allprojects {
    tasks.withType(Test).configureEach {
        reports.junitXml.outputLocation.set(new File('{reports}', project.path.replace(':', '_')))
        def seed = project.findProperty('cupelSeed')
        if (seed != null) {
            systemProperty 'junit.jupiter.testmethod.order.default', 'org.junit.jupiter.api.MethodOrderer$Random'
            systemProperty 'junit.jupiter.execution.order.random.seed', seed
        }
    }
}
";

/// Where the test tasks write their reports for `root`: a directory beside the runner's report
/// file.
fn reports(root: &Root) -> PathBuf {
    root.report.with_file_name("test-results")
}

/// `gradle test --rerun` under the init script, its reports in a directory this run empties
/// first. Narrowing arguments follow, as options of the `test` task.
fn command(root: &Root) -> Vec<String> {
    let reports = reports(root);
    let init = root.config.with_file_name("cupel-init.gradle");
    // A failure to write either surfaces as Gradle's error or as stale reports.
    let _ = std::fs::remove_dir_all(&reports);
    let script = INIT.replace(
        "{reports}",
        &reports.display().to_string().replace('\'', "\\'"),
    );
    let _ = std::fs::write(&init, script);
    vec![
        root.program.clone(),
        "--console=plain".to_owned(),
        "--init-script".to_owned(),
        init.display().to_string(),
        "test".to_owned(),
        "--rerun".to_owned(),
    ]
}

/// `--tests <class>` or `--tests <class>.<method>` for each test of the selection.
#[allow(
    clippy::unnecessary_wraps,
    reason = "every runner narrows through `RunnerSpec::select`; `--tests` narrows to anything"
)]
fn select(selected: &Selected<'_>) -> Option<Vec<String>> {
    let mut args = Vec::new();
    for (class, methods) in selected_classes(selected) {
        if methods.is_empty() {
            args.extend(["--tests".to_owned(), class]);
        } else {
            for method in methods {
                args.extend(["--tests".to_owned(), format!("{class}.{method}")]);
            }
        }
    }
    Some(args)
}

/// A shuffled run's seed, as the property the init script reads (ADR-078).
fn vary(options: &RunOptions) -> Vec<String> {
    match (options.order, options.seed) {
        (RunOrder::Shuffled, Some(seed)) => vec![format!("-PcupelSeed={seed}")],
        _ => Vec::new(),
    }
}

fn read(ran: &Ran<'_>) -> Option<Vec<TestResult>> {
    read_reports(ran, &[reports(ran.root)])
}
