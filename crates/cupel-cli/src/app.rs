// ABOUTME: Runs one invocation end to end: parse, resolve the root, open events, load configuration,
// ABOUTME: dispatch the verb, turn every outcome into an envelope, and keep the run record.
use std::ffi::OsString;
use std::fmt::Write as _;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use clap::FromArgMatches;
use clap::error::ErrorKind;
use cupel_core::config::{Config, ConfigRequest, Setting, Settings};
use cupel_core::{
    Envelope, Event, EventSink, ExitCode, Finding, RunPlan, Status, Verb, Writer, random_seed,
};
use cupel_plugin::{Discovery, Plugins};

use crate::cli::{Cli, VerbArgs, verb_name};
pub use crate::environment::{Environment, Output};
use crate::error::{BugReport, CupelError, Usage};
use crate::events::Events;
use crate::format::{self, Rendering};
use crate::install::Permission;
use crate::page::Page;
use crate::problem::Problem;
use crate::run::{Invocation, Run};
use crate::{
    against, baseline, catalog, check, describe, doctor, explain, init, oracle, persist, reference,
    registry, render, scaffold, serve, suppress, survey, tests_verb,
};

/// How an envelope reaches stdout: its renderer, whether text is colored, the findings page, and
/// the repository root SARIF locates files against, once one is resolved.
struct Presentation {
    rendering: Rendering,
    color: bool,
    page: Page,
    root: Option<PathBuf>,
}

impl Presentation {
    fn new(
        environment: &Environment,
        rendering: Rendering,
        no_color_flag: bool,
        page: Page,
    ) -> Self {
        Self {
            rendering,
            color: environment.color(rendering, no_color_flag),
            page,
            root: None,
        }
    }

    /// This presentation for a run in the repository at `root`.
    fn rooted(self, root: &Path) -> Self {
        Self {
            root: Some(root.to_path_buf()),
            ..self
        }
    }
}

pub fn run(environment: &Environment) -> Output {
    let printable = environment
        .args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let mut run = Run::start(printable);
    run.within(&environment.cancel);
    let argv = std::iter::once(OsString::from("cupel")).chain(environment.args.iter().cloned());
    let parsed = registry::command()
        .try_get_matches_from(argv)
        .and_then(|matches| Cli::from_arg_matches(&matches));
    match parsed {
        Ok(cli) => accepted(environment, &mut run, &cli),
        Err(error) => rejected(environment, &mut run, &error),
    }
}

fn accepted(environment: &Environment, run: &mut Run, cli: &Cli) -> Output {
    run.verb = cli.verb.as_ref().map(VerbArgs::verb);
    run.seed = cli.global.seed.unwrap_or_else(random_seed);
    if let Some(timeout) = cli.global.timeout {
        run.set_timeout(timeout.as_duration());
    }
    let page = Page {
        offset: cli.global.offset.unwrap_or(0),
        limit: cli.global.limit,
    };
    let presentation =
        |rendering| Presentation::new(environment, rendering, cli.global.no_color, page);
    let writer = match writer_for(environment, cli) {
        Ok(writer) => writer,
        Err(error) => {
            return output(
                run.failed(&error),
                &presentation(recovered_rendering(environment)),
                String::new(),
            );
        }
    };
    let mut stderr = String::new();
    let destination = cli.global.events.as_deref();
    let (envelope, rendering, events) = match Events::open(
        destination,
        &writer,
        (&environment.stderr, environment.events.as_ref()),
    ) {
        Ok(mut events) => {
            let (envelope, rendering, said) =
                configured(environment, run, cli, &writer, &mut events);
            stderr.push_str(&said);
            (envelope, rendering, Some(events))
        }
        Err(error) => (run.failed(&error), recovered_rendering(environment), None),
    };
    if let Some(mut events) = events {
        events.emit(Event::RunEnd {
            status: envelope.status,
            exit_code: envelope.exit_code,
        });
        if let Some(problem) = events.error() {
            let _ = writeln!(stderr, "cupel: could not write events: {problem}");
        }
    }
    if keeps_run_record(run, cli) {
        record(&writer, &envelope, &run.take_executions(), &mut stderr);
    }
    routed(
        cli,
        output(
            envelope,
            &presentation(rendering).rooted(writer.root()),
            stderr,
        ),
    )
}

