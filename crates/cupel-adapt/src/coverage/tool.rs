// ABOUTME: What every coverage producer shares (ADR-079): a `CoverageSpec` names the tool and its default report;
// ABOUTME: `CoverageTool` ingests a fresh report or runs the configured command per project root.
use std::fs;
use std::path::Path;

use cupel_core::coverage::{CoverageMap, Listing};
use cupel_core::{
    Action, ActionKind, Availability, CheckId, CostClass, Ctx, Error, Event, EventSink, Evidence,
    FactKind, Finding, FindingId, LangId, Langs, Process, ProduceCtx, Producer, ProducerId, Result,
    Scope, Severity, Tier, ToolId, ToolProbe, output_tail,
};
use serde_json::json;

use super::format::{parse, placed};
use crate::fresh::{
    REPORT, SOURCES, Sources, Tree, cache_dir, entries, entry_name, load, skip_dirs, store,
};
use crate::runner::{installed, probe, roots};
use crate::toolchain;
use crate::tree::canonical;

/// The `[lang.<id>]` key holding the coverage command (design §6.7).
const KEY: &str = "coverage";
/// Where a command's report goes: the placeholder the producer fills with a cache path.
const OUT: &str = "{out}";
/// The kind of cache the producers keep under `.cupel/cache/`.
const CACHE: &str = "coverage";

/// One coverage producer: everything that differs between `cargo llvm-cov`, `go test`, and the
/// rest.
pub struct CoverageSpec {
    /// The producer ID, such as `tool.go-coverprofile`; the toolchain table lists it.
    pub id: &'static str,
    /// The languages it serves.
    pub langs: &'static [&'static str],
    /// The program that must be installed, such as `cargo-llvm-cov`.
    pub tool: &'static str,
    /// Whether `node_modules/.bin/<tool>` in the root counts as the tool.
    pub node_bin: bool,
    /// The command that prints the tool's version.
    pub version: &'static [&'static str],
    /// Where the tool writes its report, relative to the root, when the command names no
    /// `{out}`.
    pub default_report: Option<&'static str>,
    /// Turns the report's file names into paths below the root it ran in, for tools that name
    /// files otherwise (Go's import paths); identity for the rest.
    pub resolve: fn(CoverageMap, &Path) -> CoverageMap,
    /// Which files the report of the configured command lists (ADR-152): every file with
    /// executable code, or, as coverage.py's without a source, only the files the suite loaded.
    pub listing: fn(&str) -> Listing,
}

