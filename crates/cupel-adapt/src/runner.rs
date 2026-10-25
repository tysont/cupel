// ABOUTME: What every runner producer shares (ADR-076, ADR-077): a `RunnerSpec` names the tool, builds the
// ABOUTME: command for a project root, narrows it, and reads its report; `Runner` runs it per root.
use std::path::{Path, PathBuf};
use std::time::Duration;

use cupel_core::syntax::ProjectInfo;
use cupel_core::{
    Availability, Cancellation, Captured, CostClass, Ctx, Error, EventSink, FactKind, Install,
    Langs, Process, ProduceCtx, Producer, ProducerId, Result, RootRun, RunOptions, RunOrder,
    RunOrdering, RunStatus, RunnerCommand, RunnerInvocation, Scope, TestResult, TestResults,
    TestRun, TestSelection, ToolId, ToolProbe, find_program, output_tail,
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
    /// The arguments appended to the command to run only a selection in one root; `None` when
    /// the runner cannot narrow to it.
    pub select: fn(&Selected<'_>) -> Option<Vec<String>>,
    /// The arguments appended to the command to run the whole suite of one root but these
    /// test IDs; `None` when the runner cannot leave a test out (ADR-147).
    pub exclude: fn(&[String]) -> Option<Vec<String>>,
    /// How the runner lists its tests; `None` when it cannot.
    pub list: Option<Lister>,
    /// How a run's order, seed, and parallelism reach the runner (ADR-078).
    pub vary: &'static Varying,
    /// The directories it builds into, which no copy of the tree may share (ADR-080 §8).
    pub outputs: &'static [&'static str],
}

/// How a runner varies a run: its ordering, the arguments and environment a run's
/// [`RunOptions`] become, and where a finished run says the order its tests ran in.
pub struct Varying {
    pub ordering: RunOrdering,
    /// Appended to the command after the arguments that narrow it.
    pub args: fn(&RunOptions) -> Vec<String>,
    /// Added to the environment after the spec's own variables and the options' `env`.
    pub env: fn(&RunOptions) -> Vec<(String, String)>,
    pub executed: Executed,
}

/// Where a finished run says the order its tests ran in.
pub enum Executed {
    /// The report lists the tests in the order they ran.
    Report,
    /// Events in the captured output do, as test IDs relative to the root.
    Events(fn(&Captured) -> Vec<String>),
    /// Nothing does.
    Unknown,
}

/// The variation of the Python runners: the order of the test IDs they are given, the seed as
/// `PYTHONHASHSEED`, and the report's order.
pub static PYTHON: Varying = Varying {
    ordering: RunOrdering::Explicit,
    args: |_| Vec::new(),
    env: python_hash_seed,
    executed: Executed::Report,
};

/// `PYTHONHASHSEED` from a run's seed.
fn python_hash_seed(options: &RunOptions) -> Vec<(String, String)> {
    options
        .seed
        .map(|seed| ("PYTHONHASHSEED".to_owned(), (seed % (1 << 32)).to_string()))
        .into_iter()
        .collect()
}

/// The arguments of a runner that writes the Jest JSON report (`jest`, `vitest`) for a run's
/// options: the seed (in a signed 32-bit range, all they take) as `seed_flag`, `shuffle_flag`
/// for a shuffle, and `--maxWorkers`.
pub fn jest_report_variation(
    options: &RunOptions,
    seed_flag: &str,
    shuffle_flag: &str,
) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(seed) = options.seed {
        args.push(format!("{seed_flag}={}", seed % (1 << 31)));
    }
    if options.order == RunOrder::Shuffled {
        args.push(shuffle_flag.to_owned());
    }
    if let Some(workers) = options.parallelism {
        args.push(format!("--maxWorkers={workers}"));
    }
    args
}

