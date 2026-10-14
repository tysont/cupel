// ABOUTME: The self-description registry: every verb, flag, exit code, and schema, with its text.
// ABOUTME: `describe` serializes it and clap's help is rendered from it, so the two cannot drift.
//!
//! Clap derive (`cli.rs`) owns parsing; this module owns what is said about it. A unit test below
//! requires the derived command tree and this registry to agree on every name, value type, enum
//! value, requirement, and conflict (ADR-018).
//!
//! `pending_step` names the plan step that gives a verb or argument its behavior. An argument
//! carries its own marker only when that step differs from its verb's.
use clap::{Arg, Command, CommandFactory};
use cupel_core::config::{PROFILE_ENV_VAR, Setting};
use cupel_core::{
    ArgDescription, CliDescription, CommandDescription, CommandSummary, ConfigSource, Example,
    ExitCode, ExitCodeDescription, SchemaRef, Stability, ValueType, schema,
};

use crate::cli::Cli;

/// The one-sentence summary of the tool.
const SUMMARY: &str = "Cupel grades what a coding agent produced.";

/// The CLI at a glance: what `cupel describe` returns.
pub fn cli() -> CliDescription {
    CliDescription {
        name: "cupel".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        summary: SUMMARY.to_owned(),
        stability: Stability::Experimental,
        verbs: commands()
            .into_iter()
            .map(|command| CommandSummary {
                name: command.name,
                summary: command.summary,
                stability: command.stability,
                pending_step: command.pending_step,
            })
            .collect(),
        global_flags: global_flags(),
        flags: vec![switch("--version", "Print the version and exit.")],
        exit_codes: ExitCode::ALL
            .into_iter()
            .map(|code| ExitCodeDescription {
                code,
                name: code.name().to_owned(),
                meaning: code.meaning().to_owned(),
                stability: Stability::Stable,
            })
            .collect(),
        schemas: schemas(),
        config_precedence: vec![
            ConfigSource::Flags,
            ConfigSource::Env,
            ConfigSource::Profile,
            ConfigSource::CupelToml,
            ConfigSource::Defaults,
        ],
    }
}

/// Every published schema and where it lives.
pub fn schemas() -> Vec<SchemaRef> {
    schema::PUBLISHED
        .iter()
        .map(|(name, _)| SchemaRef {
            name: (*name).to_owned(),
            url: schema::url(name),
            stability: Stability::Experimental,
        })
        .collect()
}

/// The command named by `words` (such as `["baseline", "set"]`), if there is one.
pub fn find(words: &[String]) -> Option<CommandDescription> {
    let (verb, rest) = words.split_first()?;
    let command = commands().into_iter().find(|c| c.name == *verb)?;
    rest.iter().try_fold(command, |parent, word| {
        let name = format!("{} {word}", parent.name);
        parent.subcommands.into_iter().find(|c| c.name == name)
    })
}

/// The clap command tree with every `about` and `help` text taken from the registry.
pub fn command() -> Command {
    let description = cli();
    let root_args: Vec<ArgDescription> = description
        .global_flags
        .iter()
        .chain(&description.flags)
        .cloned()
        .collect();
    let mut root = Cli::command()
        .about(description.summary)
        .mut_args(|arg| with_help(arg, &root_args));
    for spec in commands() {
        root = root.mut_subcommand(spec.name.clone(), |sub| with_text(sub, &spec));
    }
    root
}

fn with_text(command: Command, spec: &CommandDescription) -> Command {
    let mut command = command
        .about(spec.summary.clone())
        .mut_args(|arg| with_help(arg, &spec.arguments));
    for sub in &spec.subcommands {
        let word = sub.name.rsplit(' ').next().unwrap_or_default().to_owned();
        command = command.mut_subcommand(word, |c| with_text(c, sub));
    }
    command
}

fn with_help(arg: Arg, specs: &[ArgDescription]) -> Arg {
    let name = registered_name(&arg);
    match specs.iter().find(|spec| spec.name == name) {
        Some(spec) => arg.help(spec.summary.clone()),
        None => arg,
    }
}

/// The name the registry uses for a clap argument: `--long` for flags, the value name otherwise.
pub fn registered_name(arg: &Arg) -> String {
    match arg.get_long() {
        Some(long) => format!("--{long}"),
        None => arg
            .get_value_names()
            .and_then(|names| names.first())
            .map_or_else(|| arg.get_id().to_string(), ToString::to_string),
    }
}