/// `output` as the process writes it: under `serve --mcp` stdout carries the protocol, so the
/// server's own envelope follows its diagnostics on stderr (ADR-121).
fn routed(cli: &Cli, mut output: Output) -> Output {
    if matches!(&cli.verb, Some(VerbArgs::Serve(args)) if args.mcp) {
        let envelope = std::mem::take(&mut output.stdout);
        output.stderr.push_str(&envelope);
    }
    output
}

/// The writer for the repository the invocation names: recording, for a run that only plans.
fn writer_for(environment: &Environment, cli: &Cli) -> Result<Writer, CupelError> {
    let root = repository_root(
        environment.current_dir.as_deref(),
        cli.global.cwd.as_deref(),
    )?;
    let writer = Writer::new(&root, cli.global.write)
        .map_err(|_| not_a_directory(&root.display().to_string()))?;
    Ok(if cli.plans_only() {
        writer.recording()
    } else {
        writer
    })
}

/// Keeps `envelope` as the run record and in the fact store; a failure is a line on `stderr`
/// and changes nothing else (ADR-045 §8).
fn record(
    writer: &Writer,
    envelope: &Envelope,
    executed: &crate::run::Executions,
    stderr: &mut String,
) {
    if let Err(error) = writer.run_record(&envelope.run.id, envelope) {
        let _ = writeln!(stderr, "cupel: could not write the run record: {error}");
    }
    let suppressions = suppress::load(writer).ok();
    if let Err(error) = persist::record(writer, envelope, executed, suppressions.as_ref()) {
        let _ = writeln!(
            stderr,
            "cupel: could not record the run in the fact store: {error}"
        );
    }
}

/// Every parsed run of a verb other than `describe` keeps its envelope under `.cupel/runs/` and
/// in the fact store, except a dry run or a preview, which write nothing (ADR-020 §3,
/// ADR-021 §5, ADR-045 §8).
fn keeps_run_record(run: &Run, cli: &Cli) -> bool {
    !cli.plans_only() && !cli.global.describe && run.verb.is_some_and(|verb| verb != Verb::Describe)
}

/// Loads configuration, chooses the format, and runs the verb, with what to say on stderr.
/// Errors before the format is known render as ADR-018 §5 recovers it.
fn configured(
    environment: &Environment,
    run: &mut Run,
    cli: &Cli,
    writer: &Writer,
    events: &mut Events,
) -> (Envelope, Rendering, String) {
    let mut said = String::new();
    let stderr = &mut said;
    let request = config_request(environment, cli, writer.root().to_owned());
    let (config, plugins) = match load_config(&request, plugin_path(environment, cli), run) {
        Ok(loaded) => loaded,
        Err(error) if runs_without_configuration(cli) => {
            let envelope = guarded(run, stderr, cli.global.quiet, || {
                invoke(
                    (run, writer, &Plugins::default()),
                    Err(&error),
                    cli,
                    environment,
                    events,
                )
            });
            return (envelope, recovered_rendering(environment), said);
        }
        Err(error) => {
            return (
                run.failed(&error.into()),
                recovered_rendering(environment),
                said,
            );
        }
    };
    run.configure(&config);
    let format = config
        .effective
        .format
        .unwrap_or_else(|| format::default_for(environment.stdout_is_terminal));
    let rendering = format::rendering(format);
    if cli.global.verbose {
        let config_path = config.path.as_deref().map(Path::display);
        let _ = writeln!(stderr, "cupel: root: {}", writer.root().display());
        if let Some(path) = config_path {
            let _ = writeln!(stderr, "cupel: config: {path}");
        }
        let _ = writeln!(stderr, "cupel: format: {}", format.as_str());
    }
    let envelope = guarded(run, stderr, cli.global.quiet, || {
        invoke(
            (run, writer, &plugins),
            Ok(&config),
            cli,
            environment,
            events,
        )
    });
    (envelope, rendering, said)
}

/// Runs the verb of `run` with `config`, the loaded configuration or why it did not load. The
/// run may install tools as the flags say, else as whether stdout is a terminal says (ADR-081).
fn invoke(
    (run, writer, plugins): (&Run, &Writer, &Plugins),
    config: Result<&Config, &cupel_core::Error>,
    cli: &Cli,
    environment: &Environment,
    events: &mut dyn EventSink,
) -> Result<Envelope, CupelError> {
    let invocation = Invocation {
        run,
        writer,
        session: &environment.session,
        config,
        plugins,
        dry_run: cli.global.dry_run,
        installs: Permission::of(
            cli.global.install,
            cli.global.no_install,
            environment.stdout_is_terminal,
        ),
    };
    execute(&invocation, cli, environment, events)
}

