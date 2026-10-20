// ABOUTME: The text renderer (design §7.3): what a person reads from `--format text`.
// ABOUTME: Every value in the envelope is shown except evidence bodies, which are summarized (ADR-019).
mod description;

use std::fmt::Write as _;

use cupel_core::config::{OracleConfig, OracleKind};
use cupel_core::{
    CheckResult, Description, Envelope, Excerpt, Explanation, Finding, FindingId, GateOutcome,
    GateResult, GitInfo, ImpactBasis, LangId, Location, Metric, NextAction, OracleReport,
    PlannedCheck, Rankings, RecommendedTarget, RuleOutcome, RunInfo, RunPlan, ScopeInfo, SetDelta,
    Severity, Skipped, Status, Summary, Symbol, Target, TestsReport,
};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::cli::verb_name;

/// The width of the label column in the run section.
const LABEL: usize = 10;
/// Evidence strings longer than this are shortened in the one-line summary.
const EVIDENCE_STRING: usize = 40;

/// The envelope as text; the agent guide alone, so `--format text` output can be pasted into an
/// agent's instructions.
pub fn render(envelope: &Envelope, color: bool) -> String {
    let paint = Paint { color };
    if let Some(description @ Description::Agent(_)) = &envelope.description {
        return description::render(description, &paint);
    }
    let blocks = [
        status_line(envelope, &paint),
        envelope
            .description
            .as_ref()
            .map(|d| description::render(d, &paint))
            .unwrap_or_default(),
        envelope
            .plan
            .as_ref()
            .map(|plan| plan_block(plan, &paint))
            .unwrap_or_default(),
        envelope
            .explanation
            .as_ref()
            .map(|explanation| explanation_block(explanation, &envelope.findings, &paint))
            .unwrap_or_default(),
        envelope
            .summary
            .gate
            .as_ref()
            .map(|gate| gate_block(gate, &paint))
            .unwrap_or_default(),
        summary_line(&envelope.summary, envelope.total_findings, &paint),
        checks_block(&envelope.checks, &paint),
        envelope
            .rankings
            .as_ref()
            .map(|rankings| rankings_block(rankings, &paint))
            .unwrap_or_default(),
        envelope
            .tests
            .as_ref()
            .map(|tests| tests_block(tests, &paint))
            .unwrap_or_default(),
        envelope
            .oracles
            .as_ref()
            .map(|oracles| oracles_block(oracles, &paint))
            .unwrap_or_default(),
        findings_block(envelope, &paint),
        run_block(envelope, &paint),
        next_actions_block(&envelope.next_actions, &paint),
    ];
    blocks
        .into_iter()
        .filter(|block| !block.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// ANSI styling, or none (ADR-019 §4).
pub struct Paint {
    color: bool,
}

impl Paint {
    fn sgr(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("\u{1b}[{code}m{text}\u{1b}[0m")
        } else {
            text.to_owned()
        }
    }

    pub fn bold(&self, text: &str) -> String {
        self.sgr("1", text)
    }

    pub fn dim(&self, text: &str) -> String {
        self.sgr("2", text)
    }

    fn status(&self, status: Status) -> String {
        let code = match status {
            Status::Pass => "1;32",
            Status::Fail | Status::Error => "1;31",
            Status::Partial => "1;33",
            Status::Plan => "1;36",
        };
        self.sgr(code, &name(status))
    }

    fn outcome(&self, outcome: GateOutcome) -> String {
        let code = match outcome {
            GateOutcome::Pass => "32",
            GateOutcome::Fail => "31",
        };
        self.sgr(code, &name(outcome))
    }

    fn rule(&self, outcome: RuleOutcome) -> String {
        let code = match outcome {
            RuleOutcome::Pass => "32",
            RuleOutcome::Fail => "31",
            RuleOutcome::Skipped | RuleOutcome::Unsupported => "33",
        };
        self.sgr(code, &name(outcome))
    }

    fn severity(&self, severity: Severity, text: &str) -> String {
        let code = match severity {
            Severity::Fail => "1;31",
            Severity::Warn => "1;33",
            Severity::Info => "1;34",
        };
        self.sgr(code, text)
    }
}

/// A unit enum's serialized name, such as `write_test` or `changed`.
pub fn name<T: Serialize>(value: T) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(name)) => name,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}

