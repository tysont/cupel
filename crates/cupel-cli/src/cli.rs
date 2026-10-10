// ABOUTME: The command-line grammar as clap derive types: global flags, shared scope flags, verbs.
// ABOUTME: Holds no human text; help and descriptions come from the registry (ADR-018).
use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use cupel_core::Verb;

#[derive(Debug, Parser)]
#[command(
    name = "cupel",
    version,
    arg_required_else_help = true,
    disable_help_flag = true,
    disable_version_flag = true,
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,
    #[arg(long, action = ArgAction::Version)]
    pub version: Option<bool>,
    #[command(subcommand)]
    pub verb: Option<VerbArgs>,
}

/// Flags every verb accepts (design §6.3).
#[derive(Debug, Args)]
#[command(next_help_heading = "Global flags")]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each bool is an independent command-line switch"
)]
pub struct GlobalArgs {
    #[arg(long, global = true, value_name = "FORMAT", env = "CUPEL_FORMAT")]
    pub format: Option<Format>,
    // Parsed now; the plan envelope arrives with P0.7.
    #[arg(long, global = true)]
    pub dry_run: bool,
    #[arg(long, global = true)]
    pub describe: bool,
    // Parsed now; configuration discovery and loading are P0.4.
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,
    // Parsed now; profiles are P0.4.
    #[arg(long, global = true, value_name = "NAME")]
    pub profile: Option<String>,
    #[arg(long, global = true, value_name = "PATH")]
    pub cwd: Option<PathBuf>,
    #[arg(long, global = true, conflicts_with = "verbose")]
    pub quiet: bool,
    #[arg(long, global = true, conflicts_with = "quiet")]
    pub verbose: bool,
    // Parsed now; the event sink is P0.7.
    #[arg(long, global = true, value_name = "PATH|stderr")]
    pub events: Option<String>,
    // Parsed now; duration parsing and cancellation are P0.7.
    #[arg(long, global = true, value_name = "DURATION")]
    pub timeout: Option<String>,
    #[arg(long, global = true, value_name = "N")]
    pub seed: Option<u64>,
    // Parsed now; paging is P0.5.
    #[arg(long, global = true, value_name = "N")]
    pub limit: Option<u64>,
    // Parsed now; paging is P0.5.
    #[arg(long, global = true, value_name = "N")]
    pub offset: Option<u64>,
    // Parsed now; the text renderer has no color until P0.5.
    #[arg(long, global = true)]
    pub no_color: bool,
    // Parsed now; tool installation arrives with the first tool producers in P3.1.
    #[arg(long, global = true, conflicts_with = "install")]
    pub no_install: bool,
    // Parsed now; tool installation arrives with the first tool producers in P3.1.
    #[arg(long, global = true, conflicts_with = "no_install")]
    pub install: bool,
    // Parsed now; the `Writer` that enforces it is P0.7.
    #[arg(long, global = true)]
    pub write: bool,
    #[arg(long, global = true, action = ArgAction::Help)]
    pub help: Option<bool>,
}

/// Output formats (design §6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Json,
    Jsonl,
    Text,
    Sarif,
}

/// Scope, depth, and selection flags shared by the analysis verbs (design §6.4). They parse now
/// and take effect in the steps the registry names.
#[derive(Debug, Args)]
#[command(next_help_heading = "Scope, depth, and selection")]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each bool is an independent command-line switch"
)]
pub struct ScopeArgs {
    #[arg(long)]
    pub changed: bool,
    #[arg(long, value_name = "REF")]
    pub base: Option<String>,
    #[arg(long, value_name = "REF")]
    pub head: Option<String>,
    #[arg(long, value_name = "P", num_args = 1..)]
    pub paths: Vec<String>,
    #[arg(long, value_name = "S", num_args = 1..)]
    pub symbols: Vec<String>,
    #[arg(long)]
    pub since_baseline: bool,
    #[arg(long, value_name = "DEPTH")]
    pub depth: Option<Depth>,
    #[arg(long, value_name = "DURATION")]
    pub budget: Option<String>,
    #[arg(long, value_name = "ID", num_args = 1..)]
    pub only: Vec<String>,
    #[arg(long, value_name = "ID", num_args = 1..)]
    pub skip: Vec<String>,
    #[arg(long, value_name = "N+")]
    pub tier: Option<String>,
    #[arg(long, conflicts_with = "no_gate")]
    pub gate: bool,
    #[arg(long, conflicts_with = "gate")]
    pub no_gate: bool,
    #[arg(long, value_name = "baseline|REF|run:ID")]
    pub against: Option<String>,
    #[arg(long, value_name = "NAME")]
    pub oracle: Option<String>,
    #[arg(long, value_name = "N")]
    pub cases: Option<u64>,
}

/// Run depth; the names are the cost classes (design §5.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Depth {
    Quick,
    Standard,
    Deep,
    Exhaustive,
}

