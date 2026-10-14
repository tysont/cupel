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
    /// The command for one root.
    pub command: fn(&Root) -> Process,
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
        let mut version: Vec<String> = self
            .0
            .version
            .iter()
            .map(|word| (*word).to_owned())
            .collect();
        // A runner found in `node_modules/.bin` reports that copy's version.
        if self.0.node_bin
            && let Some(program) = self.program(&ctx.root)
        {
            version[0] = program;
        }
        Some(ToolProbe {
            name: ToolId::from(self.0.tool),
            version,
        })
    }

    fn produce(&self, ctx: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<TestResults> {
        let repo = &ctx.ctx.root;
        let mut invocations = Vec::new();
        let mut tests = Vec::new();
        for (index, (dir, packages)) in roots(ctx).into_iter().enumerate() {
            let absolute = repo.join(&dir);
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
            let process = (self.0.command)(&root);
            let captured = process.run(&ctx.cancel)?;
            let ran = Ran {
                root: &root,
                captured: &captured,
                report: std::fs::read_to_string(&root.report).ok(),
            };
            let found = (self.0.read)(&ran);
            let completed = found.as_ref().is_some_and(|found| {
                captured
                    .exit_code
                    .is_some_and(|code| self.0.completed_codes.contains(&code))
                    || found.iter().any(|test| test.outcome.is_failure())
            });
            invocations.push(RunnerInvocation {
                dir: dir.clone(),
                command: process.command_line(),
                exit_code: captured.exit_code,
                completed,
                output: (!completed)
                    .then(|| output_tail(&format!("{}\n{}", captured.stdout, captured.stderr)))
                    .flatten(),
            });
            tests.extend(
                found
                    .unwrap_or_default()
                    .into_iter()
                    .map(|test| placed(test, &dir)),
            );
        }
        Ok(TestResults::new(self.id(), invocations, tests, repo))
    }
}

impl Runner {
    /// The program to run in `dir`: its `node_modules/.bin` copy when the spec allows one, else
    /// the tool on `PATH`; `None` when neither exists.
    fn program(&self, dir: &Path) -> Option<String> {
        let local = dir.join("node_modules").join(".bin").join(self.0.tool);
        if self.0.node_bin && local.is_file() {
            return Some(local.to_string_lossy().into_owned());
        }
        find_program(self.0.tool).map(|_| self.0.tool.to_owned())
    }
}

/// The project roots of the language being produced, each with its packages; the repository
/// root alone when `ProjectInfo` names none.
fn roots(ctx: &ProduceCtx) -> Vec<(String, Vec<String>)> {
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

/// `test`, reported relative to the root at `dir`, made relative to the repository root.
fn placed(mut test: TestResult, dir: &str) -> TestResult {
    if dir != "." {
        test.id = format!("{dir}: {}", test.id);
        test.file = test.file.map(|file| format!("{dir}/{file}"));
    }
    test
}