/// A JSON value for people: strings bare, everything else as compact JSON.
pub fn scalar(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// Whether a value carries information: not null, not an empty list, and not a map whose
/// values are all absent.
pub fn present(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => map.values().any(present),
        _ => true,
    }
}

/// `text` with every line after the first indented by `indent` spaces.
pub fn continued(text: &str, indent: usize) -> String {
    text.lines()
        .collect::<Vec<_>>()
        .join(&format!("\n{}", " ".repeat(indent)))
}

/// `label` padded to the label column, then `value`, ending with a newline.
fn labeled(out: &mut String, label: &str, value: &str, paint: &Paint) {
    let padded = format!("{label:<LABEL$}");
    let _ = writeln!(out, "{}{}", paint.bold(&padded), continued(value, LABEL));
}

fn status_line(envelope: &Envelope, paint: &Paint) -> String {
    let verb = envelope
        .run
        .verb
        .map(|verb| format!(" {}", verb_name(verb)))
        .unwrap_or_default();
    format!(
        "cupel{verb}: {} (exit {})\n",
        paint.status(envelope.status),
        envelope.exit_code.code()
    )
}

fn gate_block(gate: &GateResult, paint: &Paint) -> String {
    let mut out = String::new();
    labeled(
        &mut out,
        "gate",
        &format!("{} ({})", paint.outcome(gate.result), gate.evaluated),
        paint,
    );
    for rule in &gate.rules {
        let actual = rule
            .actual
            .map(|actual| format!("  actual {actual}"))
            .unwrap_or_default();
        // A rule against `base` does not show the value it was compared with; the others do.
        let bound = rule
            .bound
            .filter(|_| rule.rule.contains(" base"))
            .map(|bound| format!("  bound {bound}"))
            .unwrap_or_default();
        let reason = rule
            .reason
            .as_ref()
            .map(|reason| format!("  ({reason})"))
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "  {}  {}{actual}{bound}{reason}",
            paint.rule(rule.result),
            rule.rule
        );
    }
    out
}

fn summary_line(summary: &Summary, total: u64, paint: &Paint) -> String {
    let counts = &summary.findings;
    let mut out = String::new();
    labeled(
        &mut out,
        "summary",
        &format!(
            "{total} {} · fail {} · warn {} · info {} · new {} · resolved {} · suppressed {}",
            plural(total, "finding", "findings"),
            counts.fail,
            counts.warn,
            counts.info,
            summary.new,
            summary.resolved,
            summary.suppressed
        ),
        paint,
    );
    out
}

fn plural<'a>(count: u64, one: &'a str, many: &'a str) -> &'a str {
    if count == 1 { one } else { many }
}

fn checks_block(checks: &[CheckResult], paint: &Paint) -> String {
    if checks.is_empty() {
        return String::new();
    }
    let width = checks
        .iter()
        .map(|c| c.id.as_str().len())
        .max()
        .unwrap_or(0);
    let mut out = format!("{}\n", paint.bold("checks"));
    for check in checks {
        let mut facts = vec![format!("tier {}", check.tier.value()), name(check.cost)];
        if let Some(score) = &check.score {
            facts.push(format!(
                "score {} {} (n {})",
                score.value, score.unit, score.n
            ));
        }
        if !check.metrics.is_empty() {
            let metrics: Vec<String> = check
                .metrics
                .iter()
                .map(|(metric, value)| format!("{metric} {value}"))
                .collect();
            facts.push(format!("metrics {}", metrics.join(", ")));
        }
        if let Some(delta) = &check.delta {
            facts.push(format!("delta {:+} vs {}", delta.value, delta.vs));
        }
        facts.push(format!(
            "{} {}",
            check.findings,
            plural(check.findings, "finding", "findings")
        ));
        facts.push(format!("{} ms", check.duration_ms));
        if let Some(oracle) = &check.oracle {
            facts.push(format!("oracle {oracle}"));
        }
        if let Some(skipped) = &check.skipped {
            facts.push(format!("skipped: {}", skip_reason(skipped)));
        }
        facts.extend(check.notes.iter().map(|note| format!("note: {note}")));
        let _ = writeln!(
            out,
            "  {:<width$}  {:<8} {}",
            check.id.as_str(),
            name(check.status),
            facts.join(" · ")
        );
        for set in check.delta.iter().flat_map(|delta| &delta.sets) {
            let _ = writeln!(out, "    {}", matched_set(set));
        }
    }
    out
}

