// ABOUTME: The `tests --impacted` verb (design §6.5, ADR-077): the tests a scope impacts, chosen by per-test coverage
// ABOUTME: maps where a language has one and by import-graph impact where it does not (ADR-006), with commands.
mod impact;

use std::collections::BTreeMap;

use cupel_core::config::Config;
use cupel_core::syntax::{ModuleGraph, NormTree, Symbols, TestConventions, TestFrames};
use cupel_core::{
    Attribution, Availability, CostClass, Ctx, Envelope, EventSink, ExitCode, FactKind,
    ImpactBasis, ImpactedTest, ImpactedTests, Job, Lang, LangId, PerTestMap, ProduceCtx,
    ProducerId, ProducerRegistry, Production, Scheduler, ScopeInfo, ScopeRequest, Status, Support,
    SupportMatrix, TestCommand, TestSelection, TestsReport, attributions, select_symbols,
};

use self::impact::Regions;
use crate::catalog::Catalog;
use crate::cli::ScopeArgs;
use crate::error::CupelError;
use crate::run::{Invocation, Run};
use crate::scope;

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
        return Err(needs_impacted());
    }
    let config = invocation.config()?;
    let catalog = invocation.catalog();
    let request = scope::scope_request(args);
    let scoped = scope::resolve(
        invocation.writer,
        config,
        &request,
        catalog.language_registry(),
    )?;
    let langs = scope::languages_of(catalog.language_registry(), scoped.files.files.iter());
    let mut scope = scope_info(&request, &scoped, &langs);
    let git = scope::git_info(invocation.writer.root(), &scoped.files);
    let mut envelope = if invocation.dry_run {
        invocation.nothing_planned(events)
    } else {
        let job = job(
            invocation,
            config,
            &langs,
            scoped.tree,
            request.selectors()?,
        );
        let scheduler = Scheduler {
            checks: catalog.checks(),
            producers: catalog.producers(),
        };
        let mut impact = Impact {
            production: scheduler.production(&job),
            producers: catalog.producers(),
            job: &job,
            ctx: ctx(invocation, config),
            config,
            hunks: scope::changed_lines(invocation.writer.root(), &request, &scoped.files)?,
            events,
            within_depth: false,
        };
        let by_language = files_by_language(&catalog, &scoped.files.files);
        let (report, symbols) = impact.report(&langs, &by_language);
        scope.symbols = symbols;
        impact.envelope(invocation.run, report)
    };
    envelope.run.scope = Some(scope);
    envelope.run.git = git;
    Ok(envelope)
}

/// The tests a range impacts, per language, for a check run whose checks rerun them: the
/// range's changed lines (`hunks`) in its `files`, the whole walk (`tree`) the import graph
/// reads, and the `--symbols` selectors.
pub(crate) struct RangeScope<'r> {
    pub tree: Vec<String>,
    pub files: &'r [String],
    pub hunks: Regions,
    pub selectors: Vec<cupel_core::SymbolSelector>,
}

/// Per language of `langs`, the tests `range` impacts, chosen as `tests --impacted` chooses them
/// over the whole walk, except that a per-test map is made only within the run's depth (ADR-186,
/// ADR-190).
pub(crate) fn impacted_tests(
    invocation: &Invocation<'_>,
    config: &Config,
    langs: &[Lang],
    range: RangeScope<'_>,
    events: &mut dyn EventSink,
) -> BTreeMap<LangId, ImpactedTests> {
    let catalog = invocation.catalog();
    let job = job(invocation, config, langs, range.tree, range.selectors);
    let scheduler = Scheduler {
        checks: catalog.checks(),
        producers: catalog.producers(),
    };
    let mut impact = Impact {
        production: scheduler.production(&job),
        producers: catalog.producers(),
        job: &job,
        ctx: ctx(invocation, config),
        config,
        hunks: Some(range.hunks),
        events,
        within_depth: true,
    };
    let by_language = files_by_language(&catalog, range.files);
    langs
        .iter()
        .map(|lang| {
            let files = by_language.get(&lang.id).map_or(&[][..], Vec::as_slice);
            let symbols = impact.fact::<Symbols>(FactKind::Symbols, lang);
            let regions = impact.regions(lang, files, symbols.as_ref());
            let chosen = impact.choose(lang, &regions, symbols.as_ref());
            let tests = ImpactedTests {
                runner: chosen.runner,
                selection: chosen.selection,
            };
            (lang.id.clone(), tests)
        })
        .collect()
}

/// What producers see in `tests --impacted`'s production, as in a check run.
fn ctx(invocation: &Invocation<'_>, config: &Config) -> Ctx {
    Ctx {
        exclude: config.file.scope.exclude.clone(),
        lang: config.file.lang.clone(),
        roots: config.file.roots.values().cloned().collect(),
        ..Ctx::new(invocation.writer.root().to_path_buf())
    }
}

/// The scope as the envelope reports it, before its symbols are counted.
fn scope_info(request: &ScopeRequest, scoped: &scope::Scoped, langs: &[Lang]) -> ScopeInfo {
    ScopeInfo {
        mode: request.mode(),
        files: scoped.files.files.len() as u64,
        symbols: 0,
        roots: scope::roots(&scoped.info, langs),
        languages: langs.iter().map(|lang| lang.id.clone()).collect(),
    }
}

/// The usage error of `tests` without `--impacted`, the one listing it makes.
fn needs_impacted() -> CupelError {
    CupelError::bad_argument(
        "--impacted",
        None,
        "`cupel tests` lists the tests a scope impacts and needs --impacted".to_owned(),
        "Run `cupel tests --impacted` with a scope, such as --changed.".to_owned(),
    )
}