/// What one root runs of a selection, relative to that root.
pub enum Selected<'a> {
    /// Test IDs, as the runner reports them.
    Tests(&'a [String]),
    /// Test files.
    Files(&'a [String]),
}

/// How a runner lists its tests without running them.
pub struct Lister {
    /// The command line for one root.
    pub command: fn(&Root) -> Vec<String>,
    /// The test IDs in its standard output, relative to the root.
    pub parse: fn(&str) -> Vec<String>,
}

/// A spec's `select` for runners that cannot narrow.
pub fn whole_suite(_: &Selected<'_>) -> Option<Vec<String>> {
    None
}

/// A spec's `exclude` for runners that cannot leave a test out of a run.
pub fn keeps_every_test(_: &[String]) -> Option<Vec<String>> {
    None
}

/// A spec's `select` for runners that take what they run as arguments: test files when
/// `files`, else test IDs; never the other.
pub fn as_arguments(selected: &Selected<'_>, files: bool) -> Option<Vec<String>> {
    match (selected, files) {
        (Selected::Files(paths), true) | (Selected::Tests(paths), false) => Some(paths.to_vec()),
        _ => None,
    }
}

/// A spec's `select` for runners that take test IDs and test files alike as arguments.
#[allow(
    clippy::unnecessary_wraps,
    reason = "every runner narrows through `RunnerSpec::select`; these narrow to anything"
)]
pub fn tests_or_files(selected: &Selected<'_>) -> Option<Vec<String>> {
    match selected {
        Selected::Tests(paths) | Selected::Files(paths) => Some(paths.to_vec()),
    }
}

/// A spec's `select` for runners that take test files as arguments, and no test IDs.
pub fn files(selected: &Selected<'_>) -> Option<Vec<String>> {
    as_arguments(selected, true)
}

/// The arguments `flag` and `pattern(ids)` for a selection of test IDs; files are not
/// selectable.
pub fn tests_flag(
    selected: &Selected<'_>,
    flag: &str,
    pattern: fn(&[String]) -> String,
) -> Option<Vec<String>> {
    match selected {
        Selected::Tests(ids) => Some(vec![flag.to_owned(), pattern(ids)]),
        Selected::Files(_) => None,
    }
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
    /// Whether the run is narrowed to a selection.
    pub narrowed: bool,
}

/// A finished run in one root.
pub struct Ran<'a> {
    pub root: &'a Root,
    pub captured: &'a Captured,
    /// The report file's text, when the runner wrote one.
    pub report: Option<String>,
}

/// A runner producer: `spec` run once per project root of its language.
#[derive(Clone)]
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
                install: toolchain::install(&self.id(), self.0.tool, &ctx.root),
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
        cupel_core::Runner::run(self, ctx, &TestSelection::All, &RunOptions::default())
            .map(|run| run.results)
    }
}

impl cupel_core::Runner for Runner {
    fn list(&self, ctx: &ProduceCtx) -> Result<Vec<String>> {
        let Some(lister) = &self.0.list else {
            return Err(Error::InvalidValue {
                what: "runner",
                value: self.0.id.to_owned(),
                expected: "a runner that lists its tests: tool.cargo-nextest, tool.go-test, tool.pytest",
            });
        };
        let mut ids = Vec::new();
        for (index, (dir, packages)) in suite_roots(ctx).into_iter().enumerate() {
            let root = self.root(ctx, index, &dir, packages, false)?;
            let command = (lister.command)(&root);
            let captured = self
                .process(&root, &command, &[], None)
                .run(&ctx.cancel)?;
            ids.extend(
                (lister.parse)(&captured.stdout)
                    .into_iter()
                    .map(|id| under(&dir, &id, ID_SEPARATOR)),
            );
        }
        Ok(ids)
    }

