// ABOUTME: What every runner producer shares (ADR-076): a `RunnerSpec` names the tool, builds the command
// ABOUTME: for a project root, and reads its report; `Runner` runs it once per root into `TestResults`.
use std::path::{Path, PathBuf};

use cupel_core::syntax::ProjectInfo;
use cupel_core::{
    Availability, Captured, CostClass, Ctx, EventSink, FactKind, Langs, Process, ProduceCtx,
    Producer, ProducerId, Result, RunnerInvocation, Scope, TestResult, TestResults, ToolId,
    ToolProbe, find_program, output_tail,
};

use crate::toolchain;

/// One runner: everything that differs between `cargo test`, `pytest`, `jest`, and the rest.
pub struct RunnerSpec {
    /// The producer ID, such as `tool.pytest`; the toolchain table lists it.
    pub id: &'static str,
    pub languages: fn() -> Langs,
    /// The program that must be on `PATH`, such as `pytest` or `cargo-nextest`.
    pub tool: &'static str,
    /// Whether `node_modules/.bin/<tool>` in the root counts as the tool.
    pub node_bin: bool,
    /// The command that prints the tool's version.
    pub version: &'static [&'static str],
    /// The command line for one root: the program, then its arguments.
    pub command: fn(&Root) -> Vec<String>,
    /// Variables added to the runner's environment.
    pub env: &'static [(&'static str, &'static str)],
    /// Whether the runner's standard error is merged into its standard output, in order.
    pub merged: bool,
    /// The tests a finished run reported, relative to its root; `None` when the run left no
    /// readable report.
    pub read: fn(&Ran) -> Option<Vec<TestResult>>,
    /// Exit codes that mean the suite ran to its end even with no failing test.
    pub completed_codes: &'static [i32],
}

/// A project root a runner runs in.
pub struct Root {
    /// Absolute and canonical.
    pub dir: PathBuf,
    /// The program to run: the tool on `PATH`, or the root's `node_modules/.bin` copy.
    pub program: String,
    /// Package patterns from `ProjectInfo` (`./...`); empty when the ecosystem takes none.
    pub packages: Vec<String>,
    /// Where the runner should write its report, when it writes one to a file.
    pub report: PathBuf,
    /// A file in `.cupel/` the runner may use for its own configuration.
    pub config: PathBuf,
}

/// A finished run in one root.
pub struct Ran<'a> {
    pub root: &'a Root,
    pub captured: &'a Captured,
    /// The report file's text, when the runner wrote one.
    pub report: Option<String>,
}