/// A scheduler job over the whole walk (`tree`), so the import graph sees every importer, for
/// the languages in scope.
fn job<'a>(
    invocation: &Invocation<'a>,
    config: &'a Config,
    langs: &[Lang],
    tree: Vec<String>,
    selectors: Vec<cupel_core::SymbolSelector>,
) -> Job<'a> {
    Job {
        run: invocation.run.id(),
        writer: invocation.writer,
        cancel: invocation.run.cancellation().clone(),
        config,
        selected: Vec::new(),
        langs: langs.to_vec(),
        files: tree,
        selectors,
        selectors_must_match: false,
        base: None,
        seed: invocation.run.seed,
        depth: config.effective.depth,
        changed: None,
        resources: cupel_core::budget::Resources::default(),
        oracle: None,
        cases: None,
        impacted: BTreeMap::new(),
    }
}

/// The files of `files` each language owns.
fn files_by_language(catalog: &Catalog, files: &[String]) -> BTreeMap<LangId, Vec<String>> {
    let mut owned: BTreeMap<LangId, Vec<String>> = BTreeMap::new();
    for file in files {
        if let Some(language) = catalog.language_registry().for_path(file) {
            owned.entry(language.id()).or_default().push(file.clone());
        }
    }
    owned
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
    /// Whether a per-test map is made only within the run's depth, as a check run's wants are,
    /// rather than whatever it costs (ADR-190).
    within_depth: bool,
}

impl Impact<'_, '_> {
    /// The report of every language in `langs`, given the files in scope `by_language`, and the
    /// number of symbols the scope's lines touch.
    fn report(
        &mut self,
        langs: &[Lang],
        by_language: &BTreeMap<LangId, Vec<String>>,
    ) -> (TestsReport, u64) {
        let mut report = TestsReport {
            per_test_map: true,
            impacted: Vec::new(),
            commands: Vec::new(),
            languages: Vec::new(),
        };
        let mut symbols = 0;
        for lang in langs {
            let files = by_language.get(&lang.id).map_or(&[][..], Vec::as_slice);
            symbols += self.language(lang, files, &mut report);
        }
        report.per_test_map = report.languages.iter().all(|basis| basis.per_test_map);
        (report, symbols)
    }

    /// The finished run's envelope: the producers' findings and `report`, partial when the
    /// timeout cut production short.
    fn envelope(self, run: &Run, report: TestsReport) -> Envelope {
        let interrupted = self.production.interrupted();
        let mut envelope = run.reported(
            Status::Pass,
            ExitCode::Pass,
            self.production.into_findings(),
        );
        envelope.tests = Some(report);
        if interrupted {
            Run::interrupted(envelope)
        } else {
            envelope
        }
    }

    /// Adds `lang`'s impacted tests, commands, and basis for its `files` in scope to `report`;
    /// the number of symbols the scope's lines touch.
    fn language(&mut self, lang: &Lang, files: &[String], report: &mut TestsReport) -> u64 {
        let symbols = self.fact::<Symbols>(FactKind::Symbols, lang);
        let regions = self.regions(lang, files, symbols.as_ref());
        let chosen = self.choose(lang, &regions, symbols.as_ref());
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

    /// The tests of `lang` the regions impact: by its per-test map, else by its import graph.
    fn choose(&mut self, lang: &Lang, regions: &Regions, symbols: Option<&Symbols>) -> Chosen {
        match self.map(lang) {
            Ok(map) => self.by_coverage(lang, &map, regions, symbols),
            Err(reason) => self.by_imports(lang, regions, reason),
        }
    }

    /// `lang`'s per-test map, or why there is none.
    fn map(&mut self, lang: &Lang) -> Result<PerTestMap, String> {
        match SupportMatrix::fact(self.producers, IMPACT_FACT, lang, &self.ctx) {
            Support::Supported
                if self.within_depth && !self.production.within_depth(IMPACT_FACT, lang) =>
            {
                Err(format!(
                    "the {IMPACT_FACT} costs more than the run's depth, {}",
                    self.job.depth
                ))
            }
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
        let attributions = self.attributions(&lang.id);
        let mut impacted = impact::by_coverage(map, regions, symbols, frames.as_ref(), &lang.id);
        impact::kept(&mut impacted, map, regions, frames.as_ref(), &attributions);
        let ids = impacted
            .iter()
            .filter(|test| test.kept.is_none())
            .map(|test| test.id.clone())
            .collect();
        Chosen {
            basis: ImpactBasis {
                lang: lang.id.clone(),
                per_test_map: true,
                producer: self.ready(IMPACT_FACT, lang),
                measured: Some(map.measured),
                reused: Some(map.reused),
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
        let (impacted, selection) = match (graph, conventions) {
            (Some(graph), Some(conventions)) => {
                let impacted = impact::by_imports(&graph, &conventions, regions, &lang.id);
                let files = impacted.iter().map(|test| test.id.clone()).collect();
                (impacted, TestSelection::Files(files))
            }
            // Nothing chose the tests, so a run reruns them all.
            _ => (Vec::new(), TestSelection::All),
        };
        Chosen {
            basis: ImpactBasis {
                lang: lang.id.clone(),
                per_test_map: false,
                producer: None,
                measured: None,
                reused: None,
                fallback: Some(reason),
            },
            impacted,
            runner: self.ready(FactKind::TestResults, lang),
            selection,
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

    /// `lang`'s attributions (ADR-151), which keep tests below `--depth deep` only.
    fn attributions(&self, lang: &LangId) -> Vec<Attribution> {
        if self.job.depth >= CostClass::Deep {
            return Vec::new();
        }
        self.ctx
            .lang
            .get(lang)
            .and_then(|table| attributions(lang, table).ok())
            .unwrap_or_default()
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
