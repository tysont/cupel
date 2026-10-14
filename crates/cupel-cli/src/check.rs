// ABOUTME: The `check` verb: walk the tree once, resolve the scope, select checks, run the scheduler
// ABOUTME: (or plan it for --dry-run), and assemble the envelope's checks, findings, scope, and git info.
use std::collections::BTreeSet;
use std::path::Path;

use cupel_core::config::Config;
use cupel_core::git::{ChangeKind, Git};
use cupel_core::syntax::ProjectInfo;
use cupel_core::{
    BaseTree, Check, Envelope, EventSink, ExitCode, GitInfo, Job, Lang, Outcome, PlannedWrite,
    Range, ResolvedFiles, Scheduler, ScopeInfo, ScopeRequest, Selection, Status, Writer,
    next_actions,
};
use cupel_lang::LanguageRegistry;

use crate::app;
use crate::catalog::Catalog;
use crate::cli::ScopeArgs;
use crate::error::CupelError;
use crate::project;
use crate::run::Run;

/// `cupel check`: the envelope of running the selected checks, or of planning them.
pub fn verb(
    run: &Run,
    args: &ScopeArgs,
    config: Result<&Config, &cupel_core::Error>,
    writer: &Writer,
    events: &mut dyn EventSink,
    dry_run: bool,
) -> Result<Envelope, CupelError> {
    let config = config.map_err(|error| CupelError::from(error.clone()))?;
    let mut checked = self::run(run, args, config, writer, events, dry_run)?;
    Ok(match checked.take_plan() {
        Some(plan) => {
            let findings = checked.findings();
            checked.place(app::plan(run, plan, writer, events, findings))
        }
        None => checked.envelope(run),
    })
}

/// Runs (or, with `dry_run`, plans) the checks `args` select over the scope it names.
pub fn run(
    run: &Run,
    args: &ScopeArgs,
    config: &Config,
    writer: &Writer,
    events: &mut dyn EventSink,
    dry_run: bool,
) -> Result<CheckRun, CupelError> {
    let catalog = Catalog::builtin();
    let root = writer.root();
    let info = project::detect(writer);
    let tree = cupel_lang::walk(root, catalog.language_registry(), &info);
    let request = ScopeRequest {
        changed: args.changed,
        base: args.base.clone(),
        head: args.head.clone(),
        paths: args.paths.clone(),
        symbols: args.symbols.clone(),
    };
    let resolved = request.resolve_files(&tree, root, config.file.default_branch.as_deref())?;
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
    let base = base_tree(
        run,
        root,
        writer,
        &resolved,
        &base_files,
        &selected,
        dry_run,
    )?;
    let job = Job {
        run: run.id(),
        writer,
        cancel: run.cancellation().clone(),
        config,
        selected,
        langs: langs.clone(),
        files: resolved.files.clone(),
        selectors: request.selectors()?,
        selectors_must_match: !request.has_range(),
        base,
    };
    let scheduler = Scheduler {
        checks: catalog.checks(),
        producers: catalog.producers(),
    };
    let outcome = if dry_run {
        scheduler.plan(&job, events)?
    } else {
        scheduler.run(&job, events)?
    };
    let scope = ScopeInfo {
        mode: request.mode(),
        files: resolved.files.len() as u64,
        symbols: outcome.symbols,
        roots: roots(&info, &langs),
        languages: langs.into_iter().map(|lang| lang.id).collect(),
    };
    Ok(CheckRun {
        scope,
        git: git_info(root, &resolved),
        outcome,
    })
}

/// What `check` found, before it becomes an envelope.
pub struct CheckRun {
    scope: ScopeInfo,
    git: Option<GitInfo>,
    outcome: Outcome,
}

impl CheckRun {
    /// The plan of a dry run, if this was one, with the run record a real run would keep.
    pub fn take_plan(&mut self) -> Option<cupel_core::RunPlan> {
        self.outcome.plan.take().map(|mut plan| {
            plan.would_write.push(PlannedWrite {
                path: ".cupel/runs/<id>.json".to_owned(),
                diff: None,
            });
            plan
        })
    }

    /// The findings, which a dry run reports too (such as files that do not parse).
    pub fn findings(&self) -> Vec<cupel_core::Finding> {
        self.outcome.findings.clone()
    }

    /// The envelope of a real run: `pass` (exit 0) until gates exist (P2.9), `partial` (exit 5)
    /// when the timeout cut work short.
    pub fn envelope(mut self, run: &Run) -> Envelope {
        let findings = std::mem::take(&mut self.outcome.findings);
        let mut envelope = self.place(run.reported(Status::Pass, ExitCode::Pass, findings));
        envelope.next_actions = next_actions(&envelope.findings);
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
    run: &Run,
    root: &Path,
    writer: &Writer,
    resolved: &ResolvedFiles,
    files: &[String],
    selected: &[&dyn Check],
    dry_run: bool,
) -> Result<Option<BaseTree>, CupelError> {
    let Some(commit) = &resolved.base else {
        return Ok(None);
    };
    let range = Range {
        commit: commit.clone(),
        changes: resolved.changes.clone(),
    };
    let scratch = writer.scratch(&run.id());
    let reads_base = selected.iter().any(|check| !check.base_needs().is_empty());
    let git = Git::open(root).filter(|_| reads_base && !dry_run);
    Ok(Some(match git {
        Some(git) => BaseTree::checkout(range, &git, &scratch, |path| {
            files.iter().any(|file| file == path)
        })?,
        None => BaseTree::planned(range, &scratch),
    }))
}

/// The languages that own at least one of `files`, in ID order.
fn languages_of<'a>(
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
fn roots(info: &ProjectInfo, langs: &[Lang]) -> Vec<String> {
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
fn git_info(root: &Path, resolved: &ResolvedFiles) -> Option<GitInfo> {
    let head = Git::open(root)?.head();
    Some(GitInfo {
        sha: head.sha?,
        dirty: head.dirty,
        base: resolved.base.clone(),
        branch: head.branch,
    })
}
