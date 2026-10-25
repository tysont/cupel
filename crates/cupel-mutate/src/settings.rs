// ABOUTME: The engine's settings, from `[lang.<id>] mutation = { operators, jobs, timeout_factor, strategy }` (ADR-080,
// ABOUTME: ADR-082) and the cap a check declares for its fact (ADR-147): operators, slots, timeouts, and how mutants build.
use std::time::Duration;

use cupel_core::config::TimeLimit;
use cupel_core::syntax::OperatorSet;
use cupel_core::{Error, Result};
use serde_json::{Map, Value};

/// The `[lang.<id>]` key holding the engine's table.
pub const KEY: &str = "mutation";

/// The timeout factor when none is configured (design §6.7).
pub const DEFAULT_TIMEOUT_FACTOR: u32 = 10;

/// The setting a check declares for the engine's fact that caps a mutant's run (ADR-147).
pub const MAX_MUTANT_TIMEOUT: &str = "max_mutant_timeout";

/// The cap when none is configured (ADR-147).
pub const DEFAULT_MAX_MUTANT_TIMEOUT: Duration = Duration::from_secs(120);

/// The shortest timeout a mutant's run gets (design §9.3).
const TIMEOUT_FLOOR: Duration = Duration::from_secs(2);

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
    /// The longest a mutant's run may take, whatever its reference (ADR-147).
    pub max_mutant_timeout: Duration,
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
    /// The settings in `config`, a `[lang.<id>]` table, and in `declared`, the settings checks
    /// declare for the engine's fact (`ProduceCtx::settings`); defaults for what they do not say.
    pub fn read(config: &Map<String, Value>, declared: &Map<String, Value>) -> Result<Self> {
        let mut settings = Self {
            operators: OperatorSet::Default,
            jobs: default_jobs(),
            timeout_factor: DEFAULT_TIMEOUT_FACTOR,
            strategy: Strategy::Schemata,
            max_mutant_timeout: DEFAULT_MAX_MUTANT_TIMEOUT,
        };
        if let Some(value) = declared.get(MAX_MUTANT_TIMEOUT) {
            settings.max_mutant_timeout = cap(value)?;
        }
        let Some(table) = config.get(KEY) else {
            return Ok(settings);
        };
        let table = table.as_object().ok_or_else(|| invalid(KEY, table))?;
        for (key, value) in table {
            match key.as_str() {
                "operators" => settings.operators = choice("mutation.operators", value, OPERATORS)?,
                "strategy" => settings.strategy = choice("mutation.strategy", value, STRATEGIES)?,
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

    /// The timeout of a mutant whose reference run took `reference`: `timeout_factor` times it,
    /// at least two seconds, at most `max_mutant_timeout`.
    pub fn timeout(&self, reference: Duration) -> Duration {
        TIMEOUT_FLOOR
            .max(reference * self.timeout_factor)
            .min(self.max_mutant_timeout)
    }
}

/// `value`, the cap, as a duration such as `120s` or `2m`.
fn cap(value: &Value) -> Result<Duration> {
    value
        .as_str()
        .and_then(|text| text.parse::<TimeLimit>().ok())
        .map(TimeLimit::as_duration)
        .ok_or_else(|| Error::InvalidValue {
            what: MAX_MUTANT_TIMEOUT,
            value: value.to_string(),
            expected: "a duration such as `120s` or `2m`",
        })
}

/// The operator sets, by name.
const OPERATORS: &[(&str, OperatorSet)] = &[
    ("default", OperatorSet::Default),
    ("extended", OperatorSet::Extended),
];

/// The strategies, by name.
const STRATEGIES: &[(&str, Strategy)] = &[
    ("schemata", Strategy::Schemata),
    ("file-copy", Strategy::FileCopy),
];

/// Half the cores, at least one and at most four.
fn default_jobs() -> usize {
    std::thread::available_parallelism()
        .map_or(1, |cores| cores.get() / 2)
        .clamp(1, 4)
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
