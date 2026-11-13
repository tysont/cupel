// ABOUTME: The `check` verb and the pipeline `survey` shares: walk the tree once, resolve the scope, select
// ABOUTME: checks, run the scheduler (or plan it), assemble the envelope, and grade it with the gate.
use std::collections::BTreeMap;
use std::path::Path;

use cupel_core::config::{Config, GateRuleSpec};
use cupel_core::git::{ChangeKind, Git};
use cupel_core::syntax::ProjectInfo;
use cupel_core::{
    ActionKind, BaseTree, ChangedLines, Check, Envelope, EventSink, ExitCode, FactKind, Finding,
    GateReference, GateResult, GitInfo, ImpactedTests, Job, Lang, LangId, Manifest, Measured,
    Outcome, PlannedInstall, PlannedWrite, ProducerRegistry, Range, ResolvedFiles, Scheduler,
    ScopeInfo, ScopeRequest, Selection, Severity, SeverityCounts, Status, SuiteReuse, Tolerances,
    Verb, Writer, apply_gate, evaluate_gate, gate_for, lacks_reference, next_actions,
};
use cupel_lang::LanguageRegistry;
use serde_json::json;

use crate::cli::ScopeArgs;
use crate::error::CupelError;
use crate::install::{self, Permission};
use crate::oracle::select::Selected;
use crate::problem::Problem;
use crate::resources;
use crate::run::{Executions, Invocation, Run, shell_quote};
use crate::scope;
use crate::tests_verb;

