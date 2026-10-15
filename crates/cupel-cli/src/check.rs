// ABOUTME: The `check` verb and the pipeline `survey` shares: walk the tree once, resolve the scope, select
// ABOUTME: checks, run the scheduler (or plan it), assemble the envelope, and grade it with the gate.
use std::collections::BTreeSet;
use std::path::Path;

use cupel_core::config::Config;
use cupel_core::git::{ChangeKind, Git};
use cupel_core::syntax::ProjectInfo;
use cupel_core::{
    BaseTree, ChangedLines, Check, Envelope, EventSink, ExitCode, Finding, GitInfo, Job, Lang,
    Outcome, PlannedInstall, PlannedWrite, ProducerRegistry, RANKED, Range, ResolvedFiles,
    Scheduler, ScopeInfo, ScopeRequest, Selection, Status, Verb, Writer, apply_gate, evaluate_gate,
    gate_for, next_actions, rankings,
};
use cupel_lang::LanguageRegistry;

use crate::catalog::Catalog;
use crate::cli::ScopeArgs;
use crate::error::CupelError;
use crate::install::{self, Permission};
use crate::project;
use crate::run::{Executions, Invocation, Run, shell_quote};

/// `cupel check`: the envelope of running the selected checks, or of planning them.
pub fn verb(
    invocation: &Invocation<'_>,
    args: &ScopeArgs,
    events: &mut dyn EventSink,
) -> Result<Envelope, CupelError> {
    let config = invocation.config()?;
    let (table, rules) = gate_for(&config.file.gate, scope_request(args).mode());
    if args.gate && rules.is_none() {
        return Err(CupelError::bad_argument(
            "--gate",
            None,
            format!("`--gate` needs a `[{table}]` table, and the configuration has none"),
            format!("Add `[{table}]` gate rules to cupel.toml, or drop --gate."),
        ));
    }
    let checked = self::run(invocation, config, args, events)?;
    Ok(checked.into_envelope(invocation.run, invocation.writer, events))
}

/// Grades a finished `check` or `survey` envelope after suppressions and deltas: `check`
/// evaluates the applicable gate unless `--no-gate` (ADR-060), then both rank `next_actions`
/// (ADR-061). Plans and errors pass through unchanged.
pub fn grade(envelope: &mut Envelope, verb: Verb, args: &ScopeArgs, config: &Config) {
    if !matches!(envelope.status, Status::Pass | Status::Partial) {
        return;
    }
    if verb == Verb::Check
        && !args.no_gate
        && let Some(scope) = &envelope.run.scope
        && let (table, Some(section)) = gate_for(&config.file.gate, scope.mode)
    {
        let gate = evaluate_gate(table, section.rules(), envelope);
        apply_gate(envelope, gate);
    }
    envelope.next_actions = next_actions(envelope, &rerun(verb, args));
}

/// The command that reruns `verb` over the scope `args` name (`--only` is appended per check).
fn rerun(verb: Verb, args: &ScopeArgs) -> String {
    let mut words = vec!["cupel".to_owned(), crate::cli::verb_name(verb)];
    if args.changed {
        words.push("--changed".to_owned());
    }
    for (flag, value) in [("--base", &args.base), ("--head", &args.head)] {
        if let Some(value) = value {
            words.extend([flag.to_owned(), shell_quote(value)]);
        }
    }
    for (flag, values) in [("--paths", &args.paths), ("--symbols", &args.symbols)] {
        if !values.is_empty() {
            words.push(flag.to_owned());
            words.extend(values.iter().map(|value| shell_quote(value)));
        }
    }
    words.join(" ")
}

pub(crate) fn scope_request(args: &ScopeArgs) -> ScopeRequest {
    ScopeRequest {
        changed: args.changed,
        base: args.base.clone(),
        head: args.head.clone(),
        paths: args.paths.clone(),
        symbols: args.symbols.clone(),
    }
}

