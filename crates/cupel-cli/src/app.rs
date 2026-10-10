// ABOUTME: Runs one invocation end to end: parse, choose the format, resolve the root, dispatch the
// ABOUTME: verb, and turn every outcome — success, usage error, unbuilt feature, panic — into an envelope.
use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::IsTerminal;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use clap::FromArgMatches;
use clap::error::ErrorKind;
use cupel_core::{Envelope, Verb};

use crate::cli::{Cli, Format, VerbArgs, verb_name};
use crate::error::{BugReport, CupelError, Usage};
use crate::format::{self, Rendering};
use crate::run::Run;
use crate::{describe, registry, render};

/// Everything an invocation reads from its process. `main` gathers it once; nothing else looks
/// at the process environment or working directory.
pub struct Environment {
    /// The arguments after the program name.
    pub args: Vec<OsString>,
    /// The process working directory, the root when `--cwd` is absent.
    pub current_dir: Option<PathBuf>,
    pub stdout_is_terminal: bool,
    /// `CUPEL_FORMAT`, for choosing a format when the command line did not parse.
    pub format_variable: Option<String>,
    /// `CUPEL_DEBUG_PANIC=1`: fail deliberately after parsing, to exercise exit 6 (ADR-017).
    pub debug_panic: bool,
}

impl Environment {
    pub fn from_process() -> Self {
        Self {
            args: std::env::args_os().skip(1).collect(),
            current_dir: std::env::current_dir().ok(),
            stdout_is_terminal: std::io::stdout().is_terminal(),
            format_variable: std::env::var("CUPEL_FORMAT").ok(),
            debug_panic: std::env::var_os("CUPEL_DEBUG_PANIC").is_some_and(|value| value == "1"),
        }
    }
}

/// What to write and how to exit.
pub struct Output {
    pub stdout: String,
    pub stderr: String,
    pub code: u8,
}

pub fn run(environment: &Environment) -> Output {
    let printable = environment
        .args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let mut run = Run::start(printable);
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
    // P0.7 replaces this default with a random seed; an explicit seed is echoed as given.
    run.seed = cli.global.seed.unwrap_or(0);
    let format = cli
        .global
        .format
        .unwrap_or_else(|| Format::default_for(environment.stdout_is_terminal));
    let rendering = match format.rendering() {
        Ok(rendering) => rendering,
        Err(error) => return output(&run.failed(&error), Rendering::Json, String::new()),
    };
    let root = match repository_root(
        environment.current_dir.as_deref(),
        cli.global.cwd.as_deref(),
    ) {
        Ok(root) => root,
        Err(error) => return output(&run.failed(&error), rendering, String::new()),
    };
    let mut stderr = String::new();
    if cli.global.verbose {
        let _ = writeln!(stderr, "cupel: root: {}", root.display());
        let _ = writeln!(stderr, "cupel: format: {}", format.name());
    }
    let envelope = guarded(run, &mut stderr, cli.global.quiet, || {
        execute(run, cli, environment.debug_panic)
    });
    output(&envelope, rendering, stderr)
}

fn execute(run: &Run, cli: &Cli, debug_panic: bool) -> Result<Envelope, CupelError> {
    assert!(
        !debug_panic,
        "CUPEL_DEBUG_PANIC is set; failing deliberately"
    );
    if cli.global.describe {
        let words = cli.verb.as_ref().map(VerbArgs::words).unwrap_or_default();
        return describe::command(run, &words);
    }
    match &cli.verb {
        None => Err(CupelError::missing_verb(verb_names())),
        Some(VerbArgs::Describe(args)) => describe::run(run, args),
        Some(verb) => {
            let words = verb.words();
            let step = registry::find(&words[..1])
                .and_then(|command| command.pending_step)
                .expect("every verb without behavior names its plan step in the registry");
            Err(CupelError::pending_verb(&words, &step))
        }
    }
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
        environment.format_variable.as_deref(),
        environment.stdout_is_terminal,
    );
    let rendering = recovered.format.rendering().unwrap_or(Rendering::Json);
    let words = command_words(&environment.args);
    run.verb = words.first().and_then(|word| verb_named(word));
    if recovered.describe
        && !words.is_empty()
        && let Ok(envelope) = describe::command(run, &words)
    {
        return output(&envelope, rendering, String::new());
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
    output(&run.failed(&usage), rendering, stderr)
}

fn output(envelope: &Envelope, rendering: Rendering, stderr: String) -> Output {
    Output {
        stdout: render::render(envelope, rendering),
        stderr,
        code: envelope.exit_code.code(),
    }
}

/// `--cwd` resolved against the process working directory; it must be an existing directory.
fn repository_root(current_dir: Option<&Path>, cwd: Option<&Path>) -> Result<PathBuf, CupelError> {
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
        .ok_or_else(|| {
            let shown = cwd.unwrap_or(&requested).display().to_string();
            CupelError::from(Usage {
                subject: "--cwd".to_owned(),
                title: format!("`{shown}` is not a directory"),
                fix: "Pass --cwd an existing directory: the repository root.".to_owned(),
                detail: None,
                argument: Some("--cwd".to_owned()),
                value: Some(shown),
                valid: Vec::new(),
            })
        })
}

/// The verb and sub-verb words clap can still recognize in a command line it rejected.
fn command_words(args: &[OsString]) -> Vec<String> {
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

    fn environment(args: &[&str]) -> Environment {
        Environment {
            args: args.iter().map(OsString::from).collect(),
            current_dir: std::env::temp_dir().canonicalize().ok(),
            stdout_is_terminal: false,
            format_variable: None,
            debug_panic: false,
        }
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
    fn a_describe_run_needs_nothing_from_the_process() {
        let out = run(&environment(&["describe", "check"]));
        assert_eq!(out.code, 0);
        assert!(out.stdout.contains("\"subject\": \"command\""));
    }
}
