//! Two builder setters below the clone floor and two name accessors above it (ADR-158).

/// How a run reused an earlier one.
#[derive(Debug, Clone, Copy)]
pub struct Reuse;

/// How long a run may take.
#[derive(Debug, Clone, Copy)]
pub struct Limit;

/// A run's results.
#[derive(Debug, Default)]
pub struct Results {
    reuse: Option<Reuse>,
}

impl Results {
    /// Records how the run reused an earlier one.
    pub fn with_reuse(mut self, reuse: Reuse) -> Self {
        self.reuse = Some(reuse);
        self
    }
}

/// A run's plan.
#[derive(Debug, Default)]
pub struct Plan {
    limit: Option<Limit>,
}

impl Plan {
    /// Records the run's time limit.
    pub fn with_limit(mut self, limit: Limit) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// Where an oracle's expected values come from.
#[derive(Debug, Clone, Copy)]
pub enum OracleKind {
    Ref,
    Command,
    Spec,
}

impl OracleKind {
    /// The kind's name in configuration.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ref => "ref",
            Self::Command => "command",
            Self::Spec => "spec",
        }
    }
}

/// A phase of a suite run.
#[derive(Debug, Clone, Copy)]
pub enum Phase {
    Build,
    Test,
    Report,
}

impl Phase {
    /// The phase's name in the envelope.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Test => "test",
            Self::Report => "report",
        }
    }
}
