// ABOUTME: Gate rules as written in `[gate.changed]` / `[gate.tree]`, parsed into typed keys and conditions.
// ABOUTME: Parsing only; evaluating a rule against a run's metrics belongs to the gate evaluator (P2.9).
use std::fmt;

use crate::config::error::ConfigError;
use crate::id::CheckId;

/// What a gate rule constrains: a check's metric (`tests.mutation.score`) or the check itself
/// (`tests.weakening`, a statement about its findings).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateKey {
    pub check: CheckId,
    pub metric: Option<String>,
}

/// A numeric comparison operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparison {
    Ge,
    Gt,
    Le,
    Lt,
    Eq,
}

/// The number a metric is compared with; `percent` when written with a `%` suffix (`< 5%`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Threshold {
    pub value: f64,
    pub percent: bool,
}

/// The condition a gate rule imposes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GateCondition {
    /// The metric compares with the threshold as stated.
    Compare {
        op: Comparison,
        threshold: Threshold,
    },
    /// The metric compares with its value in the reference run plus `offset` (`<= base`,
    /// `>= base - 0.02`; ADR-130).
    Relative { op: Comparison, offset: f64 },
    /// The check produced no findings.
    None,
    /// The check found no breaking change.
    NoBreaking,
}

/// One parsed gate rule.
#[derive(Debug, Clone, PartialEq)]
pub struct GateRuleSpec {
    pub key: GateKey,
    pub condition: GateCondition,
}

impl GateRuleSpec {
    /// Parses one `"<key>" = "<condition>"` entry of a gate section (ADR-017).
    pub fn parse(key: &str, condition: &str) -> Result<Self, ConfigError> {
        let invalid = |reason: String| ConfigError::InvalidGateRule {
            key: key.to_owned(),
            condition: condition.to_owned(),
            reason,
        };
        let gate_key = parse_gate_key(key).map_err(invalid)?;
        let gate_condition = parse_condition(condition).map_err(invalid)?;
        let has_metric = gate_key.metric.is_some();
        match gate_condition {
            GateCondition::Compare { op, .. } | GateCondition::Relative { op, .. }
                if !has_metric =>
            {
                Err(invalid(format!(
                    "`{}` needs a metric: write `<check-id>.<metric>`",
                    op.symbol()
                )))
            }
            GateCondition::None | GateCondition::NoBreaking if has_metric => Err(invalid(format!(
                "`{}` applies to a check, not a metric: write `<check-id>`",
                gate_condition.word()
            ))),
            _ => Ok(Self {
                key: gate_key,
                condition: gate_condition,
            }),
        }
    }
}

impl Comparison {
    /// In match order: two-character operators before their one-character prefixes.
    const MATCH_ORDER: [Comparison; 5] = [Self::Ge, Self::Le, Self::Eq, Self::Gt, Self::Lt];

    /// The operator as written in `cupel.toml`.
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Ge => ">=",
            Self::Gt => ">",
            Self::Le => "<=",
            Self::Lt => "<",
            Self::Eq => "==",
        }
    }
}

impl GateCondition {
    /// The operator as written in `cupel.toml`.
    fn word(self) -> &'static str {
        match self {
            Self::Compare { op, .. } | Self::Relative { op, .. } => op.symbol(),
            Self::None => "none",
            Self::NoBreaking => "no-breaking",
        }
    }
}

impl fmt::Display for GateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.metric {
            Some(metric) => write!(f, "{}.{metric}", self.check),
            None => write!(f, "{}", self.check),
        }
    }
}

impl fmt::Display for GateCondition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compare { op, threshold } => {
                let suffix = if threshold.percent { "%" } else { "" };
                write!(f, "{} {}{suffix}", op.symbol(), threshold.value)
            }
            Self::Relative { op, offset } if *offset > 0.0 => {
                write!(f, "{} {BASE} + {offset}", op.symbol())
            }
            Self::Relative { op, offset } if *offset < 0.0 => {
                write!(f, "{} {BASE} - {}", op.symbol(), -offset)
            }
            Self::Relative { op, .. } => write!(f, "{} {BASE}", op.symbol()),
            Self::None | Self::NoBreaking => f.write_str(self.word()),
        }
    }
}

impl fmt::Display for GateRuleSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.key, self.condition)
    }
}

const GROUP_NAME: &str = "check ID must be `<group>.<name>`";

/// Parses a check ID: exactly `<group>.<name>` (ADR-017).
pub fn parse_check_id(raw: &str) -> Result<CheckId, String> {
    let segments: Vec<&str> = raw.split('.').collect();
    if segments.len() != 2 {
        return Err(GROUP_NAME.to_owned());
    }
    validate_segments(raw, &segments)?;
    Ok(CheckId::from(raw))
}

