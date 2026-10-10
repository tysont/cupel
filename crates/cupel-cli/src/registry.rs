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
        "Detect languages, project roots, and test commands; write cupel.toml.",
        Some("P0.8"),
        vec![switch("--force", "Overwrite an existing cupel.toml.")],
        vec![example(
            "cupel init --dry-run",
            "Show the cupel.toml init would write.",
            ExitCode::Usage,
        )],
    )
}

fn doctor_verb() -> CommandDescription {
    verb(
        "doctor",
        "Verify the toolchain, tool producers, oracles, and configuration.",
        Some("P0.8"),
        vec![switch("--fix", "Attempt to fix what doctor finds.")],
        vec![example(
            "cupel doctor",
            "Check this repository's setup.",
            ExitCode::Usage,
        )],
    )
}

fn describe_verb() -> CommandDescription {
    verb(
        "describe",
        "Describe the CLI, a verb, a check, a finding kind, or a schema.",
        None,
        vec![
            positional(
                "SUBJECT",
                ValueType::String,
                "What to describe: a verb (optionally with a sub-verb), or `schema <name>`.",
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
        vec![
            example(
                "cupel describe",
                "Verbs, global flags, exit codes, and schema URLs.",
                ExitCode::Pass,
            ),
            example(
                "cupel describe check",
                "The check verb's arguments.",
                ExitCode::Pass,
            ),
            example(
                "cupel describe baseline set",
                "A sub-verb's arguments.",
                ExitCode::Pass,
            ),
            example(
                "cupel describe schema run",
                "The envelope's JSON Schema.",
                ExitCode::Pass,
            ),
            example(
                "cupel describe --all",
                "Everything, in one document.",
                ExitCode::Pass,
            ),
            example(
                "cupel check --describe",
                "Any verb describes itself.",
                ExitCode::Pass,
            ),
        ],
    )
}

fn check_verb() -> CommandDescription {
    verb(
        "check",
        "Run checks over a scope at a depth and evaluate the gate.",
        Some("P2.1"),
        scope_flags(),
        vec![example(
            "cupel check --changed --depth standard --gate",
            "Grade the working tree's changes against the gate.",
            ExitCode::Usage,
        )],
    )
}

fn survey_verb() -> CommandDescription {
    verb(
        "survey",
        "Rank a whole tree without a baseline: hot spots, weak tests, clones.",
        Some("P2.9"),
        scope_flags(),
        vec![example(
            "cupel survey --depth quick",
            "Find the worst hot spots quickly.",
            ExitCode::Usage,
        )],
    )
}

fn explain_verb() -> CommandDescription {
    verb(
        "explain",
        "Give full context and remediation for one finding or check result.",
        Some("P2.2"),
        [positional(
            "TARGET",
            ValueType::String,
            "A finding ID (fnd_…) or a check ID.",
        )]
        .into_iter()
        .chain(scope_flags())
        .collect(),
        vec![example(
            "cupel explain fnd_7c2e1a",
            "Explain one finding.",
            ExitCode::Usage,
        )],
    )
}

fn baseline_verb() -> CommandDescription {
    CommandDescription {
        subcommands: vec![
            sub(
                "baseline set",
                "Name the current run, or a given run, as the baseline.",
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
                    "The run or git ref to compare with.",
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
                "Read a baseline from a JSON file.",
                vec![positional("FILE", ValueType::Path, "The baseline to read.")],
                "cupel baseline import baseline.json",
            ),
        ],
        ..verb(
            "baseline",
            "Set, show, diff, export, or import the baseline runs are compared with.",
            Some("P2.2"),
            vec![valued(
                "--name",
                ValueType::String,
                "NAME",
                "Which named baseline; default `default`.",
            )],
            vec![example(
                "cupel baseline show",
                "Print the baseline.",
                ExitCode::Usage,
            )],
        )
    }
}

fn oracle_verb() -> CommandDescription {
    CommandDescription {
        subcommands: vec![
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
            ),
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
        ],
        ..verb(
            "oracle",
            "Add, list, remove, or verify the oracles differential checks compare with.",
            Some("P5.2"),
            vec![],
            vec![example(
                "cupel oracle list",
                "List registered oracles.",
                ExitCode::Usage,
            )],
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
        vec![example(
            "cupel tests --impacted --changed",
            "Tests to run for the working tree's changes.",
            ExitCode::Usage,
        )],
    )
}

fn suppress_verb() -> CommandDescription {
    verb(
        "suppress",
        "Suppress a finding, with a reason and an optional expiry.",
        Some("P2.2"),
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
        vec![example(
            "cupel suppress fnd_7c2e1a --reason known-flake",
            "Suppress one finding.",
            ExitCode::Usage,
        )],
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
        vec![example(
            "cupel serve --mcp",
            "Serve the verbs over MCP.",
            ExitCode::Usage,
        )],
    )
}

/// Flags every verb accepts (design §6.3).
fn global_flags() -> Vec<ArgDescription> {
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
        .pending("P3.1")
        .conflicts(&["--install"]),
        switch("--install", "Install missing tools that producers need.")
            .pending("P3.1")
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
        )
        .pending("P2.1"),
        valued(
            "--base",
            ValueType::String,
            "REF",
            "Start of an explicit range.",
        )
        .pending("P2.1"),
        valued(
            "--head",
            ValueType::String,
            "REF",
            "End of an explicit range; defaults to the working tree.",
        )
        .pending("P2.1"),
        valued(
            "--paths",
            ValueType::String,
            "P",
            "Files, directories, or globs.",
        )
        .repeated()
        .pending("P2.1"),
        valued(
            "--symbols",
            ValueType::String,
            "S",
            "Symbols by ns::name or structural hash.",
        )
        .repeated()
        .pending("P2.1"),
        switch(
            "--since-baseline",
            "Symbols whose structural hash changed since the baseline.",
        )
        .pending("P2.2"),
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
        .default_value("standard")
        .pending("P2.1"),
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
        .repeated()
        .pending("P2.1"),
        valued(
            "--skip",
            ValueType::String,
            "ID",
            "Skip these checks or groups.",
        )
        .repeated()
        .pending("P2.1"),
        valued(
            "--tier",
            ValueType::String,
            "N+",
            "Run only checks at or above tier N.",
        )
        .pending("P2.1"),
        switch(
            "--gate",
            "Evaluate the gate; the default when the config has one.",
        )
        .pending("P2.9")
        .conflicts(&["--no-gate"]),
        switch("--no-gate", "Do not evaluate the gate.")
            .pending("P2.9")
            .conflicts(&["--gate"]),
        valued(
            "--against",
            ValueType::String,
            "baseline|REF|run:ID",
            "What deltas are computed against.",
        )
        .pending("P2.2"),
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
        vec![example(example_command, summary, ExitCode::Usage)],
    )
}

fn example(command: &str, summary: &str, exit_code: ExitCode) -> Example {
    Example {
        command: command.to_owned(),
        summary: summary.to_owned(),
        exit_code,
    }
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
        for spec in commands() {
            assert_command_agrees(root.find_subcommand(&spec.name).unwrap(), &spec);
        }
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
        for command in commands() {
            assert!(!command.examples.is_empty(), "{}", command.name);
            if let Some(step) = &command.pending_step {
                assert!(step.starts_with('P') && step.contains('.'), "{step}");
            }
        }
    }
}