/// Every verb, in design §6.2 order, with sub-verbs nested.
pub fn commands() -> Vec<CommandDescription> {
    vec![
        init_verb(),
        doctor_verb(),
        describe_verb(),
        check_verb(),
        survey_verb(),
        explain_verb(),
        baseline_verb(),
        oracle_verb(),
        tests_verb(),
        suppress_verb(),
        serve_verb(),
    ]
}

fn init_verb() -> CommandDescription {
    verb(
        "init",
        "Detect project roots, languages, and test, coverage, and bench commands; write cupel.toml.",
        None,
        vec![switch("--force", "Replace an existing cupel.toml.")],
        examples(&[
            (
                "cupel init",
                "Preview the cupel.toml and .cupel/.gitignore init would write; writes nothing.",
                ExitCode::Pass,
            ),
            (
                "cupel init --dry-run",
                "The same preview, as a dry run.",
                ExitCode::Pass,
            ),
            (
                "cupel init --write",
                "Write cupel.toml and .cupel/.gitignore.",
                ExitCode::Pass,
            ),
        ]),
    )
}

fn doctor_verb() -> CommandDescription {
    verb(
        "doctor",
        "Verify the binary's version, the configuration, .cupel/ write access, and needed tools.",
        None,
        vec![
            switch(
                "--fix",
                "Install missing tools; with --install, without asking.",
            )
            .pending("P3.7"),
        ],
        examples(&[
            (
                "cupel doctor",
                "Check this repository's setup; exits 3 if something required is missing.",
                ExitCode::Pass,
            ),
            (
                "cupel doctor --dry-run",
                "A dry run; doctor writes nothing worth planning, so the plan is empty.",
                ExitCode::Pass,
            ),
        ]),
    )
}

fn describe_verb() -> CommandDescription {
    verb(
        "describe",
        "Describe the CLI, a verb, a check, a finding kind, a language, a fact, the support matrix, or a schema.",
        None,
        vec![
            positional(
                "SUBJECT",
                ValueType::String,
                "What to describe: a verb (optionally with a sub-verb), `languages`, `language <id>`, `fact <kind>`, `matrix`, or `schema <name>`.",
            )
            .optional()
            .repeated()
            .conflicts(&["--all", "--agent"]),
            switch("--all", "Describe everything in one document.")
                .conflicts(&["SUBJECT", "--agent"]),
            switch("--agent", "Emit a short agent guide in Markdown.")
                .pending("P8.3")
                .conflicts(&["SUBJECT", "--all"]),
        ],
        examples(&[
            ("cupel describe", "Verbs, global flags, exit codes, and schema URLs.", ExitCode::Pass),
            ("cupel describe check", "The check verb's arguments.", ExitCode::Pass),
            ("cupel describe baseline set", "A sub-verb's arguments.", ExitCode::Pass),
            ("cupel describe schema run", "The envelope's JSON Schema.", ExitCode::Pass),
            ("cupel describe languages", "Every language, one line each.", ExitCode::Pass),
            ("cupel describe language rust", "A language's extensions, family, facts, and checks.", ExitCode::Pass),
            ("cupel describe fact symbols", "A fact's schema, version, and producers per language.", ExitCode::Pass),
            ("cupel describe matrix", "Which checks can run for which languages, and why not.", ExitCode::Pass),
            ("cupel describe --all", "Everything, in one document.", ExitCode::Pass),
            ("cupel check --describe", "Any verb describes itself.", ExitCode::Pass),
        ]),
    )
}

fn check_verb() -> CommandDescription {
    verb(
        "check",
        "Run checks over a scope at a depth and evaluate the gate.",
        None,
        scope_flags(),
        examples(&[
            (
                "cupel check --only static.complexity",
                "Measure every function's complexity in the whole tree.",
                ExitCode::Pass,
            ),
            (
                "cupel check --depth quick --dry-run",
                "Show which checks a quick run would run and skip, with estimates.",
                ExitCode::Pass,
            ),
        ]),
    )
}

fn survey_verb() -> CommandDescription {
    verb(
        "survey",
        "Rank a whole tree without a baseline or gate: the worst targets per metric.",
        None,
        scope_flags(),
        examples(&[(
            "cupel survey --depth quick",
            "Find the worst hot spots quickly.",
            ExitCode::Pass,
        )]),
    )
}

