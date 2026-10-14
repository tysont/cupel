// ABOUTME: The `tests --impacted` verb (design §6.5, ADR-077): the tests a scope impacts, chosen by per-test coverage
// ABOUTME: maps where a language has one and by import-graph impact where it does not (ADR-006), with commands.
mod impact;

use cupel_core::config::Config;
use cupel_core::git::Git;
use cupel_core::syntax::{ModuleGraph, NormTree, Symbols, TestConventions, TestFrames};
use cupel_core::{
    Availability, Ctx, Envelope, EventSink, ExitCode, FactKind, ImpactBasis, ImpactedTest, Job,
    Lang, PerTestMap, ProduceCtx, ProducerId, ProducerRegistry, Production, Scheduler, ScopeInfo,
    ScopeRequest, Status, Support, SupportMatrix, TestCommand, TestSelection, TestsReport,
    select_symbols,
};

use self::impact::Regions;
use crate::catalog::Catalog;
use crate::check;
use crate::cli::ScopeArgs;
use crate::error::CupelError;
use crate::run::{Invocation, Run};

/// What `tests --impacted` reads to choose tests per test, where a language can produce it.
pub const IMPACT_FACT: FactKind = FactKind::PerTestMap;

/// The verb's name as `describe matrix` lists it.
pub const IMPACTED: &str = "tests --impacted";

/// `cupel tests --impacted`: the impacted tests of the scope `args` name, and how to run them.
pub fn verb(
    invocation: &Invocation<'_>,
    impacted: bool,
    args: &ScopeArgs,
    events: &mut dyn EventSink,
) -> Result<Envelope, CupelError> {
    if !impacted {
        return Err(CupelError::bad_argument(
            "--impacted",
            None,
            "`cupel tests` lists the tests a scope impacts and needs --impacted".to_owned(),
            "Run `cupel tests --impacted` with a scope, such as --changed.".to_owned(),
        ));
    }
    let config = invocation.config()?;
    let catalog = Catalog::builtin();
    let request = check::scope_request(args);
    let scoped = check::resolve(
        invocation.writer,
        config,
        &request,
        catalog.language_registry(),
    )?;
    let langs = check::languages_of(catalog.language_registry(), scoped.files.files.iter());
    let mut scope = ScopeInfo {
        mode: request.mode(),
        files: scoped.files.files.len() as u64,
        symbols: 0,
        roots: check::roots(&scoped.info, &langs),
        languages: langs.iter().map(|lang| lang.id.clone()).collect(),
    };
    let git = check::git_info(invocation.writer.root(), &scoped.files);
    if invocation.dry_run {
        let mut envelope = invocation.nothing_planned(events);
        envelope.run.scope = Some(scope);
        envelope.run.git = git;
        return Ok(envelope);
    }
    let run = invocation.run;
    let job = Job {
        run: run.id(),
        writer: invocation.writer,
        cancel: run.cancellation().clone(),
        config,
        selected: Vec::new(),
        langs: langs.clone(),
        files: scoped.tree.clone(),
        selectors: request.selectors()?,
        selectors_must_match: false,
        base: None,
    };
    let scheduler = Scheduler {
        checks: catalog.checks(),
        producers: catalog.producers(),
    };
    let mut impact = Impact {
        production: scheduler.production(&job),
        producers: catalog.producers(),
        job: &job,
        ctx: Ctx {
            exclude: config.file.scope.exclude.clone(),
            lang: config.file.lang.clone(),
            ..Ctx::new(invocation.writer.root().to_path_buf())
        },
        config,
        hunks: changed_lines(invocation.writer.root(), &request, &scoped.files)?,
        events,
    };
    let mut report = TestsReport {
        per_test_map: true,
        impacted: Vec::new(),
        commands: Vec::new(),
        languages: Vec::new(),
    };
    for lang in &langs {
        let files: Vec<String> = scoped
            .files
            .files
            .iter()
            .filter(|file| {
                catalog
                    .language_registry()
                    .for_path(file)
                    .is_some_and(|language| language.id() == lang.id)
            })
            .cloned()
            .collect();
        scope.symbols += impact.language(lang, &files, &mut report);
    }
    report.per_test_map = report.languages.iter().all(|basis| basis.per_test_map);
    let interrupted = impact.production.interrupted();
    let findings = impact.production.into_findings();
    let mut envelope = run.reported(Status::Pass, ExitCode::Pass, findings);
    envelope.run.scope = Some(scope);
    envelope.run.git = git;
    envelope.tests = Some(report);
    Ok(if interrupted {
        Run::interrupted(envelope)
    } else {
        envelope
    })
}