/// `cupel check`: the envelope of running the selected checks, or of planning them.
pub fn verb(
    invocation: &Invocation<'_>,
    args: &ScopeArgs,
    events: &mut dyn EventSink,
) -> Result<Envelope, CupelError> {
    let config = invocation.config()?;
    let (table, rules) = gate_for(&config.file.gate, scope::scope_request(args).mode());
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
/// evaluates the applicable gate unless `--no-gate` (ADR-060), with `reference` as `base`
/// (ADR-130) and the invocation's checks' metric tolerances and watch thresholds (ADR-195,
/// ADR-205), then both rank `next_actions` (ADR-061). Plans and errors pass through unchanged.
pub fn grade(
    envelope: &mut Envelope,
    verb: Verb,
    args: &ScopeArgs,
    invocation: &Invocation<'_>,
    reference: &GateReference,
) {
    if !matches!(envelope.status, Status::Pass | Status::Partial) {
        return;
    }
    if let Ok(config) = invocation.config
        && let Some((table, rules)) = gate_rules(envelope, verb, args, config)
    {
        let langs = envelope
            .run
            .scope
            .as_ref()
            .map(|scope| scope.languages.clone())
            .unwrap_or_default();
        let tolerances = Tolerances::declared(invocation.catalog().checks(), config, &langs);
        let gate = evaluate_gate(table, rules, envelope, reference, &tolerances);
        let missing = missing_baselines(&gate, &invocation.run.repro());
        envelope.findings.extend(missing);
        envelope.summary.findings = SeverityCounts::of(&envelope.findings);
        envelope.total_findings = envelope.findings.len() as u64;
        apply_gate(envelope, gate);
    }
    envelope.next_actions = next_actions(envelope, &rerun(verb, args));
}

/// One `baseline.missing` finding at info per rule of `gate` skipped because `base` had no
/// reference value (ADR-198); such a rule does not fail the gate.
fn missing_baselines(gate: &GateResult, repro: &str) -> Vec<Finding> {
    gate.rules
        .iter()
        .filter(|rule| lacks_reference(rule))
        .map(|rule| {
            let reason = rule.reason.clone().unwrap_or_default();
            Problem {
                check: "gate",
                kind: "baseline.missing",
                subject: rule.rule.clone(),
                severity: Severity::Info,
                title: format!("The gate rule `{}` was skipped: {reason}", rule.rule),
                detail: None,
                evidence: json!({"rule": rule.rule, "reason": reason}),
                action: ActionKind::Configure,
                fix: "Set a baseline with `cupel baseline set --write` from a passing run on \
                      the default branch, or pass --against REF|run:ID."
                    .to_owned(),
            }
            .finding(repro)
        })
        .collect()
}

/// The gate table `grade` evaluates on `envelope`, as `(name, rules)`: none for `survey`, under
/// `--no-gate`, or when the configuration lacks the scope's table.
pub fn gate_rules<'a>(
    envelope: &Envelope,
    verb: Verb,
    args: &ScopeArgs,
    config: &'a Config,
) -> Option<(&'static str, &'a [GateRuleSpec])> {
    if verb != Verb::Check || args.no_gate {
        return None;
    }
    let scope = envelope.run.scope.as_ref()?;
    match gate_for(&config.file.gate, scope.mode) {
        (table, Some(section)) => Some((table, section.rules())),
        (_, None) => None,
    }
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

/// Runs (or, with `dry_run`, plans) the checks `args` select over the scope it names.
pub fn run(
    invocation: &Invocation<'_>,
    config: &Config,
    args: &ScopeArgs,
    events: &mut dyn EventSink,
) -> Result<CheckRun, CupelError> {
    run_touching(invocation, config, args, None, events)
}

/// [`run`], with `touched` as the changed lines that narrow its symbols when given: a recomputed
/// base's old side of the range (ADR-175), in place of the lines its own scope changed.
pub(crate) fn run_touching(
    invocation: &Invocation<'_>,
    config: &Config,
    args: &ScopeArgs,
    touched: Option<ChangedLines>,
    events: &mut dyn EventSink,
) -> Result<CheckRun, CupelError> {
    let (run, writer) = (invocation.run, invocation.writer);
    resources::limit_threads(args.jobs);
    let catalog = invocation.catalog();
    let request = scope::scope_request(args);
    let scoped = scope::resolve(writer, config, &request, catalog.language_registry())?;
    let (info, resolved) = (scoped.info, scoped.files);
    let mut selected = selection(config, args).select(catalog.checks())?;
    let oracle = Selected::choose(writer, config, args.oracle.as_deref(), &mut selected)?;
    let languages = catalog.language_registry();
    let base_files = base_files(languages, &resolved);
    let langs = scope::languages_of(languages, resolved.files.iter().chain(&base_files));
    let git = scope::git_info(writer.root(), &resolved);
    let tree = touched.is_none().then_some(scoped.tree);
    let mut job = Job {
        run: run.id(),
        writer,
        cancel: run.cancellation().clone(),
        config,
        base: base_tree(invocation, languages, &resolved, &base_files, &selected)?,
        selected,
        langs: langs.clone(),
        files: resolved.files.clone(),
        selectors: request.selectors()?,
        selectors_must_match: !request.has_range(),
        seed: run.seed,
        depth: config.effective.depth,
        changed: touched_or_changed(touched, writer.root(), &request, &resolved)?,
        resources: resources::of(invocation, config, args, git.as_ref())?,
        oracle: oracle.run_oracle(config),
        cases: args.cases.map(u64::from),
        impacted: BTreeMap::new(),
    };
    job.impacted = impacted_tests(invocation, &job, tree, &request, events)?;
    let scheduler = Scheduler {
        checks: catalog.checks(),
        producers: catalog.producers(),
    };
    let (mut outcome, suite) = schedule(&scheduler, &job, invocation, events)?;
    oracle.apply(&mut outcome, catalog.checks());
    Ok(CheckRun {
        scope: scope_info(&request, &resolved, &outcome, &info, langs),
        git,
        suite,
        outcome,
    })
}

/// The scope a run of `langs` over `resolved` reports, with the symbols `outcome` counted.
fn scope_info(
    request: &ScopeRequest,
    resolved: &ResolvedFiles,
    outcome: &Outcome,
    info: &ProjectInfo,
    langs: Vec<Lang>,
) -> ScopeInfo {
    ScopeInfo {
        mode: request.mode(),
        files: resolved.files.len() as u64,
        symbols: outcome.symbols,
        roots: scope::roots(info, &langs),
        languages: langs.into_iter().map(|lang| lang.id).collect(),
        paths: resolved.considered(request.mode()),
        symbols_left_out: outcome.symbols_left_out.clone(),
    }
}

/// The checks `args` select at the configuration's depth.
fn selection(config: &Config, args: &ScopeArgs) -> Selection {
    Selection {
        depth: config.effective.depth,
        only: args.only.clone(),
        skip: args.skip.clone(),
        tier: args.tier.clone(),
    }
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

/// Per language, the tests `job`'s range impacts in the walk `tree`, when a selected check
/// reruns tests (ADR-186, ADR-190); none without a range, in a dry run, or without a walk (a
/// recomputed base's run).
fn impacted_tests(
    invocation: &Invocation<'_>,
    job: &Job<'_>,
    tree: Option<Vec<String>>,
    request: &ScopeRequest,
    events: &mut dyn EventSink,
) -> Result<BTreeMap<LangId, ImpactedTests>, CupelError> {
    let reruns = job
        .selected
        .iter()
        .any(|check| check.manifest().needs.contains(&FactKind::TestReruns));
    let (Some(hunks), Some(tree)) = (job.changed.clone(), tree) else {
        return Ok(BTreeMap::new());
    };
    if !reruns || invocation.dry_run {
        return Ok(BTreeMap::new());
    }
    let range = tests_verb::RangeScope {
        tree,
        files: &job.files,
        hunks,
        selectors: request.selectors()?,
    };
    Ok(tests_verb::impacted_tests(
        invocation, job.config, &job.langs, range, events,
    ))
}

/// `touched` when given, else the lines the range scope changed.
fn touched_or_changed(
    touched: Option<ChangedLines>,
    root: &Path,
    request: &ScopeRequest,
    resolved: &ResolvedFiles,
) -> Result<Option<ChangedLines>, CupelError> {
    match touched {
        Some(lines) => Ok(Some(lines)),
        None => scope::changed_lines(root, request, resolved),
    }
}

/// Runs (or, for a dry run, plans) `job`, keeping the test runs it made for the store, and
/// per language how a joint suite run reused its cache (ADR-149 §5); a red baseline is the
/// exit-4 refusal (ADR-076).
fn schedule(
    scheduler: &Scheduler<'_>,
    job: &Job<'_>,
    invocation: &Invocation<'_>,
    events: &mut dyn EventSink,
) -> Result<(Outcome, BTreeMap<LangId, SuiteReuse>), CupelError> {
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
    let suite = suites_of(&outcome);
    invocation.run.keep(Executions {
        test_results: std::mem::take(&mut outcome.test_results),
        mutation_outcomes: std::mem::take(&mut outcome.mutation_outcomes),
        durations: std::mem::take(&mut outcome.durations),
        measurements: outcome.measurements.clone(),
        matches: std::mem::take(&mut outcome.matches),
    });
    match outcome.refused.take() {
        Some(red) => Err(CupelError::Unevaluable(Box::new(red))),
        None => Ok((outcome, suite)),
    }
}

/// Per language, how the joint suite run that made its baseline reused its cache (ADR-149 §5),
/// from the test runs `outcome` kept.
fn suites_of(outcome: &Outcome) -> BTreeMap<LangId, SuiteReuse> {
    outcome
        .test_results
        .iter()
        .filter_map(|(lang, results)| Some((lang.clone(), results.reuse.clone()?)))
        .collect()
}

/// What `check` (or `survey`) found, before it becomes an envelope.
pub struct CheckRun {
    scope: ScopeInfo,
    git: Option<GitInfo>,
    /// Per language, what the joint suite run that made its baseline reran and reused.
    suite: BTreeMap<LangId, SuiteReuse>,
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

    /// What `survey` ranks: the manifests of the checks that have a result, and every
    /// per-target value they measured (ADR-062).
    pub fn take_measurements(&mut self) -> (Vec<Manifest>, Vec<Measured>) {
        let manifests = crate::catalog::checks()
            .iter()
            .map(cupel_core::Check::manifest)
            .filter(|manifest| self.outcome.checks.iter().any(|c| c.id == manifest.id))
            .cloned()
            .collect();
        (manifests, std::mem::take(&mut self.outcome.measurements))
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
        envelope.run.suite.clone_from(&self.suite);
        envelope
    }
}

/// The changed files that existed at the range's base and that a language owns (ADR-056).
pub(crate) fn base_files(languages: &LanguageRegistry, resolved: &ResolvedFiles) -> Vec<String> {
    resolved
        .changes
        .iter()
        .filter(|change| change.kind != ChangeKind::Added)
        .filter(|change| languages.for_path(&change.path).is_some())
        .map(|change| change.path.clone())
        .collect()
}

/// The base side of a range scope: checked out into the run's scratch directory when a
/// selected check reads it, with the changed manifests `languages` read, else only described
/// (ADR-056, ADR-192). `None` without a range.
fn base_tree(
    invocation: &Invocation<'_>,
    languages: &LanguageRegistry,
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
        Some(git) => BaseTree::checkout(
            range,
            &git,
            &scratch,
            |path| files.iter().any(|file| file == path),
            |path| languages.is_manifest(path),
        )?,
        None => BaseTree::planned(range, &scratch),
    }))
}
