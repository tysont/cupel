// ABOUTME: The reference run a gate rule against `base` compares with (ADR-130): `--against`, the base of a
// ABOUTME: range recomputed over the base versions of its changed files, or the default baseline.
use std::path::Path;

use cupel_core::config::Config;
use cupel_core::config::GateRuleSpec;
use cupel_core::git::Git;
use cupel_core::{Envelope, Event, GateReference, Status, Writer, needs_reference};

use crate::check::{self, gate_rules};
use crate::cli::ScopeArgs;
use crate::install::Permission;
use crate::run::Invocation;

/// What `base` stands for in this run's gate, owned.
pub enum Reference {
    Run(Box<Envelope>),
    /// Why there is no reference run.
    Missing(String),
}

impl Reference {
    /// The reference as the gate evaluator reads it.
    pub fn as_gate(&self) -> GateReference<'_> {
        match self {
            Self::Run(run) => GateReference::Run(run),
            Self::Missing(why) => GateReference::Missing(why.clone()),
        }
    }
}

/// The reference for `envelope`'s gate. Nothing is resolved unless the envelope is graded and a
/// rule of its gate compares with `base`. `--against` names it; otherwise a range scope (`--changed`, `--base`) is
/// compared with its base, recomputed; any other scope with the default baseline, `compared`.
pub fn resolve(
    invocation: &Invocation<'_>,
    config: &Config,
    args: &ScopeArgs,
    envelope: &Envelope,
    compared: Option<Envelope>,
) -> Reference {
    let graded = matches!(envelope.status, Status::Pass | Status::Partial);
    let Some(verb) = envelope.run.verb.filter(|_| graded) else {
        return Reference::Missing(String::new());
    };
    let rules = gate_rules(envelope, verb, args, config).map(|(_, rules)| rules);
    if !rules.is_some_and(needs_reference) {
        return Reference::Missing(String::new());
    }
    if args.against.is_none() && check::scope_request(args).has_range() {
        return base_run(invocation, config, args, rules.unwrap_or_default());
    }
    match compared {
        Some(run) => Reference::Run(Box::new(run)),
        None if args.against.is_some() => {
            Reference::Missing("the run --against names is not stored".to_owned())
        }
        None => Reference::Missing("no baseline is set".to_owned()),
    }
}

/// The base side of a range, recomputed: the checks the rules against `base` name, run over
/// the base versions of the range's changed files in a copy of the base commit (ADR-130).
fn base_run(
    invocation: &Invocation<'_>,
    config: &Config,
    args: &ScopeArgs,
    rules: &[GateRuleSpec],
) -> Reference {
    match recompute(invocation, config, args, rules) {
        Ok(run) => Reference::Run(Box::new(run)),
        Err(why) => Reference::Missing(why),
    }
}

fn recompute(
    invocation: &Invocation<'_>,
    config: &Config,
    args: &ScopeArgs,
    rules: &[GateRuleSpec],
) -> Result<Envelope, String> {
    let writer = invocation.writer;
    let languages = invocation.catalog();
    let languages = languages.language_registry();
    let scoped = check::resolve(writer, config, &check::scope_request(args), languages)
        .map_err(|error| format!("the range did not resolve: {error}"))?;
    let commit = scoped
        .files
        .base
        .clone()
        .ok_or_else(|| "the range has no base commit".to_owned())?;
    let files = check::base_files(languages, &scoped.files);
    if files.is_empty() {
        return Err("none of the range's files existed at its base".to_owned());
    }
    let git = Git::open(writer.root()).ok_or_else(|| "git cannot read the base".to_owned())?;
    // Outside the repository, so its ignore files do not hide the copy from the walk; removed
    // when `copy` drops.
    let copy = tempfile::Builder::new()
        .prefix("cupel-base-")
        .tempdir()
        .map_err(|error| format!("the base could not be written: {error}"))?;
    let root = git
        .export(&commit, &copy.path().join("tree"))
        .map_err(|error| format!("the base could not be written: {error}"))?;
    let short = &commit[..commit.len().min(12)];
    run_at(invocation, config, args, rules, &root, files)
        .map_err(|error| format!("the run at the base {short} failed: {error}"))
}

/// The checks `rules` name, run over `files` in the tree at `root`.
fn run_at(
    invocation: &Invocation<'_>,
    config: &Config,
    args: &ScopeArgs,
    rules: &[GateRuleSpec],
    root: &Path,
    files: Vec<String>,
) -> Result<Envelope, String> {
    let writer = Writer::new(root, false).map_err(|error| error.to_string())?;
    let mut only: Vec<String> = rules
        .iter()
        .filter(|rule| needs_reference(std::slice::from_ref(rule)))
        .map(|rule| rule.key.check.to_string())
        .collect();
    only.sort();
    only.dedup();
    let base_args = ScopeArgs {
        paths: files,
        only,
        jobs: args.jobs,
        no_gate: true,
        ..ScopeArgs::default()
    };
    let base = Invocation {
        run: invocation.run,
        writer: &writer,
        session: invocation.session,
        config: Ok(config),
        plugins: invocation.plugins,
        dry_run: false,
        installs: Permission::Never("a reference run installs nothing"),
    };
    let mut events: Vec<Event> = Vec::new();
    let checked =
        check::run(&base, config, &base_args, &mut events).map_err(|error| error.to_string())?;
    Ok(checked.into_envelope(invocation.run, &writer, &mut events))
}