/// One matched set of a check's delta (ADR-131): its symbols, similarity, the findings carried
/// across it, and each per-symbol metric over each side.
fn matched_set(set: &SetDelta) -> String {
    let symbols = |side: &[Symbol]| -> String {
        let names: Vec<&str> = side.iter().map(|symbol| symbol.display.as_str()).collect();
        names.join(", ")
    };
    let mut facts = vec![
        format!("matched {} → {}", symbols(&set.base), symbols(&set.head)),
        format!("similarity {}", set.similarity),
    ];
    if !set.findings.base.is_empty() {
        let ids = |side: &[FindingId]| -> String {
            let ids: Vec<&str> = side.iter().map(FindingId::as_str).collect();
            ids.join(", ")
        };
        facts.push(format!(
            "carried {} → {}",
            ids(&set.findings.base),
            ids(&set.findings.head)
        ));
    }
    facts.extend(
        set.metrics
            .iter()
            .map(|(metric, value)| format!("{metric} {} → {}", value.base, value.head)),
    );
    facts.join(" · ")
}

/// Each dimension as `check.metric (per, measured)` with its ranked targets beneath (ADR-062),
/// then the protect-first targets with their protection, values, and findings (ADR-132).
fn rankings_block(rankings: &Rankings, paint: &Paint) -> String {
    let mut out = format!(
        "{}  top {} per dimension\n",
        paint.bold("rankings"),
        rankings.top
    );
    for dimension in &rankings.dimensions {
        let _ = writeln!(
            out,
            "  {}.{}  {} · {} measured · {}",
            dimension.check,
            dimension.metric,
            name(dimension.per),
            dimension.measured,
            dimension.summary
        );
        for entry in &dimension.entries {
            let _ = writeln!(
                out,
                "    {:>2}. {}  {}  {}",
                entry.rank,
                entry.value,
                entry.target.display,
                paint.dim(&place(&entry.lang, &entry.target))
            );
        }
    }
    let _ = writeln!(
        out,
        "  targets  {} recommended, protect first",
        rankings.recommended
    );
    for target in &rankings.targets {
        target_lines(&mut out, target, paint);
    }
    out
}

/// A protect-first target: what to do, its protection, and where it is; beneath, its value in
/// each dimension with its rank there, and the findings that recommend it.
fn target_lines(out: &mut String, target: &RecommendedTarget, paint: &Paint) {
    let protection = &target.protection;
    let shown = |value: Option<String>| value.unwrap_or_else(|| "-".to_owned());
    let _ = writeln!(
        out,
        "    {:>2}. {}  {}  mutation {} · coverage {} · tests {}  {}",
        target.rank,
        name(target.recommend),
        target.target.display,
        shown(protection.mutation_score.map(|v| v.to_string())),
        shown(protection.coverage.map(|v| v.to_string())),
        shown(protection.tests.map(|v| v.to_string())),
        paint.dim(&place(&target.lang, &target.target))
    );
    let details: Vec<String> = target
        .values
        .iter()
        .map(|v| format!("{}.{} {} (#{})", v.check, v.metric, v.value, v.rank))
        .chain(target.findings.iter().map(ToString::to_string))
        .collect();
    if !details.is_empty() {
        let _ = writeln!(out, "        {}", paint.dim(&details.join(" · ")));
    }
}

/// A ranked target's language, location, and structural hash, as far as it has them.
fn place(lang: &LangId, target: &Target) -> String {
    let mut facts = vec![lang.to_string()];
    if let Some(at) = &target.location {
        facts.push(location(at));
    }
    if let Some(symbol) = &target.symbol {
        facts.push(symbol.to_string());
    }
    facts.join(" · ")
}

/// How much of a language's per-test map came from its producer's cache (ADR-083), such as
/// ` (cached)` or ` (2 measured, 1236 cached)`; empty when unknown.
fn cache_note(basis: &ImpactBasis) -> String {
    match (basis.measured, basis.reused) {
        (Some(0), _) => " (cached)".to_owned(),
        (Some(measured), Some(0) | None) => format!(" ({measured} measured)"),
        (Some(measured), Some(reused)) => format!(" ({measured} measured, {reused} cached)"),
        (None, _) => String::new(),
    }
}