fn explain_verb() -> CommandDescription {
    verb(
        "explain",
        "Give full context and remediation for one finding or check result.",
        None,
        [positional(
            "TARGET",
            ValueType::String,
            "A finding ID (fnd_…) or a check ID.",
        )]
        .into_iter()
        .chain(scope_flags())
        .collect(),
        examples(&[(
            "cupel explain fnd_7c2e1a",
            "Explain one finding.",
            ExitCode::Usage,
        )]),
    )
}

/// The `baseline` sub-verbs.
fn baseline_subcommands() -> Vec<CommandDescription> {
    vec![
        sub(
            "baseline set",
            "Name the latest result run, or a given run, as the baseline; previews without --write.",
            vec![valued(
                "--from",
                ValueType::String,
                "run:ID",
                "The run to use instead of the latest.",
            )],
            "cupel baseline set --write",
        ),
        sub(
            "baseline show",
            "Print the baseline.",
            vec![],
            "cupel baseline show",
        ),
        sub(
            "baseline diff",
            "Compare two runs as an envelope of new and resolved findings.",
            vec![valued(
                "--against",
                ValueType::String,
                "run:ID|REF",
                "baseline, run:ID, or a git ref to compare the latest result run with; default baseline.",
            )],
            "cupel baseline diff",
        ),
        sub(
            "baseline export",
            "Write the baseline to a file as JSON.",
            vec![positional(
                "FILE",
                ValueType::Path,
                "Where to write the baseline.",
            )],
            "cupel baseline export baseline.json",
        ),
        sub(
            "baseline import",
            "Read a baseline from a JSON file; previews without --write.",
            vec![positional("FILE", ValueType::Path, "The baseline to read.")],
            "cupel baseline import baseline.json",
        ),
    ]
}

fn baseline_verb() -> CommandDescription {
    CommandDescription {
        subcommands: baseline_subcommands(),
        ..verb(
            "baseline",
            "Set, show, diff, export, or import the baseline runs are compared with.",
            None,
            vec![valued(
                "--name",
                ValueType::String,
                "NAME",
                "Which named baseline; default `default`.",
            )],
            examples(&[(
                "cupel baseline show",
                "Print the baseline.",
                ExitCode::Usage,
            )]),
        )
    }
}

/// `oracle add`, which registers an oracle of one of three kinds.
fn oracle_add() -> CommandDescription {
    sub(
        "oracle add",
        "Register an oracle: a reference build, a command, or a specification.",
        vec![
            positional("NAME", ValueType::String, "The oracle's name."),
            choice(
                "--kind",
                "KIND",
                &["ref", "command", "spec"],
                "What the oracle compares against.",
            )
            .required(),
            valued(
                "--ref",
                ValueType::String,
                "REF",
                "The git ref of a reference build.",
            ),
            valued(
                "--cmd",
                ValueType::String,
                "CMD",
                "The command whose output is the reference.",
            ),
            valued("--spec", ValueType::Path, "FILE", "The specification file."),
            valued(
                "--lang",
                ValueType::String,
                "LANG",
                "The specification language: tla+, quint, or p.",
            ),
        ],
        "cupel oracle add pg --kind ref --ref v1.2.0",
    )
}

/// The `oracle` sub-verbs.
fn oracle_subcommands() -> Vec<CommandDescription> {
    vec![
        oracle_add(),
        sub(
            "oracle list",
            "List registered oracles.",
            vec![],
            "cupel oracle list",
        ),
        sub(
            "oracle remove",
            "Remove an oracle.",
            vec![positional(
                "NAME",
                ValueType::String,
                "The oracle to remove.",
            )],
            "cupel oracle remove pg",
        ),
        sub(
            "oracle verify",
            "Smoke-test an oracle and record that it is verified.",
            vec![
                positional("NAME", ValueType::String, "The oracle to verify.").optional(),
                switch("--all", "Verify every oracle."),
            ],
            "cupel oracle verify pg",
        ),
    ]
}

fn oracle_verb() -> CommandDescription {
    CommandDescription {
        subcommands: oracle_subcommands(),
        ..verb(
            "oracle",
            "Add, list, remove, or verify the oracles differential checks compare with.",
            Some("P5.2"),
            vec![],
            examples(&[(
                "cupel oracle list",
                "List registered oracles.",
                ExitCode::Usage,
            )]),
        )
    }
}