fn parse_gate_key(raw: &str) -> Result<GateKey, String> {
    let segments: Vec<&str> = raw.split('.').collect();
    match segments.len() {
        0 | 1 => return Err(GROUP_NAME.to_owned()),
        2 | 3 => {}
        _ => {
            return Err("a gate key is `<check-id>` or `<check-id>.<metric>`, \
                        with at most one metric segment"
                .to_owned());
        }
    }
    validate_segments(raw, &segments)?;
    Ok(GateKey {
        check: CheckId::from(segments[..2].join(".")),
        metric: segments.get(2).map(|metric| (*metric).to_owned()),
    })
}

fn validate_segments(raw: &str, segments: &[&str]) -> Result<(), String> {
    for segment in segments {
        if segment.is_empty() {
            return Err(format!("empty segment in `{raw}`"));
        }
        if !is_segment(segment) {
            return Err(format!("segment `{segment}` must match `[a-z][a-z0-9_-]*`"));
        }
    }
    Ok(())
}

/// One dot-separated part of an ID: `[a-z][a-z0-9_-]*`.
pub(crate) fn is_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn parse_condition(raw: &str) -> Result<GateCondition, String> {
    let text = raw.trim();
    for op in Comparison::MATCH_ORDER {
        if let Some(operand) = text.strip_prefix(op.symbol()) {
            let operand = operand.trim();
            if let Some(offset) = relative_operand(operand) {
                return Ok(GateCondition::Relative {
                    op,
                    offset: parse_offset(offset)?,
                });
            }
            let threshold = parse_threshold(operand).ok_or_else(|| {
                format!(
                    "`{}` expects a number (optionally with a `%` suffix) or `{BASE}` \
                     (optionally `{BASE} + <number>` or `{BASE} - <number>`)",
                    op.symbol()
                )
            })?;
            return Ok(GateCondition::Compare { op, threshold });
        }
    }
    let (word, operand) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    let condition = match word {
        "none" => GateCondition::None,
        "no-breaking" => GateCondition::NoBreaking,
        _ => {
            return Err("unknown operator; expected one of \
                        `>=`, `>`, `<=`, `<`, `==`, `none`, `no-breaking`"
                .to_owned());
        }
    };
    if operand.trim().is_empty() {
        Ok(condition)
    } else {
        Err(format!("`{word}` takes no operand"))
    }
}

/// The word naming the metric's value in the reference run.
const BASE: &str = "base";

/// What follows `base` when `operand` is `base` alone or `base` with an offset; `None` when the
/// operand is something else (`baseline`, a number).
fn relative_operand(operand: &str) -> Option<&str> {
    let rest = operand.strip_prefix(BASE)?;
    let boundary = rest
        .chars()
        .next()
        .is_none_or(|next| next.is_whitespace() || !next.is_ascii_alphanumeric());
    boundary.then(|| rest.trim())
}

/// The signed offset written after `base`: nothing, or `+`/`-` and a non-negative number in
/// the metric's unit.
fn parse_offset(offset: &str) -> Result<f64, String> {
    if offset.is_empty() {
        return Ok(0.0);
    }
    let (sign, factor, magnitude) = match (offset.strip_prefix('+'), offset.strip_prefix('-')) {
        (Some(magnitude), _) => ('+', 1.0, magnitude.trim()),
        (_, Some(magnitude)) => ('-', -1.0, magnitude.trim()),
        _ => {
            return Err(format!(
                "`{BASE}` takes an offset written `{BASE} + <number>` or `{BASE} - <number>`"
            ));
        }
    };
    if let Some(Threshold {
        value,
        percent: true,
    }) = parse_threshold(magnitude)
    {
        return Err(format!(
            "an offset from `{BASE}` is a number in the metric's unit, not a percentage: \
             write `{BASE} {sign} {}`",
            value / 100.0
        ));
    }
    match magnitude.parse::<f64>() {
        Ok(value) if value.is_finite() && value >= 0.0 => Ok(factor * value),
        _ => Err(format!(
            "`{BASE} {sign}` expects a non-negative number, such as `{BASE} {sign} 0.02`"
        )),
    }
}

fn parse_threshold(operand: &str) -> Option<Threshold> {
    let (number, percent) = match operand.strip_suffix('%') {
        Some(number) => (number, true),
        None => (operand, false),
    };
    let value: f64 = number.parse().ok()?;
    value.is_finite().then_some(Threshold { value, percent })
}