/// Runs (or, with `dry_run`, plans) the checks `args` select over the scope it names.
pub fn run(
    invocation: &Invocation<'_>,
    config: &Config,
    args: &ScopeArgs,
    events: &mut dyn EventSink,
) -> Result<CheckRun, CupelError> {
    let (run, writer) = (invocation.run, invocation.writer);
    let catalog = Catalog::builtin();
    let request = scope_request(args);
    let Scoped {
        info,
        files: resolved,
        ..
    } = resolve(writer, config, &request, catalog.language_registry())?;
    let selected = Selection {
        depth: config.effective.depth,
        only: args.only.clone(),
        skip: args.skip.clone(),
        tier: args.tier.clone(),
    }
    .select(catalog.checks())?;
    let base_files = base_files(catalog.language_registry(), &resolved);
    let langs = languages_of(
        catalog.language_registry(),
        resolved.files.iter().chain(&base_files),
    );
    let job = Job {
        run: run.id(),
        writer,
        cancel: run.cancellation().clone(),
        config,
        base: base_tree(invocation, &resolved, &base_files, &selected)?,
        selected,
        langs: langs.clone(),
        files: resolved.files.clone(),
        selectors: request.selectors()?,
        selectors_must_match: !request.has_range(),
        seed: run.seed,
        depth: config.effective.depth,
        changed: changed_lines(writer.root(), &request, &resolved)?,
    };
    let scheduler = Scheduler {
        checks: catalog.checks(),
        producers: catalog.producers(),
    };
    let outcome = schedule(&scheduler, &job, invocation, events)?;
    let scope = ScopeInfo {
        mode: request.mode(),
        files: resolved.files.len() as u64,
        symbols: outcome.symbols,
        roots: roots(&info, &langs),
        languages: langs.into_iter().map(|lang| lang.id).collect(),
    };
    Ok(CheckRun {
        scope,
        git: git_info(writer.root(), &resolved),
        outcome,
    })
}

/// Before a run, the missing tools of the producers it would choose for the job's checks and
/// languages, installed as far as the invocation may (ADR-081): the installs a dry run would
/// attempt, and the findings of the installs a real run made or was refused at the terminal's
/// question.
fn install_tools(
    invocation: &Invocation<'_>,
    producers: &ProducerRegistry,
    job: &Job<'_>,
) -> (Vec<PlannedInstall>, Vec<Finding>) {
    let (run, root) = (invocation.run, invocation.writer.root());
    let planned = crate::doctor::installs(&job.selected, producers, &job.langs, job.config, root);
    if invocation.dry_run {
        return (invocation.installs.would_install(&planned), Vec::new());
    }
    if matches!(invocation.installs, Permission::Never(_)) {
        return (Vec::new(), Vec::new());
    }
    let findings = install::problems(&install::attempt(invocation, &planned))
        .into_iter()
        .map(|problem| problem.finding(&run.repro()))
        .collect();
    (Vec::new(), findings)
}