/// The configuration and the plugins it and `path` name (ADR-088 §4), the configuration
/// rejected when it names a check, gate metric, or language the registries, plugins included,
/// do not have (ADR-017, ADR-038).
fn load_config(
    request: &ConfigRequest,
    path: Option<&str>,
    run: &Run,
) -> Result<(Config, Plugins), cupel_core::Error> {
    let config = Config::load(request)?;
    let languages = cupel_lang::LanguageRegistry::builtin().ids();
    let mut checks = catalog::checks();
    let plugins = cupel_plugin::discover(&Discovery {
        root: &request.root,
        entries: &config.file.plugins,
        path,
        languages: &languages,
        taken: checks
            .iter()
            .map(|check| check.manifest().id.clone())
            .collect(),
        cancel: run.cancellation(),
    })?;
    plugins.register_checks(&mut checks);
    config.validate_registered(&checks, &languages)?;
    Ok((config, plugins))
}

/// The `PATH` plugins are searched on: the invocation's, unless `--no-path-plugins`.
fn plugin_path<'a>(environment: &'a Environment, cli: &Cli) -> Option<&'a str> {
    environment
        .path
        .as_deref()
        .filter(|_| !cli.global.no_path_plugins)
}

/// `init`, `doctor`, and `serve` run even when configuration does not load: `init` replaces a
/// broken `cupel.toml`, `doctor` reports it (ADR-021 §6, ADR-022 §2), and the daemon reads none
/// of its own; each request it serves loads and reports configuration as a cold run would.
fn runs_without_configuration(cli: &Cli) -> bool {
    matches!(
        cli.verb,
        Some(VerbArgs::Init { .. } | VerbArgs::Doctor { .. } | VerbArgs::Serve(_))
    )
}

/// Whether the verb refuses to run when this binary does not satisfy `required_version`: every
/// verb that reads or grades the repository. `describe` describes this binary, `doctor` reports
/// the mismatch, `init` and `suppress` edit the files that settle it, and the daemon checks each
/// request it serves as a cold run would.
fn checks_required_version(cli: &Cli) -> bool {
    !matches!(
        cli.verb,
        None | Some(
            VerbArgs::Describe(_)
                | VerbArgs::Doctor { .. }
                | VerbArgs::Init { .. }
                | VerbArgs::Suppress { .. }
                | VerbArgs::Serve(_)
        )
    )
}

/// What configuration needs from the command line and the environment.
fn config_request(environment: &Environment, cli: &Cli, root: PathBuf) -> ConfigRequest {
    let scope = cli.verb.as_ref().and_then(VerbArgs::scope);
    ConfigRequest {
        root,
        config_path: cli.global.config.clone(),
        profile: cli.global.profile.clone(),
        flags: Settings {
            depth: scope.and_then(|scope| scope.depth),
            format: cli.global.format,
            budget: scope.and_then(|scope| scope.budget),
        },
        env: environment.variables.clone(),
    }
}

/// The rendering ADR-018 §5 recovers from the raw arguments, for errors raised before
/// configuration chooses the format.
fn recovered_rendering(environment: &Environment) -> Rendering {
    let recovered = format::recover(
        &environment.args,
        environment.variable(Setting::Format.env_var()),
        environment.stdout_is_terminal,
    );
    format::rendering(recovered.format)
}

