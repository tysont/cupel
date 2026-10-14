// ABOUTME: How the flake engine reruns each runner it knows (ADR-078): the arguments and environment a
// ABOUTME: rerun's `RunOptions` become, and the shared report parser that reads it, with the run order.
use std::path::{Path, PathBuf};

use cupel_core::{Captured, RunOrder, TestResult};

use super::options::RunOptions;
use super::order::go_test_order;
use crate::report::{jest_json, junit, libtest, test2json, unittest};

/// One runner the engine reruns: the producer whose baseline it repeats, and how.
pub struct RerunSpec {
    /// The runner producer's ID, such as `tool.pytest`.
    pub runner: &'static str,
    /// The program, on `PATH` or (with `node_bin`) in the root's `node_modules/.bin`.
    pub tool: &'static str,
    pub node_bin: bool,
    /// Whether the runner takes the tests to run, in order, as arguments.
    pub explicit_order: bool,
    /// Whether a serial run reports its tests in the order they ran.
    pub reports_order: bool,
    /// The command for one root.
    pub command: fn(&Rerun<'_>) -> Command,
    /// The tests a finished rerun reported, in report order; `None` without a readable report.
    pub read: fn(&Ran<'_>) -> Option<Vec<TestResult>>,
    /// The test IDs in the order a serial rerun ran them; the report order when `None`.
    pub ran_order: Option<fn(&Captured) -> Vec<String>>,
}

/// One rerun in one project root.
pub struct Rerun<'a> {
    /// The program to run.
    pub program: String,
    /// Where the runner writes its report, when it writes one to a file.
    pub report: PathBuf,
    pub options: &'a RunOptions,
    /// The baseline's command line in this root, whose packages a rerun repeats.
    pub baseline: &'a [String],
}

/// What a rerun runs: the program and its arguments, the variables added to its environment,
/// and whether its standard error is merged into its standard output, in order.
pub struct Command {
    pub argv: Vec<String>,
    pub env: Vec<(String, String)>,
    pub merged: bool,
}

/// A finished rerun in one root.
pub struct Ran<'a> {
    pub dir: &'a Path,
    pub captured: &'a Captured,
    pub report: Option<String>,
}

/// The rerun spec of the runner producer `runner`.
pub fn spec(runner: &str) -> Option<&'static RerunSpec> {
    SPECS.iter().find(|spec| spec.runner == runner)
}

/// Every runner the engine reruns, by producer ID.
pub fn runners() -> impl Iterator<Item = &'static str> {
    SPECS.iter().map(|spec| spec.runner)
}

static SPECS: [RerunSpec; 6] = [
    RerunSpec {
        runner: "tool.cargo-test",
        tool: "cargo",
        node_bin: false,
        explicit_order: false,
        reports_order: true,
        command: cargo_test,
        read: |ran| Some(libtest::parse(&ran.captured.stdout)),
        ran_order: None,
    },
    RerunSpec {
        runner: "tool.go-test",
        tool: "go",
        node_bin: false,
        explicit_order: false,
        reports_order: true,
        command: go_test,
        read: |ran| Some(test2json::parse(&ran.captured.stdout, &ran.captured.stderr)),
        ran_order: Some(|captured| go_test_order(&captured.stdout)),
    },
    RerunSpec {
        runner: "tool.pytest",
        tool: "pytest",
        node_bin: false,
        explicit_order: true,
        reports_order: true,
        command: pytest,
        read: |ran| junit::pytest(ran.report.as_deref()?),
        ran_order: None,
    },
    RerunSpec {
        runner: "tool.unittest",
        tool: "python3",
        node_bin: false,
        explicit_order: true,
        reports_order: true,
        command: unittest,
        read: |ran| Some(unittest::parse(&ran.captured.stdout)),
        ran_order: None,
    },
    RerunSpec {
        runner: "tool.vitest",
        tool: "vitest",
        node_bin: true,
        explicit_order: false,
        reports_order: false,
        command: vitest,
        read: |ran| jest_json::parse(ran.report.as_deref()?, ran.dir),
        ran_order: None,
    },
    RerunSpec {
        runner: "tool.jest",
        tool: "jest",
        node_bin: true,
        explicit_order: false,
        reports_order: false,
        command: jest,
        read: |ran| jest_json::parse(ran.report.as_deref()?, ran.dir),
        ran_order: None,
    },
];

