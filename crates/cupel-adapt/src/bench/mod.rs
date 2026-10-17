// ABOUTME: The bench producers (ADR-106): one `BenchSamples` producer per bench tool, measuring each project root at
// ABOUTME: head and, under a range, in an export of the base commit, with as many samples per side as the depth asks.
//! The bench producers.
//!
//! Each bench tool is one [`BenchTool`] spec in its own module, registered as a [`Producer`] of
//! [`BenchSamples`] for each language it serves. A producer measures every project root of its
//! language in two places: when the run has a range (`Focus::base`), first in a `git archive`
//! export of the base commit ([`crate::tree::export`]), then in the working tree. Both sides
//! are measured in one run, one after the other, so they share the machine's conditions. Every
//! sample is a cost per iteration (`ns`, `bytes`, `allocs`); nothing is written in the tree:
//! each tool's reports, storage, and build directories go under `.cupel/cache/bench/`.

mod criterion;
mod go;
mod hyperfine;
mod pytest;

use std::fs;
use std::path::{Path, PathBuf};

use cupel_core::syntax::{SymbolDef, Symbols};
use cupel_core::{
    Availability, BenchBase, BenchSamples, Benchmark, Captured, CostClass, Ctx, Error, EventSink,
    FactKind, LangId, Langs, Process, ProduceCtx, Producer, ProducerId, Result, RootRun, RunStatus,
    Scope, ToolId, ToolProbe, output_tail,
};

use crate::runner::{installed, probe, roots};
use crate::{toolchain, tree};

/// What differs between bench tools.
pub struct BenchTool {
    /// The producer ID, such as `tool.go-bench`.
    pub id: &'static str,
    pub langs: &'static [&'static str],
    /// The program it runs.
    pub program: &'static str,
    /// The command that prints its version.
    pub version: &'static [&'static str],
    pub needs: &'static [FactKind],
    pub wants: &'static [FactKind],
    /// Whether it measures each project root (else the repository root once).
    pub per_root: bool,
    /// Whether it can run here; [`installed_program`] for most tools.
    pub availability: fn(&BenchTool, &Ctx, &LangId) -> Availability,
    /// Measures one root on one side.
    pub measure: fn(&Side<'_>) -> Result<Measured>,
}

/// One project root to measure, on one side.
pub struct Side<'a> {
    pub ctx: &'a ProduceCtx,
    /// The repository root on this side: the working tree, or the base commit's export.
    pub tree: &'a Path,
    /// The project root, relative to the repository root.
    pub dir: &'a str,
    /// The root's package patterns (`<packages>`).
    pub packages: &'a [String],
    /// Samples to take of each benchmark.
    pub samples: u32,
    /// An empty directory for this root's reports and the tool's storage.
    pub scratch: PathBuf,
    /// A directory the tool may keep builds in across runs, per side.
    pub build: PathBuf,
    /// The language's symbols at head, to place benchmarks; `None` on the base side.
    pub symbols: Option<&'a Symbols>,
}

impl Side<'_> {
    /// The project root, absolute.
    pub fn root(&self) -> PathBuf {
        self.tree.join(self.dir)
    }

    /// `path`, relative to the project root, made relative to the repository root.
    pub fn file(&self, path: &str) -> String {
        if self.dir == "." {
            path.to_owned()
        } else {
            format!("{}/{path}", self.dir)
        }
    }

    /// The symbol named `name` in a file `wanted` accepts, when exactly one is.
    pub fn locate(&self, name: &str, wanted: impl Fn(&str) -> bool) -> Option<&SymbolDef> {
        let mut found = self
            .symbols?
            .symbols
            .iter()
            .filter(|symbol| symbol.name == name && wanted(&symbol.file));
        let first = found.next()?;
        found.next().is_none().then_some(first)
    }
}

