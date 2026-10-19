// ABOUTME: One invocation's run state: ID, start time, verb, seed, and time limit, and its envelopes.
// ABOUTME: Successful verbs and every error path assemble their envelope here, so all share one shape.
use std::cell::RefCell;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use cupel_core::{
    Cancellation, ConfigEcho, CostClass, CupelInfo, Description, Envelope, Event, EventSink,
    ExitCode, Finding, LangId, MutationOutcomes, RunId, RunInfo, RunPlan, SeverityCounts, Status,
    Summary, TestResults, ToolchainInfo, Verb, Writer, schema,
};

use cupel_core::budget::TaskDuration;
use cupel_core::config::Config;

use crate::error::CupelError;

pub struct Run {
    id: RunId,
    started: SystemTime,
    clock: Instant,
    /// Fires when `--timeout` passes, counted from the start of the invocation, or when
    /// `parent` fires.
    cancel: Cancellation,
    /// The invocation's outer token: never fired in a cold process; the daemon's request token,
    /// which a client's hang-up or the daemon's stop fires (ADR-085 §7).
    parent: Cancellation,
    pub verb: Option<Verb>,
    /// `--seed`, or a random seed once the command line parsed (ADR-020 §7).
    pub seed: u64,
    /// The command line after `cupel`, for repro strings.
    pub args: Vec<String>,
    /// The effective configuration; empty until configuration loads.
    pub config: ConfigEcho,
    /// The effective depth; null until configuration loads.
    pub depth: Option<CostClass>,
    /// The effective budget in milliseconds, when one is set.
    pub budget_ms: Option<u64>,
    /// What this run executed, until the run record indexes it in the store.
    executions: RefCell<Executions>,
}

/// What a run executed, by language: its test runs, green or red (ADR-076 §7), and its mutation
/// outcomes (ADR-080 §10), which the store indexes with the run record.
#[derive(Debug, Default)]
pub struct Executions {
    pub test_results: Vec<(LangId, TestResults)>,
    pub mutation_outcomes: Vec<(LangId, MutationOutcomes)>,
    /// How long its productions and check runs took (ADR-086 §2).
    pub durations: Vec<TaskDuration>,
}

impl Run {
    pub fn start(args: Vec<String>) -> Self {
        Self {
            id: RunId::generate(),
            started: SystemTime::now(),
            clock: Instant::now(),
            cancel: Cancellation::new(),
            parent: Cancellation::new(),
            verb: None,
            seed: 0,
            args,
            config: ConfigEcho::default(),
            depth: None,
            budget_ms: None,
            executions: RefCell::default(),
        }
    }

    pub fn id(&self) -> RunId {
        self.id
    }

    /// When the invocation started, as the envelope's `run.started` reports it.
    pub fn started(&self) -> String {
        rfc3339(self.started)
    }

    /// Puts this run's cancellation under `parent`, so it fires when `parent` does.
    pub fn within(&mut self, parent: &Cancellation) {
        self.parent = parent.clone();
        self.cancel = parent.under(None);
    }

    /// Cancels this run's work `limit` after the invocation started (`--timeout`).
    pub fn set_timeout(&mut self, limit: Duration) {
        self.cancel = self.parent.under(Some(self.clock + limit));
    }

    /// Keeps `executed`, what this run executed, for the store.
    pub fn keep(&self, executed: Executions) {
        let mut kept = self.executions.borrow_mut();
        kept.test_results.extend(executed.test_results);
        kept.mutation_outcomes.extend(executed.mutation_outcomes);
        kept.durations.extend(executed.durations);
    }

    /// When `limit` after the invocation started passes, as `--timeout` counts (ADR-086 §6).
    pub fn deadline_after(&self, limit: Duration) -> Instant {
        self.clock + limit
    }

    /// What was kept so far, leaving nothing.
    pub fn take_executions(&self) -> Executions {
        self.executions.take()
    }

    pub fn cancellation(&self) -> &Cancellation {
        &self.cancel
    }

    /// The envelope of a verb that ran to completion.
    pub fn passed(&self, description: Option<Description>) -> Envelope {
        self.envelope(Status::Pass, ExitCode::Pass, Vec::new(), description)
    }

    /// The envelope of a verb that ran and reports `findings` with this outcome.
    pub fn reported(
        &self,
        status: Status,
        exit_code: ExitCode,
        findings: Vec<Finding>,
    ) -> Envelope {
        self.envelope(status, exit_code, findings, None)
    }