/// How each language chose its tests, the impacted tests with why, and the commands that run
/// them (ADR-077 §6).
fn tests_block(tests: &TestsReport, paint: &Paint) -> String {
    let mut out = format!(
        "{}  {} impacted · per-test map: {}\n",
        paint.bold("tests"),
        tests.impacted.len(),
        if tests.per_test_map { "yes" } else { "no" }
    );
    for basis in &tests.languages {
        let how = match (&basis.producer, &basis.fallback) {
            (_, Some(fallback)) => format!("file-level impact: {fallback}"),
            (Some(producer), None) => format!("per-test map from {producer}{}", cache_note(basis)),
            (None, None) => format!("per-test map{}", cache_note(basis)),
        };
        let _ = writeln!(out, "  {}  {}", basis.lang, paint.dim(&how));
    }
    for test in &tests.impacted {
        let _ = writeln!(out, "  {}  {}", test.id, paint.dim(&test.lang.to_string()));
        let _ = writeln!(out, "    {}", test.reason);
    }
    for command in &tests.commands {
        let scope = if command.narrowed {
            ""
        } else {
            " (whole suite)"
        };
        let _ = writeln!(out, "  run in {}{scope}: {}", command.dir, command.command);
    }
    out
}

/// Each oracle with what it compares against, its state, and its recorded verification
/// (ADR-091 §6).
fn oracles_block(oracles: &[OracleReport], paint: &Paint) -> String {
    let mut out = format!("{}  {}\n", paint.bold("oracles"), oracles.len());
    let width = oracles
        .iter()
        .map(|oracle| oracle.name.len())
        .max()
        .unwrap_or(0);
    for oracle in oracles {
        let mut facts = vec![
            definition(&oracle.definition),
            oracle.state.as_str().to_owned(),
        ];
        if let (Some(at), Some(run)) = (&oracle.verified_at, &oracle.verified_run) {
            facts.push(paint.dim(&format!("recorded {at} by {run}")));
        }
        let _ = writeln!(out, "  {:width$}  {}", oracle.name, facts.join(" · "));
    }
    out
}

/// What an oracle compares against, as one phrase.
fn definition(oracle: &OracleConfig) -> String {
    let given = |value: &Option<String>| value.clone().unwrap_or_default();
    match oracle.kind {
        OracleKind::Ref => format!("ref {}", given(&oracle.reference)),
        OracleKind::Command if oracle.inputs.is_empty() => {
            format!("command `{}`", given(&oracle.cmd))
        }
        OracleKind::Command => format!(
            "command `{}` · inputs {}",
            given(&oracle.cmd),
            oracle.inputs.len()
        ),
        OracleKind::Spec => format!("spec {} ({})", given(&oracle.spec), given(&oracle.lang)),
    }
}

/// A skip's reason followed by whichever of its fact, language, tool, and pointer are present.
fn skip_reason(skipped: &Skipped) -> String {
    let mut why = Vec::new();
    if let Some(fact) = skipped.fact {
        why.push(format!("fact {}", name(fact)));
    }
    if let Some(lang) = &skipped.lang {
        why.push(format!("lang {lang}"));
    }
    if let Some(tool) = &skipped.tool {
        why.push(format!("tool {tool}"));
    }
    if let Some(see) = &skipped.see {
        why.push(format!("see `{see}`"));
    }
    if let Some(est_ms) = skipped.est_ms {
        why.push(format!("est {}", seconds(est_ms)));
    }
    if why.is_empty() {
        name(skipped.reason)
    } else {
        format!("{} ({})", name(skipped.reason), why.join(", "))
    }
}

/// `ms` as seconds with one decimal, such as `30.2 s`.
#[allow(
    clippy::cast_precision_loss,
    reason = "an estimate in milliseconds stays far below 2^52, where f64 is exact"
)]
fn seconds(ms: u64) -> String {
    format!("{:.1} s", ms as f64 / 1000.0)
}

/// A dry run's plan (ADR-020 §6): the estimate, then each non-empty list, with diffs indented.
fn plan_block(plan: &RunPlan, paint: &Paint) -> String {
    let mut out = String::new();
    labeled(
        &mut out,
        "plan",
        &format!("est {} ms", plan.est_total_ms),
        paint,
    );
    let run = plan.would_run.iter().map(planned_check).collect();
    rows(&mut out, "would run", run, paint);
    let skip = plan
        .would_skip
        .iter()
        .map(|skip| (skip.id.as_str().to_owned(), skip_reason(&skip.skipped)))
        .collect();
    rows(&mut out, "would skip", skip, paint);
    let install = plan
        .would_install
        .iter()
        .map(|install| {
            let command = install
                .command
                .as_ref()
                .map(|command| format!("$ {command}"))
                .unwrap_or_default();
            (install.tool.to_string(), command)
        })
        .collect();
    rows(&mut out, "would install", install, paint);
    if !plan.would_write.is_empty() {
        let _ = writeln!(out, "{}", paint.bold("would write"));
        for write in &plan.would_write {
            let _ = writeln!(out, "  {}", write.path);
            for line in write.diff.iter().flat_map(|diff| diff.lines()) {
                let _ = writeln!(out, "    {}", paint.dim(line));
            }
        }
    }
    out
}