/// What measuring one root found: its benchmarks, and why its run failed when it did.
#[derive(Debug, Default)]
pub struct Measured {
    pub benchmarks: Vec<Benchmark>,
    pub failure: Option<String>,
}

impl Measured {
    /// A run that measured nothing because of `detail`.
    pub fn failed(detail: String) -> Self {
        Self {
            benchmarks: Vec::new(),
            failure: Some(detail),
        }
    }

    /// `benchmarks`, failed with the tail of `captured`'s output unless it exited 0.
    pub fn of(benchmarks: Vec<Benchmark>, captured: &Captured) -> Self {
        Self {
            benchmarks,
            failure: (captured.exit_code != Some(0)).then(|| tail(captured)),
        }
    }
}

/// Runs `process` for `side`: what it printed, or why it could not run. Cancellation ends the
/// whole run.
pub fn execute(
    side: &Side<'_>,
    process: &Process,
) -> Result<std::result::Result<Captured, String>> {
    match process.run(&side.ctx.cancel) {
        Ok(captured) => Ok(Ok(captured)),
        Err(Error::Cancelled) => Err(Error::Cancelled),
        Err(error) => Ok(Err(error.to_string())),
    }
}

/// The last lines of what `captured` printed, standard error first.
pub fn tail(captured: &Captured) -> String {
    output_tail(&format!("{}\n{}", captured.stdout, captured.stderr))
        .unwrap_or_else(|| format!("exited with {:?}", captured.exit_code))
}

/// The samples each benchmark gets on each side at `depth` (ADR-106 §4).
pub fn samples_for(depth: CostClass) -> u32 {
    match depth {
        CostClass::Quick | CostClass::Standard => 5,
        CostClass::Deep => 10,
        CostClass::Exhaustive => 20,
    }
}

/// Ready when the tool's program is on `PATH`, else missing with the table's install command.
pub fn installed_program(tool: &BenchTool, ctx: &Ctx, _: &LangId) -> Availability {
    if installed(tool.program, false, &ctx.root).is_some() {
        Availability::Ready
    } else {
        missing(tool, tool.program, ctx)
    }
}

/// `name` is missing, with the table's install command or toolchain hint for `tool`.
pub fn missing(tool: &BenchTool, name: &str, ctx: &Ctx) -> Availability {
    Availability::MissingTool {
        name: ToolId::from(name),
        install: toolchain::install(&ProducerId::from(tool.id), name, &ctx.root),
    }
}

