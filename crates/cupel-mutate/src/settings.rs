// ABOUTME: The engine's settings, from `[lang.<id>] mutation = { operators, jobs, timeout_factor, strategy }`
// ABOUTME: (ADR-080, ADR-082): the operator set, slots at once, the timeout factor, and schemata or file copies.
use cupel_core::syntax::OperatorSet;
use cupel_core::{Error, Result};
use serde_json::{Map, Value};

/// The `[lang.<id>]` key holding the engine's table.
pub const KEY: &str = "mutation";

/// The timeout factor when none is configured (design §6.7).
pub const DEFAULT_TIMEOUT_FACTOR: u32 = 10;

/// How the engine mutates and runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// `default` mutates only the default set's sites; `extended` every site.
    pub operators: OperatorSet,
    /// Slots running mutants of one file at once.
    pub jobs: usize,
    /// A mutant's run may take this many times its reference run (floor 2 s).
    pub timeout_factor: u32,
    /// How mutants are built.
    pub strategy: Strategy,
}

/// How the engine builds mutants (ADR-082).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    /// Schemata where the language has a rewriter, file copies elsewhere.
    Schemata,
    /// Every mutant a file copy.
    FileCopy,
}

impl Settings {
    /// The settings in `config`, a `[lang.<id>]` table; defaults for what it does not say.
    pub fn read(config: &Map<String, Value>) -> Result<Self> {
        let mut settings = Self {
            operators: OperatorSet::Default,
            jobs: default_jobs(),
            timeout_factor: DEFAULT_TIMEOUT_FACTOR,
            strategy: Strategy::Schemata,
        };
        let Some(table) = config.get(KEY) else {
            return Ok(settings);
        };
        let table = table.as_object().ok_or_else(|| invalid(KEY, table))?;
        for (key, value) in table {
            match key.as_str() {
                "operators" => settings.operators = operators(value)?,
                "strategy" => settings.strategy = strategy(value)?,
                "jobs" => settings.jobs = count("mutation.jobs", value)?,
                "timeout_factor" => {
                    let what = "mutation.timeout_factor";
                    settings.timeout_factor =
                        u32::try_from(count(what, value)?).map_err(|_| invalid(what, value))?;
                }
                _ => return Err(invalid("mutation", &Value::String(key.clone()))),
            }
        }
        Ok(settings)
    }
}

/// Half the cores, at least one and at most four.
fn default_jobs() -> usize {
    std::thread::available_parallelism()
        .map_or(1, |cores| cores.get() / 2)
        .clamp(1, 4)
}

fn operators(value: &Value) -> Result<OperatorSet> {
    choice(
        "mutation.operators",
        value,
        &[
            ("default", OperatorSet::Default),
            ("extended", OperatorSet::Extended),
        ],
    )
}

fn strategy(value: &Value) -> Result<Strategy> {
    choice(
        "mutation.strategy",
        value,
        &[
            ("schemata", Strategy::Schemata),
            ("file-copy", Strategy::FileCopy),
        ],
    )
}

/// `value`, the setting `what`, as the choice it names.
fn choice<T: Copy>(what: &'static str, value: &Value, choices: &[(&str, T)]) -> Result<T> {
    choices
        .iter()
        .find(|(name, _)| value.as_str() == Some(*name))
        .map(|(_, choice)| *choice)
        .ok_or_else(|| invalid(what, value))
}

/// `value`, the setting `what`, as a positive integer.
fn count(what: &'static str, value: &Value) -> Result<usize> {
    value
        .as_u64()
        .filter(|count| *count > 0)
        .and_then(|count| usize::try_from(count).ok())
        .ok_or_else(|| invalid(what, value))
}

fn invalid(what: &'static str, value: &Value) -> Error {
    Error::InvalidValue {
        what,
        value: value.to_string(),
        expected: "`mutation = { operators = \"default\" | \"extended\", jobs = <positive \
                   integer>, timeout_factor = <positive integer>, strategy = \"schemata\" | \
                   \"file-copy\" }`",
    }
}