/// Runs the verb with `config`, the loaded configuration or why it did not load. A dry run
/// plans instead; a run whose slow work the timeout cut short reports its results as partial
/// (ADR-020 §5, §6).
fn execute(
    invocation: &Invocation<'_>,
    cli: &Cli,
    environment: &Environment,
    events: &mut dyn EventSink,
) -> Result<Envelope, CupelError> {
    assert!(
        !environment.debug_panic(),
        "CUPEL_DEBUG_PANIC is set; failing deliberately"
    );
    let run = invocation.run;
    if cli.global.describe {
        let words = cli.verb.as_ref().map(VerbArgs::words).unwrap_or_default();
        return describe::command(run, &words);
    }
    if checks_required_version(cli)
        && let Some(refused) = doctor::refusal(invocation)?
    {
        return Ok(refused);
    }
    let cut_short = !cli.plans_only()
        && environment
            .debug_sleep()
            .is_some_and(|duration| !run.cancellation().wait(duration));
    let envelope = match &cli.verb {
        None => Err(CupelError::missing_verb(verb_names())),
        Some(verb) => run_verb(invocation, (cli, environment), verb, events),
    }?;
    let envelope = graded(envelope, cli, invocation)?;
    Ok(if cut_short {
        Run::interrupted(envelope)
    } else {
        envelope
    })
}

/// Runs `verb`. The dry run of a verb that changes nothing is an empty plan.
fn run_verb(
    invocation: &Invocation<'_>,
    (cli, environment): (&Cli, &Environment),
    verb: &VerbArgs,
    events: &mut dyn EventSink,
) -> Result<Envelope, CupelError> {
    let (run, writer, dry_run) = (invocation.run, invocation.writer, invocation.dry_run);
    match verb {
        VerbArgs::Describe(args) => {
            let described = describe::run(run, args, &invocation.catalog(), writer.root())?;
            // `describe` runs nothing and writes nothing; its plan is empty.
            Ok(if dry_run {
                invocation.nothing_planned(events)
            } else {
                described
            })
        }
        // `doctor` writes only its probe and run record; its plan lists only what `--fix` would
        // install (ADR-022 §6, ADR-081).
        VerbArgs::Doctor { fix } if dry_run => Ok(doctor::plan(invocation, *fix, events)),
        // `explain` only reads the store; a dry run of `serve` starts and stops nothing.
        VerbArgs::Explain { .. } | VerbArgs::Serve(_) if dry_run => {
            Ok(invocation.nothing_planned(events))
        }
        VerbArgs::Doctor { fix } => Ok(doctor::run(invocation, *fix)),
        VerbArgs::Serve(args) => Ok(serve::verb(
            invocation,
            args,
            (cli.global.quiet, self::run),
            environment,
        )),
        VerbArgs::Init { force } => init_verb(invocation, cli, *force, events),
        VerbArgs::Check { scope, .. } => check::verb(invocation, scope, events),
        VerbArgs::Survey { scope } => survey::verb(invocation, scope, events),
        VerbArgs::Tests { impacted, scope } => {
            tests_verb::verb(invocation, *impacted, scope, events)
        }
        // `explain` only reads the store.
        VerbArgs::Explain { target, .. } => explain::run(run, writer, target),
        VerbArgs::Baseline { name, action } => {
            baseline::run(run, writer, events, name.as_deref(), action)
        }
        VerbArgs::Suppress {
            finding,
            reason,
            until,
        } => {
            let reason = reason.as_deref().unwrap_or_default();
            let envelope = suppress::run(run, writer, finding, reason, until.as_deref())?;
            Ok(if dry_run {
                invocation.nothing_planned(events)
            } else {
                envelope
            })
        }
        VerbArgs::Oracle { action } => oracle::run(invocation, action, events),
    }
}

/// `init`: writes the configuration, or previews what `--write` would write.
fn init_verb(
    invocation: &Invocation<'_>,
    cli: &Cli,
    force: bool,
    events: &mut dyn EventSink,
) -> Result<Envelope, CupelError> {
    let run = invocation.run;
    if !cli.plans_only() {
        let findings = findings(run, init::write(invocation.writer, force)?);
        return Ok(run.reported(Status::Pass, ExitCode::Pass, findings));
    }
    // A preview shows what `--write` would do, so its recording writer may plan writes
    // outside `.cupel/`; it never writes anything.
    let preview = Writer::new(invocation.writer.root(), true)?.recording();
    let findings = findings(run, init::write(&preview, force)?);
    let mut envelope = run.plan_envelope(RunPlan::default(), &preview, events, findings);
    envelope.next_actions.push(init::apply(force));
    Ok(envelope)
}