/// The lines a range scope changed per file; `None` without a range.
pub(crate) fn changed_lines(
    root: &Path,
    request: &ScopeRequest,
    resolved: &ResolvedFiles,
) -> Result<Option<ChangedLines>, CupelError> {
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

/// Runs (or, for a dry run, plans) `job`, keeping the test runs it made for the store; a red
/// baseline is the exit-4 refusal (ADR-076).
fn schedule(
    scheduler: &Scheduler<'_>,
    job: &Job<'_>,
    invocation: &Invocation<'_>,
    events: &mut dyn EventSink,
) -> Result<Outcome, CupelError> {
    let (would_install, installed) = install_tools(invocation, scheduler.producers, job);
    let mut outcome = if invocation.dry_run {
        scheduler.plan(job, events)?
    } else {
        scheduler.run(job, events)?
    };
    outcome.findings.splice(0..0, installed);
    if let Some(plan) = &mut outcome.plan {
        plan.would_install = would_install;
    }
    invocation.run.keep(Executions {
        test_results: std::mem::take(&mut outcome.test_results),
        mutation_outcomes: std::mem::take(&mut outcome.mutation_outcomes),
    });
    match outcome.refused.take() {
        Some(red) => Err(CupelError::Unevaluable(Box::new(red))),
        None => Ok(outcome),
    }
}

/// A resolved scope: the project roots outside `[scope] exclude`, every file of the walk, and
/// the files and range changes the request selects from it (ADR-065).
pub(crate) struct Scoped {
    pub info: ProjectInfo,
    pub tree: Vec<String>,
    pub files: ResolvedFiles,
}

/// Walks the repository and resolves `request` against the walk.
pub(crate) fn resolve(
    writer: &Writer,
    config: &Config,
    request: &ScopeRequest,
    languages: &LanguageRegistry,
) -> Result<Scoped, CupelError> {
    let root = writer.root();
    let exclude = &config.file.scope.exclude;
    let info = project::detect(writer, exclude);
    let tree = cupel_lang::walk_excluding(root, languages, &info, exclude);
    let mut files = request.resolve_files(&tree, root, config.file.default_branch.as_deref())?;
    files
        .changes
        .retain(|change| !exclude.excludes(&change.path));
    Ok(Scoped { info, tree, files })
}

/// What `check` (or `survey`) found, before it becomes an envelope.
pub struct CheckRun {
    scope: ScopeInfo,
    git: Option<GitInfo>,
    outcome: Outcome,
}

impl CheckRun {
    /// The plan of a dry run, if this was one, with the run record a real run would keep.
    fn take_plan(&mut self) -> Option<cupel_core::RunPlan> {
        self.outcome.plan.take().map(|mut plan| {
            plan.would_write.push(PlannedWrite {
                path: ".cupel/runs/<id>.json".to_owned(),
                diff: None,
            });
            plan
        })
    }

    /// The plan envelope of a dry run, or the envelope of a real run.
    pub fn into_envelope(
        mut self,
        run: &Run,
        writer: &Writer,
        events: &mut dyn EventSink,
    ) -> Envelope {
        match self.take_plan() {
            Some(plan) => {
                let findings = self.outcome.findings.clone();
                self.place(run.plan_envelope(plan, writer, events, findings))
            }
            None => self.envelope(run),
        }
    }

    /// The rankings of every per-target metric of the checks that have a result (ADR-062).
    pub fn rankings(&self) -> cupel_core::Rankings {
        let registry = crate::catalog::checks();
        let manifests = registry
            .iter()
            .map(cupel_core::Check::manifest)
            .filter(|manifest| self.outcome.checks.iter().any(|c| c.id == manifest.id));
        rankings(manifests, &self.outcome.measurements, RANKED)
    }

    /// The envelope of a real run: `pass` (exit 0), or `partial` (exit 5) when the timeout cut
    /// work short; the gate and `next_actions` come after grading (`grade`).
    fn envelope(mut self, run: &Run) -> Envelope {
        let findings = std::mem::take(&mut self.outcome.findings);
        let mut envelope = self.place(run.reported(Status::Pass, ExitCode::Pass, findings));
        envelope.checks = std::mem::take(&mut self.outcome.checks);
        if self.outcome.interrupted {
            Run::interrupted(envelope)
        } else {
            envelope
        }
    }

    /// `envelope` with this run's scope and git information.
    pub fn place(&self, mut envelope: Envelope) -> Envelope {
        envelope.run.scope = Some(self.scope.clone());
        envelope.run.git.clone_from(&self.git);
        envelope
    }
}

/// The changed files that existed at the range's base and that a language owns (ADR-056).
fn base_files(languages: &LanguageRegistry, resolved: &ResolvedFiles) -> Vec<String> {
    resolved
        .changes
        .iter()
        .filter(|change| change.kind != ChangeKind::Added)
        .filter(|change| languages.for_path(&change.path).is_some())
        .map(|change| change.path.clone())
        .collect()
}

/// The base side of a range scope: checked out into the run's scratch directory when a
/// selected check reads it, else only described (ADR-056). `None` without a range.
fn base_tree(
    invocation: &Invocation<'_>,
    resolved: &ResolvedFiles,
    files: &[String],
    selected: &[&dyn Check],
) -> Result<Option<BaseTree>, CupelError> {
    let Some(commit) = &resolved.base else {
        return Ok(None);
    };
    let range = Range {
        commit: commit.clone(),
        changes: resolved.changes.clone(),
    };
    let scratch = invocation.writer.scratch(&invocation.run.id());
    let reads_base = selected.iter().any(|check| !check.base_needs().is_empty());
    let git = Git::open(invocation.writer.root()).filter(|_| reads_base && !invocation.dry_run);
    Ok(Some(match git {
        Some(git) => BaseTree::checkout(range, &git, &scratch, |path| {
            files.iter().any(|file| file == path)
        })?,
        None => BaseTree::planned(range, &scratch),
    }))
}

/// The languages that own at least one of `files`, in ID order.
pub(crate) fn languages_of<'a>(
    languages: &LanguageRegistry,
    files: impl Iterator<Item = &'a String>,
) -> Vec<Lang> {
    let owning: BTreeSet<_> = files
        .filter_map(|file| languages.for_path(file))
        .map(|language| language.id())
        .collect();
    languages
        .langs()
        .into_iter()
        .filter(|lang| owning.contains(&lang.id))
        .collect()
}

/// The detected project roots of the languages in scope; `.` when none is detected.
pub(crate) fn roots(info: &ProjectInfo, langs: &[Lang]) -> Vec<String> {
    let roots: BTreeSet<String> = info
        .roots
        .iter()
        .filter(|root| langs.iter().any(|lang| lang.id == root.lang))
        .map(|root| root.path.clone())
        .collect();
    if roots.is_empty() {
        vec![".".to_owned()]
    } else {
        roots.into_iter().collect()
    }
}

/// `run.git`: null outside a work tree or before the first commit.
pub(crate) fn git_info(root: &Path, resolved: &ResolvedFiles) -> Option<GitInfo> {
    let head = Git::open(root)?.head();
    Some(GitInfo {
        sha: head.sha?,
        dirty: head.dirty,
        base: resolved.base.clone(),
        branch: head.branch,
    })
}