/// Measures a language's benchmarks with one bench tool (ADR-106).
pub struct BenchProducer(&'static BenchTool, LangId);

inventory::submit!(crate::ToolProducer {
    id: "tool.go-bench",
    register: |registry| register(registry, &go::GO_BENCH),
});

inventory::submit!(crate::ToolProducer {
    id: "tool.criterion",
    register: |registry| register(registry, &criterion::CRITERION),
});

inventory::submit!(crate::ToolProducer {
    id: "tool.pytest-benchmark",
    register: |registry| register(registry, &pytest::PYTEST_BENCHMARK),
});

inventory::submit!(crate::ToolProducer {
    id: "tool.hyperfine",
    register: |registry| register(registry, &hyperfine::HYPERFINE),
});

fn register(registry: &mut cupel_core::ProducerRegistry, tool: &'static BenchTool) {
    for lang in tool.langs {
        registry.register::<BenchSamples, _>(BenchProducer(tool, LangId::from(*lang)));
    }
}

impl Producer<BenchSamples> for BenchProducer {
    fn id(&self) -> ProducerId {
        ProducerId::from(self.0.id)
    }

    fn languages(&self) -> Langs {
        Langs::Only(vec![self.1.clone()])
    }

    fn needs(&self) -> &[FactKind] {
        self.0.needs
    }

    fn wants(&self) -> &[FactKind] {
        self.0.wants
    }

    fn cost(&self) -> CostClass {
        CostClass::Deep
    }

    fn availability(&self, ctx: &Ctx) -> Availability {
        (self.0.availability)(self.0, ctx, &self.1)
    }

    fn detected(&self, ctx: &Ctx) -> bool {
        toolchain::detects(&self.id(), &ctx.root)
    }

    fn tool(&self, ctx: &Ctx) -> Option<ToolProbe> {
        Some(probe(self.0.program, false, self.0.version, &ctx.root))
    }

    fn produce(&self, ctx: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<BenchSamples> {
        let run = Run {
            ctx,
            tool: self.0,
            samples: samples_for(ctx.focus.depth),
            roots: if self.0.per_root {
                roots(ctx)
            } else {
                vec![(".".to_owned(), Vec::new())]
            },
            cache: ctx
                .writer
                .cache_dir()
                .join("bench")
                .join(self.0.id.trim_start_matches("tool.")),
        };
        let base = match &ctx.focus.base {
            Some(commit) => Some(run.base(commit)?),
            None => None,
        };
        let symbols = ctx.upstream.get::<Symbols>();
        let (runs, benchmarks) = run.side(&ctx.ctx.root, "head", symbols)?;
        Ok(BenchSamples {
            tool: self.id(),
            samples: run.samples,
            runs,
            benchmarks,
            base,
        })
    }
}

/// One producer run: the roots to measure, how many samples, and where to keep what it writes.
struct Run<'a> {
    ctx: &'a ProduceCtx,
    tool: &'static BenchTool,
    samples: u32,
    roots: Vec<(String, Vec<String>)>,
    cache: PathBuf,
}

impl Run<'_> {
    /// The base commit's measurements, in an export of it; a failed export fails every root.
    fn base(&self, commit: &str) -> Result<BenchBase> {
        let scratch = tempfile::tempdir().map_err(|error| io_error(&error))?;
        let exported = scratch.path().join("tree");
        let (runs, benchmarks) = match tree::export(
            &self.ctx.ctx.root,
            commit,
            scratch.path(),
            &exported,
            &self.ctx.cancel,
        ) {
            Ok(()) => self.side(&exported, "base", None)?,
            Err(Error::Cancelled) => return Err(Error::Cancelled),
            Err(error) => (
                vec![RootRun {
                    dir: ".".to_owned(),
                    status: RunStatus::Failed,
                    detail: Some(error.to_string()),
                }],
                Vec::new(),
            ),
        };
        Ok(BenchBase {
            commit: commit.to_owned(),
            runs,
            benchmarks,
        })
    }

    /// Every root measured in `tree`, the side `name`.
    fn side(
        &self,
        tree: &Path,
        name: &str,
        symbols: Option<&Symbols>,
    ) -> Result<(Vec<RootRun>, Vec<Benchmark>)> {
        let mut runs = Vec::new();
        let mut benchmarks = Vec::new();
        for (index, (dir, packages)) in self.roots.iter().enumerate() {
            self.ctx.cancel.checkpoint()?;
            let scratch = self.cache.join(name).join(index.to_string());
            if scratch.exists() {
                fs::remove_dir_all(&scratch).map_err(|error| io_error(&error))?;
            }
            fs::create_dir_all(&scratch).map_err(|error| io_error(&error))?;
            let measured = (self.tool.measure)(&Side {
                ctx: self.ctx,
                tree,
                dir,
                packages,
                samples: self.samples,
                scratch,
                build: self.cache.join(format!("{name}-build")),
                symbols,
            })?;
            runs.push(RootRun {
                dir: dir.clone(),
                status: if measured.failure.is_some() {
                    RunStatus::Failed
                } else {
                    RunStatus::Ran
                },
                detail: measured.failure,
            });
            benchmarks.extend(measured.benchmarks);
        }
        Ok((runs, benchmarks))
    }
}

fn io_error(error: &std::io::Error) -> Error {
    Error::Process {
        program: "bench".to_owned(),
        message: error.to_string(),
    }
}