    fn run(
        &self,
        ctx: &ProduceCtx,
        selection: &TestSelection,
        options: &RunOptions,
    ) -> Result<TestRun> {
        let cancel = options
            .timeout
            .map_or_else(|| ctx.cancel.clone(), |limit| ctx.cancel.child(limit));
        let mut invocations = Vec::new();
        let mut tests = Vec::new();
        let mut executed = Vec::new();
        for (index, (dir, packages), narrowing) in self.narrowed(ctx, selection)? {
            let root = self.root(ctx, index, &dir, packages, !narrowing.is_empty())?;
            let run = Run {
                narrowing: &narrowing,
                options,
                cancel: &cancel,
            };
            let (invocation, found, order) = self.run_root(ctx, &root, &dir, &run)?;
            invocations.push(invocation);
            executed.extend(order.into_iter().map(|id| under(&dir, &id, ID_SEPARATOR)));
            tests.extend(found.into_iter().map(|test| placed(test, &dir)));
        }
        let reports_order = !matches!(self.0.vary.executed, Executed::Unknown);
        Ok(TestRun {
            results: TestResults::new(self.id(), invocations, tests, &ctx.ctx.root),
            executed: reports_order.then_some(executed),
        })
    }

    fn commands(&self, ctx: &ProduceCtx, selection: &TestSelection) -> Option<Vec<RunnerCommand>> {
        let narrowed = self.narrowed(ctx, selection).ok()?;
        Some(
            narrowed
                .into_iter()
                .map(|(_, (dir, packages), narrowing)| {
                    let root = toolchain::Root::new(ctx.ctx.root.join(&dir), Vec::new(), packages);
                    let base = toolchain::runner_command(&self.id(), &root)
                        .unwrap_or_else(|| self.0.tool.to_owned());
                    let words: Vec<String> = narrowing.iter().map(|word| quote(word)).collect();
                    RunnerCommand {
                        dir,
                        command: std::iter::once(base)
                            .chain(words)
                            .collect::<Vec<_>>()
                            .join(" "),
                    }
                })
                .collect(),
        )
    }

    fn ordering(&self) -> RunOrdering {
        self.0.vary.ordering
    }

    fn outputs(&self) -> &'static [&'static str] {
        self.0.outputs
    }
}

/// How one root's run differs from the whole suite's.
struct Run<'a> {
    /// Arguments appended to the command.
    narrowing: &'a [String],
    options: &'a RunOptions,
    cancel: &'a Cancellation,
}

/// A root to run in: its index, its directory and packages, and the arguments that narrow it.
type Narrowed = (usize, (String, Vec<String>), Vec<String>);

impl Runner {
    /// The roots `selection` reaches, each with the arguments that narrow its run; an error
    /// when the runner cannot narrow to it.
    fn narrowed(&self, ctx: &ProduceCtx, selection: &TestSelection) -> Result<Vec<Narrowed>> {
        let roots = suite_roots(ctx);
        let dirs: Vec<String> = roots.iter().map(|(dir, _)| dir.clone()).collect();
        let mut narrowed = Vec::new();
        for (index, root) in roots.into_iter().enumerate() {
            let narrowing = match selection {
                TestSelection::All => Vec::new(),
                TestSelection::Tests(ids) => {
                    let own = owned(ids, &root.0, &dirs, ID_SEPARATOR);
                    if own.is_empty() {
                        continue;
                    }
                    self.select(&Selected::Tests(&own))?
                }
                TestSelection::Files(files) => {
                    let own = owned(files, &root.0, &dirs, "/");
                    if own.is_empty() {
                        continue;
                    }
                    self.select(&Selected::Files(&own))?
                }
                TestSelection::Except(ids) => {
                    let own = owned(ids, &root.0, &dirs, ID_SEPARATOR);
                    if own.is_empty() {
                        Vec::new()
                    } else {
                        self.exclude(&own)?
                    }
                }
            };
            narrowed.push((index, root, narrowing));
        }
        Ok(narrowed)
    }

    fn exclude(&self, ids: &[String]) -> Result<Vec<String>> {
        (self.0.exclude)(ids).ok_or_else(|| Error::InvalidValue {
            what: "test selection",
            value: self.0.id.to_owned(),
            expected: "a runner that can leave a test out of a run",
        })
    }