/// A planned check as (ID, its targets, estimate, needs, and note).
fn planned_check(check: &PlannedCheck) -> (String, String) {
    let mut facts = vec![
        format!("{} targets", check.targets),
        format!("est {} ms", check.est_ms),
    ];
    if !check.needs.is_empty() {
        facts.push(format!("needs {}", check.needs.join(", ")));
    }
    let mut value = facts.join(" · ");
    if let Some(note) = &check.note {
        let _ = write!(value, "\n{note}");
    }
    (check.id.as_str().to_owned(), value)
}

/// A heading and its (key, value) rows with the values aligned; nothing for no rows.
fn rows(out: &mut String, heading: &str, rows: Vec<(String, String)>, paint: &Paint) {
    if rows.is_empty() {
        return;
    }
    let width = rows.iter().map(|(key, _)| key.len()).max().unwrap_or(0);
    let _ = writeln!(out, "{}", paint.bold(heading));
    for (key, value) in rows {
        if value.is_empty() {
            let _ = writeln!(out, "  {key}");
        } else {
            let _ = writeln!(out, "  {key:<width$}  {}", continued(&value, width + 4));
        }
    }
}

/// What `explain` adds: where it came from, the source excerpt with the location's lines marked,
/// the full evidence of the explained finding (the one place evidence bodies are printed),
/// test hints, and metrics.
fn explanation_block(explanation: &Explanation, findings: &[Finding], paint: &Paint) -> String {
    let mut out = String::new();
    labeled(
        &mut out,
        "explain",
        &format!(
            "{} {} from {}",
            name(explanation.subject),
            explanation.target,
            explanation.run
        ),
        paint,
    );
    if let Some(excerpt) = &explanation.excerpt {
        excerpt_lines(&mut out, excerpt, paint);
    }
    let explained = findings
        .iter()
        .find(|finding| finding.id.as_str() == explanation.target);
    if let Some(finding) = explained.filter(|finding| !finding.evidence.0.is_empty()) {
        let evidence = serde_json::to_string_pretty(&finding.evidence).unwrap_or_default();
        let _ = writeln!(out, "  {}", paint.bold("evidence"));
        for line in evidence.lines() {
            let _ = writeln!(out, "    {line}");
        }
    }
    if !explanation.covering_tests.is_empty() {
        let _ = writeln!(
            out,
            "  {}  {}",
            paint.bold("covering tests"),
            explanation.covering_tests.join(", ")
        );
    }
    if let Some(file) = &explanation.nearest_test_file {
        let _ = writeln!(out, "  {}  {file}", paint.bold("nearest test file"));
    }
    metric_lines(&mut out, &explanation.metrics, paint);
    out
}

/// The excerpt's file, then its numbered lines, the location's marked with `>`.
fn excerpt_lines(out: &mut String, excerpt: &Excerpt, paint: &Paint) {
    let width = excerpt
        .lines
        .iter()
        .map(|line| line.line.to_string().len())
        .max()
        .unwrap_or(0);
    let _ = writeln!(out, "  {}", paint.bold(&excerpt.file));
    for line in &excerpt.lines {
        let marker = if line.focus { ">" } else { " " };
        let _ = writeln!(out, "  {marker} {:>width$} | {}", line.line, line.text);
    }
}

/// Each stored metric: its symbol (or `(check)`), name, and value.
fn metric_lines(out: &mut String, metrics: &[Metric], paint: &Paint) {
    if metrics.is_empty() {
        return;
    }
    let _ = writeln!(out, "  {}", paint.bold("metrics"));
    let width = metrics
        .iter()
        .map(|metric| metric.key.len())
        .max()
        .unwrap_or(0);
    for metric in metrics {
        let symbol = metric
            .symbol
            .as_ref()
            .map_or_else(|| "(check)".to_owned(), ToString::to_string);
        let _ = writeln!(
            out,
            "    {symbol:<19}  {:<width$}  {}",
            metric.key, metric.value
        );
    }
}