fn tests_verb() -> CommandDescription {
    verb(
        "tests",
        "List the tests that cover a scope, with a command to run them.",
        Some("P3.3"),
        [switch(
            "--impacted",
            "Select the minimal test set covering the scope.",
        )]
        .into_iter()
        .chain(scope_flags())
        .collect(),
        examples(&[(
            "cupel tests --impacted --changed",
            "Tests to run for the working tree's changes.",
            ExitCode::Usage,
        )]),
    )
}

fn suppress_verb() -> CommandDescription {
    verb(
        "suppress",
        "Suppress a finding, with a reason and an optional expiry.",
        None,
        vec![
            positional(
                "FINDING",
                ValueType::String,
                "The finding ID (fnd_…) to suppress.",
            ),
            valued(
                "--reason",
                ValueType::String,
                "TEXT",
                "Why the finding is suppressed.",
            )
            .required(),
            valued(
                "--until",
                ValueType::String,
                "REF|DATE",
                "When the suppression expires.",
            ),
        ],
        examples(&[(
            "cupel suppress fnd_7c2e1a --reason known-flake",
            "Suppress one finding.",
            ExitCode::Usage,
        )]),
    )
}

fn serve_verb() -> CommandDescription {
    verb(
        "serve",
        "Run the daemon and MCP server with warm caches and event streaming.",
        Some("P4.2"),
        vec![
            valued(
                "--socket",
                ValueType::Path,
                "PATH",
                "The Unix socket to listen on.",
            ),
            switch("--mcp", "Expose the verbs as MCP tools."),
            switch("--watch", "Reparse files as they change."),
        ],
        examples(&[(
            "cupel serve --mcp",
            "Serve the verbs over MCP.",
            ExitCode::Usage,
        )]),
    )
}

/// Flags every verb accepts (design §6.3).
fn global_flags() -> Vec<ArgDescription> {
    [invocation_flags(), run_control_flags()].concat()
}

/// What to read and where to report: format, dry run, description, configuration, root, and
/// diagnostics.
fn invocation_flags() -> Vec<ArgDescription> {
    vec![
        choice(
            "--format",
            "FORMAT",
            &["json", "jsonl", "text", "sarif"],
            "Output format; json when stdout is not a terminal, text when it is.",
        )
        .env(Setting::Format.env_var()),
        switch(
            "--dry-run",
            "Preview what would run or be written; change nothing.",
        ),
        switch(
            "--describe",
            "Describe this verb instead of running it, and exit 0.",
        ),
        valued(
            "--config",
            ValueType::Path,
            "PATH",
            "The cupel.toml to use instead of the nearest one.",
        ),
        valued(
            "--profile",
            ValueType::String,
            "NAME",
            "Apply [profile.NAME] from cupel.toml.",
        )
        .env(PROFILE_ENV_VAR),
        valued(
            "--cwd",
            ValueType::Path,
            "PATH",
            "The repository root; defaults to the current directory.",
        ),
        switch("--quiet", "Write no diagnostics to stderr.").conflicts(&["--verbose"]),
        switch("--verbose", "Write extra diagnostics to stderr.").conflicts(&["--quiet"]),
        valued(
            "--events",
            ValueType::String,
            "PATH|stderr",
            "Stream progress events as JSON lines to stderr or a file under .cupel/.",
        ),
    ]
}

/// What a run may do and how much it returns: time, seed, paging, color, installs, writes.
fn run_control_flags() -> Vec<ArgDescription> {
    vec![
        valued(
            "--timeout",
            ValueType::Duration,
            "DURATION",
            "Stop after this long, keeping partial results.",
        ),
        valued(
            "--seed",
            ValueType::Integer,
            "N",
            "Seed for generative checks; recorded in the envelope.",
        )
        .default_value("random"),
        valued(
            "--limit",
            ValueType::Integer,
            "N",
            "Return at most N findings.",
        ),
        valued(
            "--offset",
            ValueType::Integer,
            "N",
            "Skip the first N findings.",
        ),
        switch("--no-color", "Render text without color.").env("NO_COLOR"),
        switch(
            "--no-install",
            "Never install tools; the default unless stdout is a terminal.",
        )
        .pending("P3.7")
        .conflicts(&["--install"]),
        switch("--install", "Install missing tools that producers need.")
            .pending("P3.7")
            .conflicts(&["--no-install"]),
        switch("--write", "Permit writes outside .cupel/."),
        switch("--help", "Print help and exit."),
    ]
}