/// The lines a range scope changed per file; `None` without a range.
fn changed_lines(
    root: &std::path::Path,
    request: &ScopeRequest,
    resolved: &cupel_core::ResolvedFiles,
) -> Result<Option<Regions>, CupelError> {
    let (Some(base), Some(git)) = (&resolved.base, Git::open(root)) else {
        return Ok(None);
    };
    let hunks = git
        .changed_lines(base, request.head.as_deref())
        .map_err(|error| {
            CupelError::bad_argument(
                if request.changed {
                    "--changed"
                } else {
                    "--base"
                },
                None,
                format!("cannot read the changed lines: {error}"),
                "Run in a git work tree with git installed, or scope with --paths instead."
                    .to_owned(),
            )
        })?;
    Ok(Some(hunks))
}

/// One `tests --impacted` run in progress: the facts it produces and what it reads them with.
struct Impact<'a, 'e> {
    production: Production<'a>,
    producers: &'a ProducerRegistry,
    job: &'a Job<'a>,
    ctx: Ctx,
    config: &'a Config,
    hunks: Option<Regions>,
    events: &'e mut dyn EventSink,
}

impl Impact<'_, '_> {
    /// Adds `lang`'s impacted tests, commands, and basis for its `files` in scope to `report`;
    /// the number of symbols the scope's lines touch.
    fn language(&mut self, lang: &Lang, files: &[String], report: &mut TestsReport) -> u64 {
        let symbols = self.fact::<Symbols>(FactKind::Symbols, lang);
        let regions = self.regions(lang, files, symbols.as_ref());
        let chosen = match self.map(lang) {
            Ok(map) => self.by_coverage(lang, &map, &regions, symbols.as_ref()),
            Err(reason) => self.by_imports(lang, &regions, reason),
        };
        if !chosen.impacted.is_empty()
            && let Some(runner) = &chosen.runner
        {
            report
                .commands
                .extend(self.commands(lang, runner, &chosen.selection));
        }
        report.impacted.extend(chosen.impacted);
        report.languages.push(chosen.basis);
        impact::touched(&regions, symbols.as_ref())
    }

    /// `lang`'s per-test map, or why there is none.
    fn map(&mut self, lang: &Lang) -> Result<PerTestMap, String> {
        match SupportMatrix::fact(self.producers, IMPACT_FACT, lang, &self.ctx) {
            Support::Supported => self
                .production
                .ensure(IMPACT_FACT, lang, self.events)
                .map_err(|error| format!("the {IMPACT_FACT} could not be produced: {error}"))?
                .downcast_ref::<PerTestMap>()
                .cloned()
                .ok_or_else(|| format!("the {IMPACT_FACT} could not be read")),
            other => Err(unsupported(&other)),
        }
    }

    /// The tests `map` says execute the regions' lines.
    fn by_coverage(
        &mut self,
        lang: &Lang,
        map: &PerTestMap,
        regions: &Regions,
        symbols: Option<&Symbols>,
    ) -> Chosen {
        let frames = self.fact::<TestFrames>(FactKind::TestFrames, lang);
        let impacted = impact::by_coverage(map, regions, symbols, frames.as_ref(), &lang.id);
        let ids = impacted.iter().map(|test| test.id.clone()).collect();
        Chosen {
            basis: ImpactBasis {
                lang: lang.id.clone(),
                per_test_map: true,
                producer: self.ready(IMPACT_FACT, lang),
                fallback: None,
            },
            impacted,
            runner: Some(map.runner.clone()),
            selection: TestSelection::Tests(ids),
        }
    }