/// A coverage producer: `spec` run, or its report ingested, once per project root.
pub struct CoverageTool(pub &'static CoverageSpec);

/// Leaves a report's paths as the tool wrote them.
pub fn as_written(map: CoverageMap, _: &Path) -> CoverageMap {
    map
}

/// The report lists every file with executable code, whatever the command.
pub fn every_file(_: &str) -> Listing {
    Listing::All
}

impl Producer<CoverageMap> for CoverageTool {
    fn id(&self) -> ProducerId {
        ProducerId::from(self.0.id)
    }

    fn languages(&self) -> Langs {
        Langs::Only(
            self.0
                .langs
                .iter()
                .map(|lang| LangId::from(*lang))
                .collect(),
        )
    }

    fn needs(&self) -> &[FactKind] {
        &[FactKind::ProjectInfo]
    }

    fn cost(&self) -> CostClass {
        CostClass::Standard
    }

    /// `Ready` with a fresh report to ingest; else `unconfigured` without a coverage command;
    /// else whether the tool is installed (ADR-079).
    fn availability(&self, ctx: &Ctx) -> Availability {
        if self.ingestible(ctx) {
            return Availability::Ready;
        }
        if self.command(ctx, None).is_none() {
            return Availability::Unconfigured {
                key: format!("lang.{}.{KEY}", self.0.langs[0]),
            };
        }
        if installed(self.0.tool, self.0.node_bin, &ctx.root).is_some() {
            Availability::Ready
        } else {
            Availability::MissingTool {
                name: ToolId::from(self.0.tool),
                install: toolchain::install(&self.id(), self.0.tool, &ctx.root),
            }
        }
    }

    /// Detected with a runner the toolchain table pairs it with: the pinned runner when one is
    /// pinned, else one whose rules hold; a producer with one command for every runner always is.
    fn detected(&self, ctx: &Ctx) -> bool {
        let companions = toolchain::companions(&self.id());
        if companions.is_empty() {
            return true;
        }
        let pinned = self
            .0
            .langs
            .iter()
            .find_map(|lang| ctx.pin(FactKind::TestResults, &LangId::from(*lang)));
        companions.iter().any(|runner| match pinned {
            Some(pin) => {
                runner.as_str() == pin || runner.as_str().strip_prefix("tool.") == Some(pin)
            }
            None => toolchain::detects(runner, &ctx.root),
        })
    }

    /// The coverage tool, probed as a runner is: it is what `doctor` requires (ADR-079).
    fn tool(&self, ctx: &Ctx) -> Option<ToolProbe> {
        let spec = self.0;
        probe(spec.tool, spec.node_bin, spec.version, &ctx.root).into()
    }

    fn produce(
        &self,
        ctx: &ProduceCtx,
        _: &Scope,
        sink: &mut dyn EventSink,
    ) -> Result<CoverageMap> {
        // Every root runs the language's one configured command, so each root's map has the
        // same listing; the merged map keeps it.
        let mut files = Vec::new();
        let mut listing = Listing::All;
        for (dir, _) in roots(ctx) {
            let skip_dirs = self.skip_dirs(ctx, &dir);
            let tree = Tree {
                repo: &ctx.ctx.root,
                dir: &dir,
                skip_dirs: &skip_dirs,
                exclude: &ctx.ctx.exclude,
            };
            let map = self.root(ctx, &tree, sink)?;
            listing = map.listing;
            files.extend(map.files);
        }
        Ok(CoverageMap::new(files).listed(listing))
    }
}

impl CoverageTool {
    /// The coverage of one project root: its fresh cached report, else the fresh report the
    /// tool left at its default location, else a new run of the configured command.
    fn root(
        &self,
        ctx: &ProduceCtx,
        tree: &Tree<'_>,
        sink: &mut dyn EventSink,
    ) -> Result<CoverageMap> {
        let entry = cache_dir(&ctx.ctx.root, CACHE, self.0.id).join(entry_name(tree.dir));
        let command = self.command(&ctx.ctx, Some(&ctx.config));
        let listing = (self.0.listing)(command.as_deref().unwrap_or_default());
        if let Some(map) = self.cached(&entry, tree, listing) {
            return Ok(map);
        }
        let root = canonical(&tree.root());
        if let Some(map) = self.existing(&root, tree, listing) {
            return Ok(map);
        }
        self.run(ctx, command, &root, tree, sink)
    }

    /// The report in the cache entry `entry`, when it was made for this root and is fresh.
    fn cached(&self, entry: &Path, tree: &Tree<'_>, listing: Listing) -> Option<CoverageMap> {
        let (sources, written, report) = load(entry)?;
        let fresh =
            sources.dir == tree.dir && sources.fresh(tree.repo, tree.exclude, written, &report);
        fresh.then(|| self.read(&report, &sources.ran_in, &tree.root(), tree.dir, listing))?
    }

    /// The report the tool left at its default location in `root`, when it is newer than
    /// every other file there (the developer ran the tool).
    fn existing(&self, root: &Path, tree: &Tree<'_>, listing: Listing) -> Option<CoverageMap> {
        let report = tree.root().join(self.0.default_report?);
        if !tree.is_newest(&report) {
            return None;
        }
        self.read(&fs::read(&report).ok()?, root, root, tree.dir, listing)
    }

    /// `report`, written by the tool running in `ran_in`, with paths relative to the repository
    /// and the `listing` of the command that wrote it. Names the report resolves through project
    /// files (Go import paths through `go.mod`) are resolved in `root`, the project root here: a
    /// committed report's `ran_in` is on the machine that made it.
    fn read(
        &self,
        report: &[u8],
        ran_in: &Path,
        root: &Path,
        dir: &str,
        listing: Listing,
    ) -> Option<CoverageMap> {
        let map = parse(std::str::from_utf8(report).ok()?).ok()?;
        let map = (self.0.resolve)(map, root);
        Some(placed(map, &[ran_in, &canonical(ran_in)], dir).listed(listing))
    }

    /// Runs the configured command in `root` and caches its report with the record of the
    /// tree it covers; a run that fails or leaves no readable report is reported and fails.
    fn run(
        &self,
        ctx: &ProduceCtx,
        command: Option<String>,
        root: &Path,
        tree: &Tree<'_>,
        sink: &mut dyn EventSink,
    ) -> Result<CoverageMap> {
        let command = command.ok_or_else(|| Error::Process {
            program: self.0.tool.to_owned(),
            message: format!("no `[lang.<id>] {KEY}` command is configured"),
        })?;
        let listing = (self.0.listing)(&command);
        let area = ctx.writer.create_dir(
            &ctx.writer
                .cache_dir()
                .join(CACHE)
                .join(self.0.id)
                .join(entry_name(tree.dir)),
        )?;
        let out = area.join(REPORT);
        let _ = fs::remove_file(area.join(SOURCES));
        let _ = fs::remove_file(&out);
        let line = command.replace(OUT, &quoted(&out));
        let captured = Process::new("sh", root)
            .args(["-c", line.as_str()])
            .merged()
            .run(&ctx.cancel)?;
        let written = if command.contains(OUT) {
            Some(out.clone())
        } else {
            self.0.default_report.map(|report| root.join(report))
        };
        let report = written.and_then(|path| fs::read(path).ok());
        let map = report
            .as_deref()
            .filter(|_| captured.exit_code == Some(0))
            .and_then(|report| self.read(report, root, root, tree.dir, listing));
        let (Some(report), Some(map)) = (report, map) else {
            let failure = Failure {
                producer: self.0.id,
                dir: tree.dir,
                command: &line,
                exit_code: captured.exit_code,
                output: &captured.stdout,
            };
            sink.emit(Event::Finding(Box::new(failure.finding())));
            return Err(Error::Process {
                program: "sh".to_owned(),
                message: failure.reason(),
            });
        };
        let sources = Sources::record(tree, root, &report);
        store(&ctx.writer, &area, &report, &sources)?;
        Ok(map)
    }

    /// The configured coverage command: the language's own `[lang.<id>] coverage` when
    /// `config` is that table, else the first language it serves that configures one.
    fn command(
        &self,
        ctx: &Ctx,
        config: Option<&serde_json::Map<String, serde_json::Value>>,
    ) -> Option<String> {
        let own = config.and_then(|table| table.get(KEY)?.as_str());
        own.or_else(|| {
            self.0
                .langs
                .iter()
                .find_map(|lang| ctx.lang.get(&LangId::from(*lang))?.get(KEY)?.as_str())
        })
        .map(str::to_owned)
    }

    /// The directories skipped below the root `dir` when hashing it: the generated ones its
    /// `ProjectInfo` root names, and the one the tool writes its default report in.
    fn skip_dirs(&self, ctx: &ProduceCtx, dir: &str) -> Vec<String> {
        let output = self
            .0
            .default_report
            .and_then(|report| report.split_once('/'))
            .map(|(dir, _)| dir);
        skip_dirs(ctx, dir, output.as_slice())
    }

    /// Whether a fresh cached report can be ingested: this producer has cache entries, and
    /// every one is fresh.
    fn ingestible(&self, ctx: &Ctx) -> bool {
        let entries = entries(&cache_dir(&ctx.root, CACHE, self.0.id));
        !entries.is_empty()
            && entries.iter().all(|entry| {
                load(entry).is_some_and(|(sources, written, report)| {
                    sources.fresh(&ctx.root, &ctx.exclude, written, &report)
                })
            })
    }
}

/// `path` quoted for `sh`.
fn quoted(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', r"'\''"))
}

/// A coverage run that produced no coverage.
struct Failure<'a> {
    producer: &'a str,
    dir: &'a str,
    command: &'a str,
    exit_code: Option<i32>,
    output: &'a str,
}