/// The verbs of design §6.2. The analysis verbs carry `ScopeArgs` for parsing even though their
/// behavior lands later, so the command line is stable from P0.3 on.
#[derive(Debug, Subcommand)]
pub enum VerbArgs {
    Init {
        #[arg(long)]
        force: bool,
    },
    Doctor {
        #[arg(long)]
        fix: bool,
    },
    Describe(DescribeArgs),
    Check {
        #[command(flatten)]
        scope: ScopeArgs,
    },
    Survey {
        #[command(flatten)]
        scope: ScopeArgs,
    },
    Explain {
        #[arg(value_name = "TARGET")]
        target: String,
        #[command(flatten)]
        scope: ScopeArgs,
    },
    Baseline {
        #[arg(long, value_name = "NAME")]
        name: Option<String>,
        #[command(subcommand)]
        action: BaselineArgs,
    },
    Oracle {
        #[command(subcommand)]
        action: OracleArgs,
    },
    Tests {
        #[arg(long)]
        impacted: bool,
        #[command(flatten)]
        scope: ScopeArgs,
    },
    Suppress {
        #[arg(value_name = "FINDING")]
        finding: String,
        #[arg(long, value_name = "TEXT", required = true)]
        reason: Option<String>,
        #[arg(long, value_name = "REF|DATE")]
        until: Option<String>,
    },
    Serve {
        #[arg(long, value_name = "PATH")]
        socket: Option<PathBuf>,
        #[arg(long)]
        mcp: bool,
        #[arg(long)]
        watch: bool,
    },
}

impl VerbArgs {
    pub fn verb(&self) -> Verb {
        match self {
            Self::Init { .. } => Verb::Init,
            Self::Doctor { .. } => Verb::Doctor,
            Self::Describe(_) => Verb::Describe,
            Self::Check { .. } => Verb::Check,
            Self::Survey { .. } => Verb::Survey,
            Self::Explain { .. } => Verb::Explain,
            Self::Baseline { .. } => Verb::Baseline,
            Self::Oracle { .. } => Verb::Oracle,
            Self::Tests { .. } => Verb::Tests,
            Self::Suppress { .. } => Verb::Suppress,
            Self::Serve { .. } => Verb::Serve,
        }
    }

    /// The words naming the command after `cupel`, such as `["baseline", "set"]`.
    pub fn words(&self) -> Vec<String> {
        let mut words = vec![verb_name(self.verb())];
        match self {
            Self::Baseline { action, .. } => words.push(action.name().to_owned()),
            Self::Oracle { action } => words.push(action.name().to_owned()),
            _ => {}
        }
        words
    }
}

/// A verb's command-line spelling.
pub fn verb_name(verb: Verb) -> String {
    serde_json::to_value(verb)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

#[derive(Debug, Args)]
pub struct DescribeArgs {
    #[arg(value_name = "SUBJECT", num_args = 0..=2, conflicts_with_all = ["all", "agent"])]
    pub subject: Vec<String>,
    #[arg(long, conflicts_with_all = ["subject", "agent"])]
    pub all: bool,
    #[arg(long, conflicts_with_all = ["subject", "all"])]
    pub agent: bool,
}

#[derive(Debug, Subcommand)]
pub enum BaselineArgs {
    Set {
        #[arg(long, value_name = "run:ID")]
        from: Option<String>,
    },
    Show,
    Diff {
        #[arg(long, value_name = "run:ID|REF")]
        against: Option<String>,
    },
    Export {
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
    Import {
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
}

impl BaselineArgs {
    fn name(&self) -> &'static str {
        match self {
            Self::Set { .. } => "set",
            Self::Show => "show",
            Self::Diff { .. } => "diff",
            Self::Export { .. } => "export",
            Self::Import { .. } => "import",
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum OracleArgs {
    Add {
        #[arg(value_name = "NAME")]
        name: String,
        #[arg(long, value_name = "KIND")]
        kind: OracleKind,
        #[arg(long = "ref", value_name = "REF")]
        reference: Option<String>,
        #[arg(long, value_name = "CMD")]
        cmd: Option<String>,
        #[arg(long, value_name = "FILE")]
        spec: Option<PathBuf>,
        #[arg(long, value_name = "LANG")]
        lang: Option<String>,
    },
    List,
    Remove {
        #[arg(value_name = "NAME")]
        name: String,
    },
    Verify {
        #[arg(value_name = "NAME", required_unless_present = "all")]
        name: Option<String>,
        #[arg(long)]
        all: bool,
    },
}

impl OracleArgs {
    fn name(&self) -> &'static str {
        match self {
            Self::Add { .. } => "add",
            Self::List => "list",
            Self::Remove { .. } => "remove",
            Self::Verify { .. } => "verify",
        }
    }
}

/// What an oracle compares against (design §6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OracleKind {
    Ref,
    Command,
    Spec,
}
