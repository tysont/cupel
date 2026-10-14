// ABOUTME: Helpers shared by the cupel-checks integration tests: a sink that keeps findings and measurements,
// ABOUTME: and per-language results to combine `scope` metrics over. Each test file includes it with `mod support;`.
#![allow(
    dead_code,
    reason = "each test crate uses a different subset of the helpers"
)]

use std::collections::BTreeMap;

use cupel_core::{
    Check, CheckResult, CheckStatus, CostClass, Event, EventSink, Finding, Measurement, Score, Tier,
};

/// Keeps every finding and measurement a check reports.
#[derive(Default)]
pub struct Recorded {
    pub findings: Vec<Finding>,
    pub measurements: Vec<Measurement>,
}

impl EventSink for Recorded {
    fn emit(&mut self, event: Event) {
        if let Event::Finding(finding) = event {
            self.findings.push(*finding);
        }
    }

    fn measure(&mut self, measurement: Measurement) {
        self.measurements.push(measurement);
    }
}

impl Recorded {
    /// Each measurement as (metric, target display, value), in the order reported.
    pub fn measured(&self) -> Vec<(&str, &str, f64)> {
        self.measurements
            .iter()
            .map(|m| (m.metric.as_str(), m.target.display.as_str(), m.value))
            .collect()
    }
}

/// One language's result of `check`: `metrics`, over a score covering `n` targets.
pub fn language(check: &dyn Check, metrics: &[(&str, f64)], n: u64) -> CheckResult {
    CheckResult {
        id: check.manifest().id.clone(),
        status: CheckStatus::Pass,
        tier: Tier::try_from(0).unwrap(),
        cost: CostClass::Quick,
        score: Some(Score {
            value: 0.0,
            unit: String::new(),
            n,
        }),
        metrics: metrics
            .iter()
            .map(|(name, value)| ((*name).to_owned(), *value))
            .collect::<BTreeMap<_, _>>(),
        delta: None,
        findings: 0,
        duration_ms: 0,
        skipped: None,
        oracle: None,
    }
}

/// `name` combined across `results` the way the scheduler does: over the results reporting it.
pub fn combined(check: &dyn Check, name: &str, results: &[CheckResult]) -> Option<f64> {
    let reported: Vec<&CheckResult> = results
        .iter()
        .filter(|result| result.metrics.contains_key(name))
        .collect();
    check.combine_metric(name, &reported)
}