fn words(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

fn var(key: &str, value: &str) -> (String, String) {
    (key.to_owned(), value.to_owned())
}

/// `cargo test` over the test targets (no doc-tests, whose harness takes no unstable flags):
/// `--test-threads`, and for a shuffle libtest's `--shuffle-seed`, unstable on stable Rust, so
/// the test binaries (never `rustc`) run with `RUSTC_BOOTSTRAP=1` through a target runner.
fn cargo_test(rerun: &Rerun<'_>) -> Command {
    let options = rerun.options;
    let mut command = vec![rerun.program.clone()];
    command.extend(words(&["test", "--workspace", "--no-fail-fast", "--tests"]));
    let shuffled = options.order == RunOrder::Shuffled;
    if shuffled {
        command.extend(words(&[
            "--config",
            r#"target.'cfg(all())'.runner = ["env", "RUSTC_BOOTSTRAP=1"]"#,
        ]));
    }
    command.push("--".to_owned());
    if shuffled {
        command.extend(words(&["-Z", "unstable-options", "--shuffle-seed"]));
        command.push(options.seed.to_string());
    }
    if let Some(threads) = options.parallelism {
        command.push(format!("--test-threads={threads}"));
    }
    Command {
        argv: command,
        env: vec![var("CARGO_TERM_COLOR", "never")],
        merged: true,
    }
}

/// `go test -json` over the baseline's packages, uncached: `-shuffle` off or seeded, and one
/// package and one parallel test at a time when serial.
fn go_test(rerun: &Rerun<'_>) -> Command {
    let options = rerun.options;
    let mut command = vec![rerun.program.clone()];
    command.extend(words(&["test", "-json", "-count=1"]));
    command.push(match options.order {
        RunOrder::Shuffled => format!("-shuffle={}", options.seed_i63()),
        _ => "-shuffle=off".to_owned(),
    });
    if let Some(parallel) = options.parallelism {
        command.push(format!("-p={parallel}"));
        command.push(format!("-parallel={parallel}"));
    }
    let packages: Vec<String> = rerun
        .baseline
        .iter()
        .skip(2)
        .filter(|word| !word.starts_with('-'))
        .cloned()
        .collect();
    if packages.is_empty() {
        command.push("./...".to_owned());
    }
    command.extend(packages);
    Command {
        argv: command,
        env: Vec::new(),
        merged: false,
    }
}

/// `PYTHONHASHSEED` for the Python runners, with no bytecode written into the tree.
fn python_env(options: &RunOptions) -> Vec<(String, String)> {
    vec![
        var("PYTHONDONTWRITEBYTECODE", "1"),
        var("PYTHONHASHSEED", &(options.seed % (1 << 32)).to_string()),
    ]
}

/// pytest with a `JUnit` report, as the runner writes it, given the node IDs in order to
/// reverse; pytest runs one test at a time.
fn pytest(rerun: &Rerun<'_>) -> Command {
    let mut command = vec![rerun.program.clone()];
    command.extend(words(&[
        "-p",
        "no:cacheprovider",
        "-o",
        "junit_family=xunit1",
        "-o",
        "junit_logging=all",
    ]));
    command.push(format!("--junitxml={}", rerun.report.display()));
    command.extend(rerun.options.explicit.iter().cloned());
    Command {
        argv: command,
        env: python_env(rerun.options),
        merged: false,
    }
}

/// unittest: discovery, or the dotted test IDs in order to reverse.
fn unittest(rerun: &Rerun<'_>) -> Command {
    let mut command = vec![rerun.program.clone()];
    command.extend(words(&["-m", "unittest"]));
    if rerun.options.explicit.is_empty() {
        command.push("discover".to_owned());
    }
    command.extend(words(&["-v", "-b"]));
    command.extend(rerun.options.explicit.iter().cloned());
    Command {
        argv: command,
        env: python_env(rerun.options),
        merged: true,
    }
}

/// jest with its JSON report: `--seed`, `--randomize` for a shuffle, one worker when serial.
fn jest(rerun: &Rerun<'_>) -> Command {
    jest_json_runner(rerun, &["--ci", "--json"], "--seed", "--randomize")
}

/// vitest with its JSON report: `--sequence.seed`, `--sequence.shuffle` for a shuffle, one
/// worker when serial.
fn vitest(rerun: &Rerun<'_>) -> Command {
    jest_json_runner(
        rerun,
        &["run", "--reporter=json"],
        "--sequence.seed",
        "--sequence.shuffle",
    )
}

/// A runner that writes the Jest JSON report: its program and `args`, the flags that pass an
/// empty suite and name the report, the seed as `seed_flag`, `shuffle_flag` for a shuffle, and
/// `--maxWorkers`.
fn jest_json_runner(
    rerun: &Rerun<'_>,
    args: &[&str],
    seed_flag: &str,
    shuffle_flag: &str,
) -> Command {
    let options = rerun.options;
    let mut command = vec![rerun.program.clone()];
    command.extend(words(args));
    command.push("--passWithNoTests".to_owned());
    command.push(format!("--outputFile={}", rerun.report.display()));
    command.push(format!("{seed_flag}={}", options.seed_i32()));
    if options.order == RunOrder::Shuffled {
        command.push(shuffle_flag.to_owned());
    }
    if let Some(workers) = options.parallelism {
        command.push(format!("--maxWorkers={workers}"));
    }
    Command {
        argv: command,
        env: Vec::new(),
        merged: false,
    }
}