fn findings_block(envelope: &Envelope, paint: &Paint) -> String {
    let mut out = String::new();
    if envelope.truncated {
        let _ = writeln!(
            out,
            "showing {} of {} findings (truncated); page with --offset N --limit N",
            envelope.findings.len(),
            envelope.total_findings
        );
    }
    for severity in [Severity::Fail, Severity::Warn, Severity::Info] {
        let group: Vec<&Finding> = envelope
            .findings
            .iter()
            .filter(|finding| finding.severity == severity)
            .collect();
        if group.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        severity_group(&mut out, severity, &group, paint);
    }
    out
}

/// The findings of one severity, grouped by check in envelope order.
fn severity_group(out: &mut String, severity: Severity, group: &[&Finding], paint: &Paint) {
    let heading = format!("{} ({})", name(severity), group.len());
    let _ = writeln!(out, "{}", paint.severity(severity, &heading));
    let mut checks: Vec<&str> = Vec::new();
    for finding in group {
        if !checks.contains(&finding.check.as_str()) {
            checks.push(finding.check.as_str());
        }
    }
    for check in checks {
        let _ = writeln!(out, "  {}", paint.bold(check));
        for finding in group.iter().filter(|f| f.check.as_str() == check) {
            finding_lines(out, finding, paint);
        }
    }
}