    /// The plan envelope for a dry run: the verb's plan plus every write its recording writer
    /// recorded, announced by one `plan` event.
    pub fn plan_envelope(
        &self,
        mut plan: RunPlan,
        writer: &Writer,
        events: &mut dyn EventSink,
        findings: Vec<Finding>,
    ) -> Envelope {
        plan.would_write.extend(writer.planned());
        plan.est_total_ms = plan.would_run.iter().map(|check| check.est_ms).sum();
        events.emit(Event::Plan {
            run: self.id(),
            checks: u32::try_from(plan.would_run.len()).unwrap_or(u32::MAX),
            est_ms: plan.est_total_ms,
        });
        self.planned(plan, findings)
    }

    /// Records the loaded configuration's echo, depth, and budget for the envelope.
    pub fn configure(&mut self, config: &Config) {
        self.config = config.echo();
        self.depth = Some(config.effective.depth);
        self.budget_ms = config
            .effective
            .budget
            .map(|budget| u64::try_from(budget.as_duration().as_millis()).unwrap_or(u64::MAX));
    }

    /// The envelope of a `--dry-run`: the plan, what planning found, and nothing done.
    pub fn planned(&self, plan: RunPlan, findings: Vec<Finding>) -> Envelope {
        let mut envelope = self.envelope(Status::Plan, ExitCode::Pass, findings, None);
        envelope.plan = Some(plan);
        envelope
    }

    /// `envelope` for a run whose work was cut short by cancellation: a finished result becomes
    /// `partial` (exit 5) with everything it holds kept; an error stays an error (ADR-020 §5).
    pub fn interrupted(envelope: Envelope) -> Envelope {
        if envelope.status == Status::Error {
            return envelope;
        }
        Envelope {
            status: Status::Partial,
            exit_code: ExitCode::Partial,
            ..envelope
        }
    }

    /// The envelope reporting `error` as its findings: one, except for a red baseline, which
    /// has one per failing test and incomplete run (ADR-076 §6).
    pub fn failed(&self, error: &CupelError) -> Envelope {
        let findings = error.findings(&self.repro());
        self.envelope(Status::Error, error.exit_code(), findings, None)
    }

    /// The command line that reproduces this run.
    pub fn repro(&self) -> String {
        std::iter::once("cupel".to_owned())
            .chain(self.args.iter().map(|arg| shell_quote(arg)))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn envelope(
        &self,
        status: Status,
        exit_code: ExitCode,
        findings: Vec<Finding>,
        description: Option<Description>,
    ) -> Envelope {
        let counts = SeverityCounts::of(&findings);
        let total_findings = findings.len() as u64;
        Envelope {
            cupel: CupelInfo {
                version: env!("CARGO_PKG_VERSION").to_owned(),
                schema: schema::url("run"),
            },
            run: RunInfo {
                id: self.id,
                verb: self.verb,
                started: self.started(),
                duration_ms: u64::try_from(self.clock.elapsed().as_millis()).unwrap_or(u64::MAX),
                git: None,
                scope: None,
                depth: self.depth,
                budget_ms: self.budget_ms,
                seed: self.seed,
                toolchain: ToolchainInfo::default(),
                config: self.config.clone(),
                baseline: None,
            },
            status,
            exit_code,
            checks: Vec::new(),
            findings,
            summary: Summary {
                findings: counts,
                new: 0,
                resolved: 0,
                suppressed: 0,
                gate: None,
            },
            next_actions: Vec::new(),
            description,
            plan: None,
            explanation: None,
            rankings: None,
            tests: None,
            oracles: None,
            truncated: false,
            total_findings,
        }
    }
}

/// Quotes an argument for a POSIX shell when it contains anything but safe characters.
pub(crate) fn shell_quote(arg: &str) -> String {
    let safe = !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./=:,+@%".contains(c));
    if safe {
        arg.to_owned()
    } else {
        format!("'{}'", arg.replace('\'', r"'\''"))
    }
}

/// `time` as RFC 3339 in UTC with second precision, such as `2026-10-10T16:31:04Z`.
fn rfc3339(time: SystemTime) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let days = i64::try_from(seconds / 86_400).unwrap_or(0);
    let of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        of_day / 3600,
        of_day % 3600 / 60,
        of_day % 60
    )
}