    /// The test files that import the regions' files, for a language without a per-test map
    /// (`reason` says why, ADR-006).
    fn by_imports(&mut self, lang: &Lang, regions: &Regions, reason: String) -> Chosen {
        let graph = self.fact::<ModuleGraph>(FactKind::ModuleGraph, lang);
        let conventions = self.fact::<TestConventions>(FactKind::TestConventions, lang);
        let impacted = match (graph, conventions) {
            (Some(graph), Some(conventions)) => {
                impact::by_imports(&graph, &conventions, regions, &lang.id)
            }
            _ => Vec::new(),
        };
        let files = impacted.iter().map(|test| test.id.clone()).collect();
        Chosen {
            basis: ImpactBasis {
                lang: lang.id.clone(),
                per_test_map: false,
                producer: None,
                fallback: Some(reason),
            },
            impacted,
            runner: self.ready(FactKind::TestResults, lang),
            selection: TestSelection::Files(files),
        }
    }

    /// The lines in scope of `lang`'s `files`: the selected symbols' spans with `--symbols`,
    /// the changed lines widened to their symbols with a range, else every line.
    fn regions(&mut self, lang: &Lang, files: &[String], symbols: Option<&Symbols>) -> Regions {
        if !self.job.selectors.is_empty() {
            let norm = self
                .job
                .selectors
                .iter()
                .any(cupel_core::SymbolSelector::needs_hashes)
                .then(|| self.fact::<NormTree>(FactKind::NormTree, lang))
                .flatten();
            let Some(symbols) = symbols else {
                return Regions::new();
            };
            let (keys, _) = select_symbols(&self.job.selectors, symbols, norm.as_ref());
            let mut regions = impact::spans(Some(symbols), &keys);
            regions.retain(|file, _| files.contains(file));
            return regions;
        }
        match &self.hunks {
            Some(hunks) => impact::widened(files, hunks, symbols),
            None => impact::whole(files),
        }
    }

    /// The fact `kind` of type `F` for `lang`, when it can be produced.
    fn fact<F: cupel_core::Fact + Clone>(&mut self, kind: FactKind, lang: &Lang) -> Option<F> {
        let fact = self.production.ensure(kind, lang, self.events).ok()?;
        fact.downcast_ref::<F>().cloned()
    }

    /// The first ready candidate producer of `kind` for `lang`, else the first.
    fn ready(&self, kind: FactKind, lang: &Lang) -> Option<ProducerId> {
        let candidates = self.producers.candidates(kind, lang, &self.ctx).ok()?;
        candidates
            .iter()
            .find(|producer| producer.availability(&self.ctx) == Availability::Ready)
            .or(candidates.first())
            .map(|producer| producer.id())
    }

    /// The commands that run `selection` with `runner` in `lang`'s project roots: narrowed when
    /// the runner can, else its whole suite.
    fn commands(
        &mut self,
        lang: &Lang,
        runner: &ProducerId,
        selection: &TestSelection,
    ) -> Vec<TestCommand> {
        let Some(found) = self.producers.runner(runner) else {
            return Vec::new();
        };
        let mut ctx = ProduceCtx::new(
            self.ctx.clone(),
            self.job.writer.confined(),
            self.job.cancel.clone(),
        );
        ctx.config = self.config.lang_config(&lang.id);
        if let Ok(info) = self
            .production
            .ensure(FactKind::ProjectInfo, lang, self.events)
        {
            ctx.upstream.insert(info);
        }
        let (commands, narrowed) = match found.commands(&ctx, selection) {
            Some(commands) => (commands, true),
            None => (
                found
                    .commands(&ctx, &TestSelection::All)
                    .unwrap_or_default(),
                false,
            ),
        };
        commands
            .into_iter()
            .map(|command| TestCommand {
                lang: lang.id.clone(),
                runner: runner.clone(),
                dir: command.dir,
                command: command.command,
                narrowed,
            })
            .collect()
    }
}

/// One language's impacted tests, how they were chosen, and the runner and selection that run
/// them.
struct Chosen {
    basis: ImpactBasis,
    impacted: Vec<ImpactedTest>,
    runner: Option<ProducerId>,
    selection: TestSelection,
}

/// Why a language whose per-test map cannot be produced falls back (ADR-006).
fn unsupported(support: &Support) -> String {
    match support {
        Support::Supported => String::new(),
        Support::Unsupported(fact) => {
            format!("no producer makes {fact} for this language (see `cupel describe matrix`)")
        }
        Support::Unavailable(tool) => {
            format!("the {IMPACT_FACT} producer needs {tool}, which is not installed")
        }
        Support::Unconfigured(key) => format!("the {IMPACT_FACT} producer needs `{key}` set"),
    }
}