/// Scope, depth, and selection flags shared by the analysis verbs (design §6.4).
fn scope_flags() -> Vec<ArgDescription> {
    [
        scope_what_flags(),
        scope_how_much_flags(),
        scope_which_flags(),
    ]
    .concat()
}

/// Which code a run considers.
fn scope_what_flags() -> Vec<ArgDescription> {
    vec![
        switch(
            "--changed",
            "Scope to changes since the merge base, including the working tree.",
        ),
        valued(
            "--base",
            ValueType::String,
            "REF",
            "Start of an explicit range.",
        ),
        valued(
            "--head",
            ValueType::String,
            "REF",
            "End of an explicit range; defaults to the working tree.",
        ),
        valued(
            "--paths",
            ValueType::String,
            "P",
            "Files, directories, or globs.",
        )
        .repeated(),
        valued(
            "--symbols",
            ValueType::String,
            "S",
            "Symbols by ns::name or structural hash.",
        )
        .repeated(),
        switch(
            "--since-baseline",
            "Symbols whose structural hash changed since the baseline.",
        )
        .pending("P2.1"),
    ]
}

/// How much evidence a run gathers.
fn scope_how_much_flags() -> Vec<ArgDescription> {
    vec![
        choice(
            "--depth",
            "DEPTH",
            &["quick", "standard", "deep", "exhaustive"],
            "How much evidence to gather.",
        )
        .env(Setting::Depth.env_var())
        .default_value("standard"),
        valued(
            "--budget",
            ValueType::Duration,
            "DURATION",
            "A time budget that overrides the depth.",
        )
        .env(Setting::Budget.env_var())
        .pending("P4.3"),
    ]
}

/// Which checks run and what they are compared with.
fn scope_which_flags() -> Vec<ArgDescription> {
    vec![
        valued(
            "--only",
            ValueType::String,
            "ID",
            "Run only these checks or groups.",
        )
        .repeated(),
        valued(
            "--skip",
            ValueType::String,
            "ID",
            "Skip these checks or groups.",
        )
        .repeated(),
        valued(
            "--tier",
            ValueType::String,
            "N+",
            "Run only checks at or above tier N.",
        ),
        switch(
            "--gate",
            "Evaluate the gate for this scope; the default when the config has its table.",
        )
        .conflicts(&["--no-gate"]),
        switch("--no-gate", "Do not evaluate the gate.").conflicts(&["--gate"]),
        valued(
            "--against",
            ValueType::String,
            "baseline|REF|run:ID",
            "What deltas are computed against.",
        ),
        valued(
            "--oracle",
            ValueType::String,
            "NAME",
            "The oracle for differential checks.",
        )
        .pending("P5.3"),
        valued(
            "--cases",
            ValueType::Integer,
            "N",
            "The case budget for generative checks.",
        )
        .pending("P5.4"),
    ]
}

fn verb(
    name: &str,
    summary: &str,
    pending_step: Option<&str>,
    arguments: Vec<ArgDescription>,
    examples: Vec<Example>,
) -> CommandDescription {
    CommandDescription {
        name: name.to_owned(),
        summary: summary.to_owned(),
        stability: Stability::Experimental,
        pending_step: pending_step.map(str::to_owned),
        arguments,
        subcommands: Vec::new(),
        examples,
    }
}

/// A sub-verb; it is pending in the same step as its verb, so it carries no marker of its own.
fn sub(
    name: &str,
    summary: &str,
    arguments: Vec<ArgDescription>,
    example_command: &str,
) -> CommandDescription {
    verb(
        name,
        summary,
        None,
        arguments,
        examples(&[(example_command, summary, ExitCode::Usage)]),
    )
}

/// Examples from (command, what it does, the exit code it ends with) rows.
fn examples(rows: &[(&str, &str, ExitCode)]) -> Vec<Example> {
    rows.iter()
        .map(|(command, summary, exit_code)| Example {
            command: (*command).to_owned(),
            summary: (*summary).to_owned(),
            exit_code: *exit_code,
        })
        .collect()
}