/// A finished run of a verb that grades the repository with the repository's suppressions
/// applied (ADR-047 §5) and, for `check` and `survey`, results against an unverified oracle
/// capped first (ADR-091 §4), then deltas against `--against` or the default baseline
/// (ADR-046 §5), the gate with its reference run (ADR-060, ADR-130), and `next_actions`
/// (ADR-061). Plans and the bookkeeping
/// verbs pass through unchanged.
fn graded(
    mut envelope: Envelope,
    cli: &Cli,
    invocation: &Invocation<'_>,
) -> Result<Envelope, CupelError> {
    let (writer, config) = (invocation.writer, invocation.config);
    if envelope.status == Status::Plan {
        return Ok(envelope);
    }
    match &cli.verb {
        Some(VerbArgs::Init { .. } | VerbArgs::Doctor { .. } | VerbArgs::Tests { .. }) => {
            suppress::apply(&mut envelope, writer)?;
        }
        Some(VerbArgs::Check { scope, .. } | VerbArgs::Survey { scope }) => {
            if let Ok(config) = config {
                oracle::cap::cap(&mut envelope, writer, config, &invocation.run.repro())?;
            }
            suppress::apply(&mut envelope, writer)?;
            let compared = against::compare(&mut envelope, writer, scope.against.as_deref())?;
            if let (Some(verb), Ok(config)) = (envelope.run.verb, config) {
                let reference = reference::resolve(invocation, config, scope, &envelope, compared);
                check::grade(&mut envelope, verb, scope, config, &reference.as_gate());
            }
            if let Some(VerbArgs::Check { scaffold: true, .. }) = &cli.verb {
                scaffold::apply(
                    &mut envelope,
                    writer,
                    invocation.catalog().language_registry(),
                    &invocation.run.repro(),
                    cli.global.write,
                )?;
            }
        }
        _ => {}
    }
    Ok(envelope)
}

/// `problems` as findings of `run`.
fn findings(run: &Run, problems: Vec<Problem>) -> Vec<Finding> {
    problems
        .into_iter()
        .map(|problem| problem.finding(&run.repro()))
        .collect()
}

/// A command line clap rejected: help and version print as usual; everything else becomes a
/// usage envelope, unless `--describe` was asked of a recognizable command.
fn rejected(environment: &Environment, run: &mut Run, error: &clap::Error) -> Output {
    if matches!(
        error.kind(),
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
    ) {
        return Output {
            stdout: error.render().to_string(),
            stderr: String::new(),
            code: 0,
        };
    }
    let recovered = format::recover(
        &environment.args,
        environment.variable(Setting::Format.env_var()),
        environment.stdout_is_terminal,
    );
    let rendering = format::rendering(recovered.format);
    // Paging flags are not trusted from a command line that did not parse.
    let presentation =
        Presentation::new(environment, rendering, recovered.no_color, Page::default());
    let words = command_words(&environment.args);
    run.verb = words.first().and_then(|word| verb_named(word));
    if recovered.describe
        && !words.is_empty()
        && let Ok(envelope) = describe::command(run, &words)
    {
        return output(envelope, &presentation, String::new());
    }
    let usage = if error.kind() == ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand {
        CupelError::missing_verb(verb_names())
    } else {
        Usage::from_clap(error, &verb_names()).into()
    };
    let stderr = if recovered.quiet {
        String::new()
    } else {
        error.render().to_string()
    };
    output(run.failed(&usage), &presentation, stderr)
}

fn output(mut envelope: Envelope, presentation: &Presentation, stderr: String) -> Output {
    presentation.page.apply(&mut envelope);
    Output {
        stdout: render::render(
            &envelope,
            presentation.rendering,
            presentation.color,
            presentation.root.as_deref(),
        ),
        stderr,
        code: envelope.exit_code.code(),
    }
}

/// `--cwd` resolved against the process working directory; it must be an existing directory.
pub(crate) fn repository_root(
    current_dir: Option<&Path>,
    cwd: Option<&Path>,
) -> Result<PathBuf, CupelError> {
    let requested = match (cwd, current_dir) {
        (Some(cwd), Some(current)) => current.join(cwd),
        (Some(cwd), None) => cwd.to_path_buf(),
        (None, Some(current)) => current.to_path_buf(),
        (None, None) => PathBuf::new(),
    };
    requested
        .canonicalize()
        .ok()
        .filter(|root| root.is_dir())
        .ok_or_else(|| not_a_directory(&cwd.unwrap_or(&requested).display().to_string()))
}