fn finding_lines(out: &mut String, finding: &Finding, paint: &Paint) {
    const BODY: usize = 6;
    let pad = " ".repeat(BODY);
    let _ = writeln!(out, "    {}", continued(&finding.title, 4));
    let mut facts = vec![
        finding.id.to_string(),
        finding.kind.clone(),
        format!("tier {}", finding.tier.value()),
        format!("confidence {}", finding.confidence),
    ];
    if finding.new_since_baseline {
        facts.push("new".to_owned());
    }
    if finding.suppressed {
        facts.push("suppressed".to_owned());
    }
    let _ = writeln!(out, "{pad}{}", facts.join(" · "));
    let place = [
        finding.location.as_ref().map(location),
        finding
            .symbol
            .as_ref()
            .map(|symbol| format!("{} ({}, {})", symbol.display, symbol.lang, symbol.hash)),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    if !place.is_empty() {
        let _ = writeln!(out, "{pad}at {}", place.join(" in "));
    }
    if let Some(detail) = &finding.detail {
        let _ = writeln!(out, "{pad}{}", continued(detail, BODY));
    }
    if let Some(evidence) = evidence_summary(&finding.evidence.0) {
        let _ = writeln!(
            out,
            "{pad}evidence: {evidence} (full: cupel explain {})",
            finding.id
        );
    }
    let _ = writeln!(
        out,
        "{pad}action {} (priority {}): {}",
        name(finding.action.kind),
        finding.action.priority,
        continued(&finding.action.summary, BODY + 2)
    );
    hints(out, &finding.action.hints, BODY + 2);
    let _ = writeln!(
        out,
        "{pad}{}",
        paint.dim(&format!("repro: {}", finding.repro))
    );
    let _ = writeln!(
        out,
        "{pad}{}",
        paint.dim(&format!("suppress: {}", finding.suppress))
    );
}

/// `file:line[:col][-end_line[:end_col]]`, leaving out an end equal to the start.
fn location(location: &Location) -> String {
    let start = match location.col {
        Some(col) => format!("{}:{col}", location.line),
        None => location.line.to_string(),
    };
    let end = match location.end_col {
        Some(col) => format!("{}:{col}", location.end_line),
        None => location.end_line.to_string(),
    };
    if start == end {
        format!("{}:{start}", location.file)
    } else {
        format!("{}:{start}-{end}", location.file)
    }
}

/// Evidence on one line: each present key with a scalar shown (strings shortened) and a list or
/// map shown by its size.
fn evidence_summary(evidence: &Map<String, Value>) -> Option<String> {
    let parts: Vec<String> = evidence
        .iter()
        .filter(|(_, value)| !value.is_null())
        .map(|(key, value)| match value {
            Value::Array(items) => format!("{key} [{}]", items.len()),
            Value::Object(map) => format!("{key} {{{}}}", map.len()),
            Value::String(text) if text.chars().count() > EVIDENCE_STRING => {
                let short: String = text.chars().take(EVIDENCE_STRING).collect();
                format!("{key} \"{short}…\"")
            }
            Value::String(text) => format!("{key} \"{text}\""),
            other => format!("{key} {other}"),
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// Each present entry of an open map as `key: value`, indented.
fn hints(out: &mut String, map: &Map<String, Value>, indent: usize) {
    for (key, value) in map.iter().filter(|(_, value)| present(value)) {
        let _ = writeln!(out, "{}{key}: {}", " ".repeat(indent), scalar(value));
    }
}

fn run_block(envelope: &Envelope, paint: &Paint) -> String {
    let run: &RunInfo = &envelope.run;
    let mut out = String::new();
    let lines = [
        (
            "run",
            Some(format!(
                "{} · started {} · {} ms · seed {}",
                run.id, run.started, run.duration_ms, run.seed
            )),
        ),
        ("git", run.git.as_ref().map(git_line)),
        ("scope", run.scope.as_ref().map(scope_line)),
        ("depth", effort_line(run)),
        (
            "baseline",
            run.baseline
                .as_ref()
                .map(|baseline| format!("{} ({})", baseline.name, baseline.id)),
        ),
        ("toolchain", toolchain_line(run)),
        ("config", config_line(run)),
        (
            "cupel",
            Some(format!(
                "{} · {}",
                envelope.cupel.version, envelope.cupel.schema
            )),
        ),
    ];
    for (label, line) in lines {
        if let Some(line) = line {
            labeled(&mut out, label, &line, paint);
        }
    }
    out
}

fn git_line(git: &GitInfo) -> String {
    let mut facts = vec![git.sha.clone()];
    if let Some(branch) = &git.branch {
        facts.push(format!("branch {branch}"));
    }
    if let Some(base) = &git.base {
        facts.push(format!("base {base}"));
    }
    facts.push(if git.dirty { "dirty" } else { "clean" }.to_owned());
    facts.join(" · ")
}

fn scope_line(scope: &ScopeInfo) -> String {
    let mut facts = vec![
        name(scope.mode),
        format!("{} files", scope.files),
        format!("{} symbols", scope.symbols),
    ];
    if !scope.roots.is_empty() {
        facts.push(format!("roots {}", scope.roots.join(", ")));
    }
    if !scope.languages.is_empty() {
        let languages: Vec<&str> = scope
            .languages
            .iter()
            .map(cupel_core::LangId::as_str)
            .collect();
        facts.push(format!("languages {}", languages.join(", ")));
    }
    facts.join(" · ")
}

/// The depth and budget, when either is known.
fn effort_line(run: &RunInfo) -> Option<String> {
    let effort = [
        run.depth.map(name),
        run.budget_ms.map(|ms| format!("budget {ms} ms")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    (!effort.is_empty()).then(|| effort.join(" · "))
}

fn toolchain_line(run: &RunInfo) -> Option<String> {
    let tools: Vec<String> = run
        .toolchain
        .0
        .iter()
        .map(|(tool, version)| format!("{tool} {version}"))
        .collect();
    (!tools.is_empty()).then(|| tools.join(" · "))
}

/// The configuration's sources, then each effective setting on its own line.
fn config_line(run: &RunInfo) -> Option<String> {
    if run.config.sources.is_empty() && run.config.effective.is_empty() {
        return None;
    }
    let sources: Vec<String> = run.config.sources.iter().map(|s| name(*s)).collect();
    let mut config = format!("from {}", sources.join(", "));
    for (key, value) in run.config.effective.iter().filter(|(_, v)| present(v)) {
        let _ = write!(config, "\n{key}: {}", scalar(value));
    }
    Some(config)
}

fn next_actions_block(actions: &[NextAction], paint: &Paint) -> String {
    if actions.is_empty() {
        return String::new();
    }
    let mut out = format!("{}\n", paint.bold("next actions"));
    for action in actions {
        let number = format!("{}. ", action.priority);
        let indent = 2 + number.len();
        let finding = action
            .finding
            .as_ref()
            .map(|id| format!(" (finding {id})"))
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "  {number}{}: {}{finding}",
            name(action.kind),
            continued(&action.summary, indent)
        );
        if let Some(command) = &action.command {
            let _ = writeln!(out, "{}$ {command}", " ".repeat(indent));
        }
    }
    out
}