/// A runner producer: `spec` run once per project root of its language.
pub struct Runner(pub &'static RunnerSpec);

impl Producer<TestResults> for Runner {
    fn id(&self) -> ProducerId {
        ProducerId::from(self.0.id)
    }

    fn languages(&self) -> Langs {
        (self.0.languages)()
    }

    fn needs(&self) -> &[FactKind] {
        &[FactKind::ProjectInfo]
    }

    fn cost(&self) -> CostClass {
        CostClass::Standard
    }

    fn availability(&self, ctx: &Ctx) -> Availability {
        if self.program(&ctx.root).is_some() {
            Availability::Ready
        } else {
            Availability::MissingTool {
                name: ToolId::from(self.0.tool),
                install: None,
            }
        }
    }

    fn detected(&self, ctx: &Ctx) -> bool {
        toolchain::detects(&self.id(), &ctx.root)
    }

    fn tool(&self, ctx: &Ctx) -> Option<ToolProbe> {
        Some(probe(
            self.0.tool,
            self.0.node_bin,
            self.0.version,
            &ctx.root,
        ))
    }

    fn produce(&self, ctx: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<TestResults> {
        let mut invocations = Vec::new();
        let mut tests = Vec::new();
        for (index, (dir, packages)) in roots(ctx).into_iter().enumerate() {
            let (invocation, found) = self.run_root(ctx, index, &dir, packages)?;
            invocations.push(invocation);
            tests.extend(found.into_iter().map(|test| placed(test, &dir)));
        }
        Ok(TestResults::new(
            self.id(),
            invocations,
            tests,
            &ctx.ctx.root,
        ))
    }
}

impl Runner {
    /// Runs the suite in the project root `dir` (the `index`th root) and reads what it reported,
    /// relative to that root.
    fn run_root(
        &self,
        ctx: &ProduceCtx,
        index: usize,
        dir: &str,
        packages: Vec<String>,
    ) -> Result<(RunnerInvocation, Vec<TestResult>)> {
        let absolute = ctx.ctx.root.join(dir);
        let absolute = absolute.canonicalize().unwrap_or(absolute);
        let area = ctx.writer.create_dir(
            &ctx.writer
                .cache_dir()
                .join("runners")
                .join(self.0.id)
                .join(index.to_string()),
        )?;
        let root = Root {
            program: self
                .program(&absolute)
                .unwrap_or_else(|| self.0.tool.to_owned()),
            dir: absolute,
            packages,
            report: area.join("report"),
            config: area.join("config.toml"),
        };
        let _ = std::fs::remove_file(&root.report);
        let process = self.process(&root);
        let captured = process.run(&ctx.cancel)?;
        let found = (self.0.read)(&Ran {
            root: &root,
            captured: &captured,
            report: std::fs::read_to_string(&root.report).ok(),
        });
        let completed = found.as_ref().is_some_and(|found| {
            captured
                .exit_code
                .is_some_and(|code| self.0.completed_codes.contains(&code))
                || found.iter().any(|test| test.outcome.is_failure())
        });
        let invocation = RunnerInvocation {
            dir: dir.to_owned(),
            command: process.command_line(),
            exit_code: captured.exit_code,
            completed,
            output: (!completed)
                .then(|| output_tail(&format!("{}\n{}", captured.stdout, captured.stderr)))
                .flatten(),
        };
        Ok((invocation, found.unwrap_or_default()))
    }

    /// The process that runs the suite in `root`.
    fn process(&self, root: &Root) -> Process {
        let command = (self.0.command)(root);
        let (program, args) = command.split_first().unwrap_or((&root.program, &[]));
        let mut process = Process::new(program, &root.dir).args(args.iter().cloned());
        for (key, value) in self.0.env {
            process = process.env(key, value);
        }
        if self.0.merged {
            process = process.merged();
        }
        process
    }

    /// The program to run in `dir`: its `node_modules/.bin` copy when the spec allows one, else
    /// the tool on `PATH`; `None` when neither exists.
    fn program(&self, dir: &Path) -> Option<String> {
        installed(self.0.tool, self.0.node_bin, dir)
    }
}

/// The probe of `tool`, whose version `version` prints; a tool found in `dir`'s
/// `node_modules/.bin` (when `node_bin` allows one) reports that copy's version.
pub(crate) fn probe(tool: &str, node_bin: bool, version: &[&str], dir: &Path) -> ToolProbe {
    let mut version: Vec<String> = version.iter().map(|word| (*word).to_owned()).collect();
    if node_bin && let Some(program) = installed(tool, node_bin, dir) {
        version[0] = program;
    }
    ToolProbe {
        name: ToolId::from(tool),
        version,
    }
}

/// The program to run for `tool` in `dir`: its `node_modules/.bin` copy when `node_bin` allows
/// one, else the tool on `PATH`; `None` when neither exists.
pub(crate) fn installed(tool: &str, node_bin: bool, dir: &Path) -> Option<String> {
    let local = dir.join("node_modules").join(".bin").join(tool);
    if node_bin && local.is_file() {
        return Some(local.to_string_lossy().into_owned());
    }
    find_program(tool).map(|_| tool.to_owned())
}

/// The project roots of the language being produced, each with its packages; the repository
/// root alone when `ProjectInfo` names none.
pub(crate) fn roots(ctx: &ProduceCtx) -> Vec<(String, Vec<String>)> {
    let roots: Vec<(String, Vec<String>)> = ctx
        .upstream
        .get::<ProjectInfo>()
        .map(|info| {
            info.roots
                .iter()
                .map(|root| (root.path.clone(), root.packages.clone()))
                .collect()
        })
        .unwrap_or_default();
    if roots.is_empty() {
        vec![(".".to_owned(), Vec::new())]
    } else {
        roots
    }
}

/// The command line of a runner that writes the Jest JSON report (`jest`, `vitest`): the root's
/// program, `args`, then the flags that pass an empty suite and name the report file.
pub fn jest_report_command(root: &Root, args: &[&str]) -> Vec<String> {
    std::iter::once(root.program.clone())
        .chain(args.iter().map(|arg| (*arg).to_owned()))
        .chain([
            "--passWithNoTests".to_owned(),
            format!("--outputFile={}", root.report.display()),
        ])
        .collect()
}

/// `test`, reported relative to the root at `dir`, made relative to the repository root.
fn placed(mut test: TestResult, dir: &str) -> TestResult {
    if dir != "." {
        test.id = format!("{dir}: {}", test.id);
        test.file = test.file.map(|file| format!("{dir}/{file}"));
    }
    test
}