/// The usage error for a `--cwd` (shown as given) that is not a usable directory.
fn not_a_directory(shown: &str) -> CupelError {
    CupelError::from(Usage {
        subject: "--cwd".to_owned(),
        title: format!("`{shown}` is not a directory"),
        fix: "Pass --cwd an existing directory: the repository root.".to_owned(),
        detail: None,
        argument: Some("--cwd".to_owned()),
        value: Some(shown.to_owned()),
        valid: Vec::new(),
    })
}

/// The verb and sub-verb words clap can still recognize in a command line it rejected.
pub(crate) fn command_words(args: &[OsString]) -> Vec<String> {
    let command_line = std::iter::once(OsString::from("cupel")).chain(args.iter().cloned());
    let Ok(matches) = registry::command()
        .ignore_errors(true)
        .try_get_matches_from(command_line)
    else {
        return Vec::new();
    };
    let mut words = Vec::new();
    let mut current = &matches;
    while let Some((name, sub)) = current.subcommand() {
        words.push(name.to_owned());
        current = sub;
    }
    words
}

fn verb_named(word: &str) -> Option<Verb> {
    Verb::ALL.into_iter().find(|verb| verb_name(*verb) == word)
}

fn verb_names() -> Vec<String> {
    Verb::ALL.into_iter().map(verb_name).collect()
}

/// The message and source location of the most recent panic, recorded by the hook that
/// `capture_panics` installs.
static LAST_PANIC: Mutex<Option<(String, String)>> = Mutex::new(None);

/// Replaces the default panic output with a record the internal-error envelope reports.
pub fn capture_panics() {
    panic::set_hook(Box::new(|info| {
        let location = info
            .location()
            .map(|at| format!("{}:{}:{}", at.file(), at.line(), at.column()))
            .unwrap_or_default();
        let message = payload_message(info.payload());
        if let Ok(mut last) = LAST_PANIC.lock() {
            *last = Some((message, location));
        }
    }));
}

/// Runs `work`, turning an error into its envelope and a panic into an exit-6 bug report.
fn guarded(
    run: &Run,
    stderr: &mut String,
    quiet: bool,
    work: impl FnOnce() -> Result<Envelope, CupelError>,
) -> Envelope {
    match panic::catch_unwind(AssertUnwindSafe(work)) {
        Ok(Ok(envelope)) => envelope,
        Ok(Err(error)) => run.failed(&error),
        Err(payload) => {
            let recorded = LAST_PANIC.lock().ok().and_then(|mut last| last.take());
            let (message, location) = recorded
                .unwrap_or_else(|| (payload_message(payload.as_ref()), "unknown".to_owned()));
            if !quiet {
                let _ = writeln!(stderr, "cupel: internal error at {location}: {message}");
            }
            run.failed(&CupelError::Internal(BugReport {
                message,
                location,
                args: run.args.clone(),
            }))
        }
    }
}