fn switch(name: &str, summary: &str) -> ArgDescription {
    ArgDescription {
        name: name.to_owned(),
        positional: false,
        value: ValueType::Switch,
        value_name: None,
        values: Vec::new(),
        default: None,
        env: None,
        required: false,
        repeated: false,
        conflicts: Vec::new(),
        summary: summary.to_owned(),
        stability: Stability::Experimental,
        pending_step: None,
    }
}

fn valued(name: &str, value: ValueType, value_name: &str, summary: &str) -> ArgDescription {
    ArgDescription {
        value,
        value_name: Some(value_name.to_owned()),
        ..switch(name, summary)
    }
}

fn choice(name: &str, value_name: &str, values: &[&str], summary: &str) -> ArgDescription {
    ArgDescription {
        values: values.iter().map(|v| (*v).to_owned()).collect(),
        ..valued(name, ValueType::Enum, value_name, summary)
    }
}

/// A required positional argument named by its value name.
fn positional(value_name: &str, value: ValueType, summary: &str) -> ArgDescription {
    ArgDescription {
        positional: true,
        required: true,
        ..valued(value_name, value, value_name, summary)
    }
}

/// Builder-style refinements of an argument description.
trait Refine {
    fn pending(self, step: &str) -> Self;
    fn env(self, variable: &str) -> Self;
    fn default_value(self, value: &str) -> Self;
    fn conflicts(self, names: &[&str]) -> Self;
    fn required(self) -> Self;
    fn optional(self) -> Self;
    fn repeated(self) -> Self;
}

impl Refine for ArgDescription {
    fn pending(self, step: &str) -> Self {
        Self {
            pending_step: Some(step.to_owned()),
            ..self
        }
    }

    fn env(self, variable: &str) -> Self {
        Self {
            env: Some(variable.to_owned()),
            ..self
        }
    }

    fn default_value(self, value: &str) -> Self {
        Self {
            default: Some(value.to_owned()),
            ..self
        }
    }

    fn conflicts(self, names: &[&str]) -> Self {
        Self {
            conflicts: names.iter().map(|n| (*n).to_owned()).collect(),
            ..self
        }
    }

    fn required(self) -> Self {
        Self {
            required: true,
            ..self
        }
    }

    fn optional(self) -> Self {
        Self {
            required: false,
            ..self
        }
    }