impl Failure<'_> {
    fn reason(&self) -> String {
        match self.exit_code {
            Some(0) => format!("`{}` left no readable coverage report", self.command),
            Some(code) => format!("`{}` exited with {code}", self.command),
            None => format!("`{}` was ended by a signal", self.command),
        }
    }

    /// The `coverage.run-failed` finding: a red or broken suite has no coverage (ADR-079).
    fn finding(&self) -> Finding {
        let check = CheckId::from("coverage");
        let kind = "coverage.run-failed";
        let id = FindingId::for_subject(&check, kind, &format!("{}:{}", self.producer, self.dir));
        let evidence = json!({
            "producer": self.producer,
            "dir": self.dir,
            "command": self.command,
            "exit_code": self.exit_code,
            "output": output_tail(self.output),
        });
        Finding {
            suppress: format!("cupel suppress {id} --reason '…'"),
            repro: format!("cd {} && {}", self.dir, self.command),
            id,
            check,
            kind: kind.to_owned(),
            severity: Severity::Warn,
            confidence: 1.0,
            tier: Tier::try_from(2).expect("tier 2 is valid"),
            symbol: None,
            location: None,
            title: format!("No coverage for `{}`: {}", self.dir, self.reason()),
            detail: None,
            evidence: Evidence(evidence.as_object().cloned().unwrap_or_default()),
            action: Action {
                kind: ActionKind::FixBug,
                priority: 1,
                summary: "Make the test suite pass under the coverage command; a red or broken \
                          suite has no coverage."
                    .to_owned(),
                hints: serde_json::Map::new(),
            },
            new_since_baseline: false,
            suppressed: false,
        }
    }
}