    fn select(&self, selected: &Selected<'_>) -> Result<Vec<String>> {
        (self.0.select)(selected).ok_or_else(|| Error::InvalidValue {
            what: "test selection",
            value: self.0.id.to_owned(),
            expected: "a selection the runner can narrow its suite to",
        })
    }

    /// The `index`th project root, at `dir`, with its report area under `.cupel/`.
    fn root(
        &self,
        ctx: &ProduceCtx,
        index: usize,
        dir: &str,
        packages: Vec<String>,
        narrowed: bool,
    ) -> Result<Root> {
        let absolute = ctx.ctx.root.join(dir);
        let absolute = absolute.canonicalize().unwrap_or(absolute);
        let area = ctx.writer.create_dir(
            &ctx.writer
                .cache_dir()
                .join("runners")
                .join(self.0.id)
                .join(index.to_string()),
        )?;
        Ok(Root {
            program: self
                .program(&absolute)
                .unwrap_or_else(|| self.0.tool.to_owned()),
            dir: absolute,
            packages,
            report: area.join("report"),
            config: area.join("config.toml"),
            narrowed,
        })
    }

    /// Runs the suite, narrowed and varied by `run`, in `root` (at `dir`) and reads what it
    /// reported, relative to that root, with the IDs in the order they ran when it says. A run
    /// its own timeout ended is an incomplete invocation.
    fn run_root(
        &self,
        ctx: &ProduceCtx,
        root: &Root,
        dir: &str,
        run: &Run<'_>,
    ) -> Result<(RunnerInvocation, Vec<TestResult>, Vec<String>)> {
        let _ = std::fs::remove_file(&root.report);
        let vary = self.0.vary;
        let command: Vec<String> = (self.0.command)(root)
            .into_iter()
            .chain(run.narrowing.iter().cloned())
            .chain((vary.args)(run.options))
            .chain(run.options.args.iter().cloned())
            .collect();
        let env: Vec<(String, String)> = run
            .options
            .env
            .iter()
            .cloned()
            .chain((vary.env)(run.options))
            .collect();
        let process = self.process(root, &command, &env, run.options.memory);
        let captured = match process.run(run.cancel) {
            Err(Error::Cancelled) if !ctx.cancel.is_cancelled() => {
                let limit = run.options.timeout.unwrap_or_default();
                let invocation = timed_out(dir, process.command_line(), limit);
                return Ok((invocation, Vec::new(), Vec::new()));
            }
            ran => ran?,
        };
        let found = (self.0.read)(&Ran {
            root,
            captured: &captured,
            report: std::fs::read_to_string(&root.report).ok(),
        });
        let invocation = finished(
            dir,
            process.command_line(),
            &captured,
            found.as_deref(),
            self.0.completed_codes,
        );
        let found = found.unwrap_or_default();
        let order = match vary.executed {
            Executed::Report => found.iter().map(|test| test.id.clone()).collect(),
            Executed::Events(order) => order(&captured),
            Executed::Unknown => Vec::new(),
        };
        Ok((invocation, found, order))
    }

