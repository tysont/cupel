//! Three `as_str` accessors of 30 nodes each, which map enum variants to names: a clone cluster
//! reported at `info` with the shape `enum-mapping` (ADR-158 §3).

/// Where an oracle's expected values come from.
#[derive(Debug, Clone, Copy)]
pub enum Source {
    Reference,
    Command,
}

impl Source {
    /// The source's name in configuration.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::Command => "command",
        }
    }
}

/// A phase of a suite run.
#[derive(Debug, Clone, Copy)]
pub enum Phase {
    Build,
    Test,
}

impl Phase {
    /// The phase's name in the envelope.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Test => "test",
        }
    }
}

/// How a run reused an earlier one.
#[derive(Debug, Clone, Copy)]
pub enum Reuse {
    Fresh,
    Cached,
}

impl Reuse {
    /// The reuse's name in the run record.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Cached => "cached",
        }
    }
}