    fn repeated(self) -> Self {
        Self {
            repeated: true,
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::ArgAction;

    use super::*;

    /// The variable read in place of a flag: a layered setting's, `CUPEL_PROFILE`, or
    /// `NO_COLOR`. Clap itself reads no variables, so configuration can report which layer set
    /// a value.
    fn configured_env(name: &str) -> Option<String> {
        match name {
            "--profile" => return Some(PROFILE_ENV_VAR.to_owned()),
            "--no-color" => return Some("NO_COLOR".to_owned()),
            _ => {}
        }
        Setting::ALL
            .into_iter()
            .find(|setting| format!("--{}", setting.key()) == name)
            .map(|setting| setting.env_var().to_owned())
    }

    /// What clap knows about one argument, in registry terms. Used to prove the two agree.
    fn as_registered(command: &Command, arg: &Arg) -> ArgDescription {
        assert!(
            arg.get_env().is_none(),
            "{}: configuration, not clap, reads CUPEL_* variables",
            arg.get_id()
        );
        let takes_value = matches!(arg.get_action(), ArgAction::Set | ArgAction::Append);
        let values: Vec<String> = if takes_value {
            arg.get_possible_values()
                .iter()
                .map(|v| v.get_name().to_owned())
                .collect()
        } else {
            Vec::new()
        };
        let mut conflicts: Vec<String> = command
            .get_arguments()
            .filter(|other| {
                other.get_id() != arg.get_id()
                    && command
                        .get_arg_conflicts_with(arg)
                        .iter()
                        .any(|c| c.get_id() == other.get_id())
            })
            .map(registered_name)
            .collect();
        conflicts.sort();
        ArgDescription {
            name: registered_name(arg),
            positional: arg.is_positional(),
            value: if takes_value {
                ValueType::String
            } else {
                ValueType::Switch
            },
            value_name: if takes_value {
                arg.get_value_names()
                    .and_then(|names| names.first())
                    .map(ToString::to_string)
            } else {
                None
            },
            values,
            default: None,
            env: configured_env(&registered_name(arg)),
            required: arg.is_required_set(),
            repeated: arg.get_num_args().is_some_and(|n| n.max_values() > 1),
            conflicts,
            summary: arg.get_help().map(ToString::to_string).unwrap_or_default(),
            stability: Stability::Experimental,
            pending_step: None,
        }
    }

    /// The registry entry reduced to what clap can know: value types collapse to switch or
    /// string, and defaults, stability, and pending steps are registry-only.
    fn comparable(spec: &ArgDescription) -> ArgDescription {
        let mut conflicts = spec.conflicts.clone();
        conflicts.sort();
        ArgDescription {
            value: if spec.value == ValueType::Switch {
                ValueType::Switch
            } else {
                ValueType::String
            },
            default: None,
            conflicts,
            stability: Stability::Experimental,
            pending_step: None,
            ..spec.clone()
        }
    }

    fn assert_agree(clap: &Command, specs: &[ArgDescription], path: &str) {
        let mut from_clap: Vec<ArgDescription> = clap
            .get_arguments()
            .filter(|arg| !arg.is_global_set() || path == "cupel")
            .map(|arg| as_registered(clap, arg))
            .collect();
        let mut from_registry: Vec<ArgDescription> = specs.iter().map(comparable).collect();
        from_clap.sort_by(|a, b| a.name.cmp(&b.name));
        from_registry.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(from_clap, from_registry, "arguments of `{path}`");
    }

    fn assert_command_agrees(clap: &Command, spec: &CommandDescription) {
        assert_eq!(
            clap.get_about().map(ToString::to_string).as_deref(),
            Some(spec.summary.as_str()),
            "{}",
            spec.name
        );
        assert_agree(clap, &spec.arguments, &spec.name);
        let clap_subs: Vec<String> = clap
            .get_subcommands()
            .map(|sub| format!("{} {}", spec.name, sub.get_name()))
            .collect();
        let registry_subs: Vec<String> = spec.subcommands.iter().map(|s| s.name.clone()).collect();
        assert_eq!(clap_subs, registry_subs, "sub-verbs of `{}`", spec.name);
        for sub in &spec.subcommands {
            let word = sub.name.rsplit(' ').next().unwrap();
            assert_command_agrees(clap.find_subcommand(word).unwrap(), sub);
        }
    }

    /// Asserts each of `specs` agrees with the subcommand of `root` it names.
    fn assert_commands_agree(root: &Command, specs: &[CommandDescription]) {
        for spec in specs {
            assert_command_agrees(root.find_subcommand(&spec.name).unwrap(), spec);
        }
    }

    #[test]
    fn clap_command_tree_matches_the_registry() {
        let mut root = command();
        root.build();
        let description = cli();
        let root_specs: Vec<ArgDescription> = description
            .global_flags
            .iter()
            .chain(&description.flags)
            .cloned()
            .collect();
        assert_agree(&root, &root_specs, "cupel");
        let verbs: Vec<&str> = root.get_subcommands().map(Command::get_name).collect();
        let registered: Vec<String> = commands().into_iter().map(|c| c.name).collect();
        assert_eq!(verbs, registered);
        assert_commands_agree(&root, &commands());
    }

    #[test]
    fn find_resolves_verbs_and_sub_verbs() {
        let words = |s: &str| s.split(' ').map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(find(&words("check")).unwrap().name, "check");
        assert_eq!(find(&words("baseline set")).unwrap().name, "baseline set");
        assert!(find(&words("baseline frob")).is_none());
        assert!(find(&words("frob")).is_none());
    }

    #[test]
    fn every_verb_has_an_example_and_pending_verbs_name_a_plan_step() {
        let without_examples: Vec<String> = commands()
            .into_iter()
            .filter(|command| command.examples.is_empty())
            .map(|command| command.name)
            .collect();
        assert_eq!(without_examples, Vec::<String>::new());
        let pending: Vec<(String, String)> = commands()
            .into_iter()
            .filter_map(|command| Some((command.name, command.pending_step?)))
            .collect();
        let expected = [("oracle", "P5.2"), ("tests", "P3.3"), ("serve", "P4.2")]
            .map(|(name, step)| (name.to_owned(), step.to_owned()));
        assert_eq!(pending, expected);
    }
}