    /// The process that runs `command` in `root`, with the spec's environment and `env`, and
    /// the run's memory budget (ADR-148) when it has one.
    fn process(
        &self,
        root: &Root,
        command: &[String],
        env: &[(String, String)],
        memory: Option<u64>,
    ) -> Process {
        let (program, args) = command.split_first().unwrap_or((&root.program, &[]));
        let mut process = Process::new(program, &root.dir).args(args.iter().cloned());
        if let Some(bytes) = memory {
            process = process.memory(bytes);
        }
        for (key, value) in self.0.env {
            process = process.env(key, value);
        }
        for (key, value) in env {
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

/// The invocation in `dir` of `command`, which finished as `captured` reports and reported
/// `found` (`None`: no readable report). It completed when it reported its tests and either
/// exited with one of `completed_codes` or reported a failure.
pub(crate) fn finished(
    dir: &str,
    command: Vec<String>,
    captured: &Captured,
    found: Option<&[TestResult]>,
    completed_codes: &[i32],
) -> RunnerInvocation {
    let completed = found.is_some_and(|found| {
        captured
            .exit_code
            .is_some_and(|code| completed_codes.contains(&code))
            || found.iter().any(|test| test.outcome.is_failure())
    });
    RunnerInvocation {
        dir: dir.to_owned(),
        command,
        exit_code: captured.exit_code,
        completed,
        output: (!completed)
            .then(|| output_tail(&format!("{}\n{}", captured.stdout, captured.stderr)))
            .flatten(),
    }
}

/// The invocation of a run that its own timeout ended after `limit`.
fn timed_out(dir: &str, command: Vec<String>, limit: Duration) -> RunnerInvocation {
    RunnerInvocation {
        dir: dir.to_owned(),
        command,
        exit_code: None,
        completed: false,
        output: Some(format!("timed out after {} ms", limit.as_millis())),
    }
}

/// The entries of `paths` (test IDs joined to their root by `separator`, or files) that belong
/// to the root at `dir` among `dirs`, made relative to it: the deepest root naming them, `.`
/// for those no other root names.
fn owned(paths: &[String], dir: &str, dirs: &[String], separator: &str) -> Vec<String> {
    let owner = |path: &str| {
        dirs.iter()
            .filter(|candidate| *candidate != ".")
            .filter(|candidate| path.starts_with(&format!("{candidate}{separator}")))
            .max_by_key(|candidate| candidate.len())
            .map_or(".", String::as_str)
    };
    paths
        .iter()
        .filter(|path| owner(path) == dir)
        .map(|path| {
            if dir == "." {
                path.clone()
            } else {
                path[dir.len() + separator.len()..].to_owned()
            }
        })
        .collect()
}

/// `word` as one shell word.
pub(crate) fn quote(word: &str) -> String {
    let plain = word
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_./:=+,@%".contains(c));
    if plain && !word.is_empty() {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
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

/// Ready when `program` is on `PATH`, else missing, installed as `install` says.
pub(crate) fn on_path(program: &str, install: Install) -> Availability {
    if find_program(program).is_some() {
        Availability::Ready
    } else {
        Availability::MissingTool {
            name: ToolId::from(program),
            install: Some(install),
        }
    }
}

/// A tool's run in the project root `dir`: failed, saying why, when `failed` says so.
pub(crate) fn root_run(dir: String, failed: Option<String>) -> RootRun {
    RootRun {
        dir,
        status: if failed.is_some() {
            RunStatus::Failed
        } else {
            RunStatus::Ran
        },
        detail: failed,
    }
}

/// The project roots a runner runs the suite in: none when `ProjectInfo` found no project of
/// the language, since there is no suite to run (a `Cargo.toml`-less crate cannot be tested),
/// else [`roots`].
pub(crate) fn suite_roots(ctx: &ProduceCtx) -> Vec<(String, Vec<String>)> {
    match ctx.upstream.get::<ProjectInfo>() {
        Some(info) if info.roots.is_empty() => Vec::new(),
        _ => roots(ctx),
    }
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
pub(crate) fn placed(mut test: TestResult, dir: &str) -> TestResult {
    test.id = under(dir, &test.id, ID_SEPARATOR);
    test.file = test.file.map(|file| under(dir, &file, "/"));
    test
}

/// What separates a root from a test ID of that root (ADR-075).
pub(crate) const ID_SEPARATOR: &str = ": ";

/// `path` (a test ID or file) of the root at `dir`, joined to the root with `separator` so it
/// is unique across roots; unchanged for the repository root.
pub(crate) fn under(dir: &str, path: &str, separator: &str) -> String {
    if dir == "." {
        path.to_owned()
    } else {
        format!("{dir}{separator}{path}")
    }
}