/// Days since 1970-01-01 to a proleptic Gregorian date (Howard Hinnant's `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = u32::try_from(day_of_year - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let month = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// What a verb runs with: this run, the repository's writer, the configuration or why it did not
/// load, and whether this is a dry run.
pub struct Invocation<'a> {
    pub run: &'a Run,
    pub writer: &'a Writer,
    /// The caches this invocation may find warm (ADR-085 §3).
    pub session: &'a crate::session::Session,
    pub config: Result<&'a Config, &'a cupel_core::Error>,
    /// The plugins discovered for this invocation (ADR-088); none when configuration did not
    /// load.
    pub plugins: &'a cupel_plugin::Plugins,
    pub dry_run: bool,
    /// Whether the run may install the tools its producers need (ADR-081).
    pub installs: crate::install::Permission,
}

impl Invocation<'_> {
    /// The configuration, or the usage error it failed to load with.
    pub fn config(&self) -> Result<&Config, CupelError> {
        self.config.map_err(|error| CupelError::from(error.clone()))
    }

    /// The registries this invocation reads: the compiled-in ones, parsing through the session's
    /// cache, with its plugins.
    pub fn catalog(&self) -> crate::catalog::Catalog {
        crate::catalog::Catalog::with_cache(std::sync::Arc::clone(self.session.cache()))
            .with_plugins(self.plugins)
    }

    /// A plan that would do nothing: the dry run of a verb that changes nothing.
    pub fn nothing_planned(&self, events: &mut dyn EventSink) -> Envelope {
        self.run
            .plan_envelope(RunPlan::default(), self.writer, events, Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn rfc3339_matches_independently_computed_dates() {
        // Expected values from Python's `datetime.fromtimestamp(t, UTC)`.
        for (seconds, expected) in [
            (0, "1970-01-01T00:00:00Z"),
            (951_782_400, "2000-02-29T00:00:00Z"),
            (1_791_649_864, "2026-10-10T16:31:04Z"),
            (4_102_444_799, "2099-12-31T23:59:59Z"),
        ] {
            assert_eq!(rfc3339(UNIX_EPOCH + Duration::from_secs(seconds)), expected);
        }
    }

    #[test]
    fn repro_quotes_only_arguments_that_need_it() {
        let run = Run::start(vec![
            "suppress".to_owned(),
            "--reason".to_owned(),
            "it's flaky".to_owned(),
            "--format=json".to_owned(),
        ]);
        assert_eq!(
            run.repro(),
            r"cupel suppress --reason 'it'\''s flaky' --format=json"
        );
        assert_eq!(Run::start(Vec::new()).repro(), "cupel");
    }

    #[test]
    fn interrupted_runs_keep_their_results_but_report_partial() {
        let run = Run::start(vec!["describe".to_owned()]);
        let partial = Run::interrupted(run.passed(None));
        assert_eq!(partial.status, Status::Partial);
        assert_eq!(partial.exit_code, ExitCode::Partial);
        let error = run.failed(&CupelError::missing_verb(Vec::new()));
        assert_eq!(Run::interrupted(error.clone()), error);
    }

    #[test]
    fn a_timeout_counts_from_the_start_of_the_invocation() {
        let mut run = Run::start(Vec::new());
        let before = run.cancellation().is_cancelled();
        run.set_timeout(Duration::ZERO);
        assert_eq!((before, run.cancellation().is_cancelled()), (false, true));
    }

    #[test]
    fn planned_envelopes_carry_the_plan_with_status_plan_and_exit_0() {
        let envelope = Run::start(Vec::new()).planned(RunPlan::default(), Vec::new());
        assert_eq!(envelope.status, Status::Plan);
        assert_eq!(envelope.exit_code, ExitCode::Pass);
        assert_eq!(envelope.plan, Some(RunPlan::default()));
    }

    #[test]
    fn failed_envelopes_count_their_finding_and_carry_its_exit_code() {
        let run = Run::start(vec!["check".to_owned()]);
        let envelope = run.failed(&CupelError::missing_verb(Vec::new()));
        assert_eq!(envelope.status, Status::Error);
        assert_eq!(envelope.exit_code, ExitCode::Usage);
        assert_eq!(envelope.summary.findings.fail, 1);
        assert_eq!(envelope.total_findings, 1);
        assert_eq!(envelope.findings[0].repro, "cupel check");
    }
}
