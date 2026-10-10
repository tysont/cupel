// ABOUTME: One invocation's run record: ID, start time, verb, and seed, and the envelope built from it.
// ABOUTME: Successful verbs and every error path assemble their envelope here, so all share one shape.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use cupel_core::{
    ConfigEcho, CupelInfo, Description, Envelope, ExitCode, Finding, RunId, RunInfo, Severity,
    SeverityCounts, Status, Summary, ToolchainInfo, Verb, schema,
};

use crate::error::CupelError;

pub struct Run {
    id: RunId,
    started: SystemTime,
    clock: Instant,
    pub verb: Option<Verb>,
    /// `--seed`, or 0 until P0.7 generates a random default.
    pub seed: u64,
    /// The command line after `cupel`, for repro strings.
    pub args: Vec<String>,
}

impl Run {
    pub fn start(args: Vec<String>) -> Self {
        Self {
            id: RunId::generate(),
            started: SystemTime::now(),
            clock: Instant::now(),
            verb: None,
            seed: 0,
            args,
        }
    }

    /// The envelope of a verb that ran to completion.
    pub fn passed(&self, description: Option<Description>) -> Envelope {
        self.envelope(Status::Pass, ExitCode::Pass, Vec::new(), description)
    }

    /// The envelope reporting `error` as its one finding.
    pub fn failed(&self, error: &CupelError) -> Envelope {
        let finding = error.finding(&self.repro());
        self.envelope(Status::Error, error.exit_code(), vec![finding], None)
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
        let mut counts = SeverityCounts::default();
        for finding in &findings {
            match finding.severity {
                Severity::Fail => counts.fail += 1,
                Severity::Warn => counts.warn += 1,
                Severity::Info => counts.info += 1,
            }
        }
        let total_findings = findings.len() as u64;
        Envelope {
            cupel: CupelInfo {
                version: env!("CARGO_PKG_VERSION").to_owned(),
                schema: schema::url("run"),
            },
            run: RunInfo {
                id: self.id,
                verb: self.verb,
                started: rfc3339(self.started),
                duration_ms: u64::try_from(self.clock.elapsed().as_millis()).unwrap_or(u64::MAX),
                git: None,
                scope: None,
                depth: None,
                budget_ms: None,
                seed: self.seed,
                toolchain: ToolchainInfo::default(),
                // The effective configuration is echoed once P0.4 loads it.
                config: ConfigEcho::default(),
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
            truncated: false,
            total_findings,
        }
    }
}

/// Quotes an argument for a POSIX shell when it contains anything but safe characters.
fn shell_quote(arg: &str) -> String {
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
    fn failed_envelopes_count_their_finding_and_carry_its_exit_code() {
        let run = Run::start(vec!["check".to_owned()]);
        let envelope = run.failed(&CupelError::pending_verb(&["check".to_owned()], "P2.1"));
        assert_eq!(envelope.status, Status::Error);
        assert_eq!(envelope.exit_code, ExitCode::Usage);
        assert_eq!(envelope.summary.findings.fail, 1);
        assert_eq!(envelope.total_findings, 1);
        assert_eq!(envelope.findings[0].repro, "cupel check");
    }
}