fn payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic with a non-string payload".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An invocation of `args` in `dir`, so verbs that keep a run record write there.
    fn environment(dir: &Path, args: &[&str]) -> Environment {
        Environment::new(
            args.iter().map(OsString::from).collect(),
            Some(dir.to_path_buf()),
        )
    }

    #[test]
    fn relative_cwd_resolves_against_the_working_directory_not_the_process() {
        let base = std::env::temp_dir().canonicalize().unwrap();
        let parent = base.parent().unwrap().to_path_buf();
        let root = repository_root(Some(&base), Some(Path::new(".."))).unwrap();
        assert_eq!(root, parent);
    }

    #[test]
    fn missing_cwd_is_a_usage_error_about_cwd() {
        let error =
            repository_root(Some(Path::new("/")), Some(Path::new("/no/such/dir"))).unwrap_err();
        assert_eq!(error.exit_code(), cupel_core::ExitCode::Usage);
        let CupelError::Usage(usage) = error else {
            panic!("usage error expected")
        };
        assert_eq!(usage.subject, "--cwd");
    }

    #[test]
    fn panics_inside_a_verb_become_internal_error_envelopes() {
        let run = Run::start(vec!["describe".to_owned()]);
        let mut stderr = String::new();
        let envelope = guarded(&run, &mut stderr, true, || panic!("boom"));
        assert_eq!(envelope.exit_code, cupel_core::ExitCode::Internal);
        assert_eq!(envelope.findings[0].kind, "cli.internal");
        assert_eq!(stderr, "");
    }

    #[test]
    fn command_words_finds_verbs_in_a_rejected_command_line() {
        let words =
            |args: &[&str]| command_words(&args.iter().map(OsString::from).collect::<Vec<_>>());
        assert_eq!(words(&["suppress", "--describe"]), ["suppress"]);
        assert_eq!(words(&["baseline", "set", "--bogus"]), ["baseline", "set"]);
        assert!(words(&["frobnicate"]).is_empty());
    }

    #[test]
    fn text_on_a_terminal_is_colored_unless_color_is_turned_off() {
        let dir = tempfile::tempdir().unwrap();
        let environment = |args: &[&str]| environment(dir.path(), args);
        let on_terminal = |args: &[&str], no_color_variable: bool| {
            let mut environment = environment(args);
            environment.stdout_is_terminal = true;
            environment.no_color_variable = no_color_variable;
            run(&environment).stdout.contains('\u{1b}')
        };
        // (arguments, NO_COLOR set) on a terminal; a rejected invocation is colored too.
        let colored = [
            on_terminal(&["check"], false),
            on_terminal(&["check", "--no-color"], false),
            on_terminal(&["check"], true),
            on_terminal(&["check", "--format", "json"], false),
            on_terminal(&["check", "--bogus"], false),
            on_terminal(&["check", "--bogus", "--no-color"], false),
            run(&environment(&["check", "--format", "text"]))
                .stdout
                .contains('\u{1b}'),
        ];
        assert_eq!(colored, [true, false, false, false, true, false, false]);
    }

    #[test]
    fn a_describe_run_needs_nothing_from_the_process() {
        let dir = tempfile::tempdir().unwrap();
        let out = run(&environment(dir.path(), &["describe", "check"]));
        assert_eq!(out.code, 0);
        assert!(out.stdout.contains("\"subject\": \"command\""));
    }

    /// `args` with a slow task injected through `CUPEL_DEBUG_SLEEP_MS`.
    fn slow(dir: &Path, args: &[&str], sleep_ms: &str) -> Output {
        let mut environment = environment(dir, args);
        environment
            .variables
            .insert("CUPEL_DEBUG_SLEEP_MS".to_owned(), sleep_ms.to_owned());
        run(&environment)
    }

    #[test]
    fn a_timeout_that_cuts_the_slow_task_short_makes_the_verb_partial() {
        let dir = tempfile::tempdir().unwrap();
        let out = slow(dir.path(), &["describe", "--timeout", "1ms"], "60000");
        assert_eq!(out.code, 5, "{}", out.stdout);
        assert!(out.stdout.contains("\"status\": \"partial\""));
        assert!(out.stdout.contains("\"subject\": \"cli\""));
    }

    #[test]
    fn a_slow_task_that_finishes_in_time_leaves_the_run_complete() {
        let dir = tempfile::tempdir().unwrap();
        let out = slow(dir.path(), &["describe", "--timeout", "1h"], "1");
        assert_eq!(out.code, 0, "{}", out.stdout);
    }

    #[test]
    fn an_error_cut_short_by_the_timeout_keeps_its_exit_code() {
        let dir = tempfile::tempdir().unwrap();
        let out = slow(
            dir.path(),
            &["describe", "nonsense", "--timeout", "1ms"],
            "60000",
        );
        assert_eq!(out.code, 2, "{}", out.stdout);
    }

    #[test]
    fn a_dry_run_runs_no_slow_task() {
        let dir = tempfile::tempdir().unwrap();
        let out = slow(
            dir.path(),
            &["describe", "--dry-run", "--timeout", "1ms"],
            "60000",
        );
        assert_eq!(out.code, 0, "{}", out.stdout);
        assert!(out.stdout.contains("\"status\": \"plan\""));
    }

    #[test]
    fn an_unwritable_run_record_is_reported_on_stderr_and_changes_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        // A file where the `.cupel` directory belongs makes every write under it fail.
        std::fs::write(dir.path().join(".cupel"), "").unwrap();
        let out = run(&environment(dir.path(), &["tests", "--quiet"]));
        assert_eq!(out.code, 2);
        assert!(
            out.stderr
                .starts_with("cupel: could not write the run record: "),
            "{}",
            out.stderr
        );
    }
}
