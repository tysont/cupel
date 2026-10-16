// ABOUTME: Reads what `<plugin> run` printed (ADR-089 §4–6): every line an event, findings and metrics the manifest
// ABOUTME: declares, exactly one result or fact of the declared kind and version, and the exit status weighed last.
use cupel_core::plugin::{PluginEvent, PluginResult, ProducerManifest};
use cupel_core::{Captured, CheckStatus, Event, Fact, Finding, Manifest, MetricPer, Result};
use serde_json::Value;

use crate::command::{PluginCommand, ended};
use crate::evidence;

/// A check plugin's accepted run: its result and the events to emit, in order.
pub(crate) struct CheckOutput {
    pub result: PluginResult,
    pub events: Vec<Event>,
}

/// What a finished call printed, accepted by `parse`, weighed with how it exited: a clean exit
/// needs valid output; a failed one is an error unless its output is valid, which is kept with
/// a note (`what` names the event that makes output valid, such as `result`).
pub(crate) fn accepted<T>(
    command: &PluginCommand,
    captured: &Captured,
    what: &str,
    parse: impl FnOnce(&str) -> std::result::Result<T, String>,
) -> Result<(T, Option<String>)> {
    let parsed = parse(&captured.stdout);
    if captured.exit_code == Some(0) {
        return parsed
            .map(|value| (value, None))
            .map_err(|problem| command.error(problem));
    }
    match parsed {
        Ok(value) => {
            let note = format!(
                "plugin `{}` {}",
                command.name,
                ended(captured, &format!(" after its {what}"))
            );
            Ok((value, Some(note)))
        }
        Err(_) => Err(command.error(ended(captured, &format!(" without a {what}")))),
    }
}

/// Every non-blank line of `stdout` as an event.
fn events(stdout: &str) -> std::result::Result<Vec<PluginEvent>, String> {
    stdout
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str(line)
                .map_err(|error| format!("line {} is not a protocol event: {error}", index + 1))
        })
        .collect()
}

/// A check plugin's output: findings and progress of its own check, then exactly one result.
pub(crate) fn check_output(
    manifest: &Manifest,
    stdout: &str,
) -> std::result::Result<CheckOutput, String> {
    let mut result = None;
    let mut emitted = Vec::new();
    for event in events(stdout)? {
        emitted.extend(take(manifest, event, &mut result)?);
    }
    let result = result.ok_or_else(|| "no result".to_owned())?;
    check_result(manifest, &result)?;
    Ok(CheckOutput {
        result,
        events: emitted,
    })
}

/// Takes one event of `manifest`'s check: a result into `result`, a finding or progress as the
/// event the run emits.
fn take(
    manifest: &Manifest,
    event: PluginEvent,
    result: &mut Option<PluginResult>,
) -> std::result::Result<Option<Event>, String> {
    match event {
        PluginEvent::Result(reported) => match result.replace(reported) {
            Some(_) => Err("more than one result".to_owned()),
            None => Ok(None),
        },
        PluginEvent::Finding(finding) => {
            check_finding(manifest, &finding)?;
            Ok(Some(Event::Finding(finding)))
        }
        PluginEvent::Progress { id, .. } if id != manifest.id => {
            Err(format!("progress of check `{id}`, not `{}`", manifest.id))
        }
        PluginEvent::Progress {
            id,
            done,
            total,
            eta_ms,
        } => Ok(Some(Event::Progress {
            id,
            done,
            total,
            eta_ms,
        })),
        PluginEvent::Fact(_) => Err("a check plugin printed a fact".to_owned()),
    }
}

/// A finding must be about `manifest`'s check, of a declared kind, with evidence its schema
/// accepts (ADR-089 §5).
fn check_finding(manifest: &Manifest, finding: &Finding) -> std::result::Result<(), String> {
    if finding.check != manifest.id {
        return Err(format!(
            "a finding of check `{}`, not `{}`",
            finding.check, manifest.id
        ));
    }
    let spec = manifest
        .findings
        .iter()
        .find(|spec| spec.kind == finding.kind)
        .ok_or_else(|| {
            format!(
                "finding kind `{}` is not declared in the manifest",
                finding.kind
            )
        })?;
    evidence::validate(&spec.evidence, &Value::Object(finding.evidence.0.clone()))
        .map_err(|problem| format!("`{}` evidence: {problem}", finding.kind))
}

/// A result's status must be one a run can end with, and its metrics declared scope metrics.
fn check_result(manifest: &Manifest, result: &PluginResult) -> std::result::Result<(), String> {
    if result.status == CheckStatus::Skipped {
        return Err("a result's status must be pass, fail, partial, or error".to_owned());
    }
    let declared = |name: &String| {
        manifest
            .metrics
            .iter()
            .any(|spec| spec.per == MetricPer::Scope && spec.name == *name)
    };
    match result.metrics.keys().find(|name| !declared(name)) {
        Some(name) => Err(format!("metric `{name}` is not a declared scope metric")),
        None => Ok(()),
    }
}

/// A producer plugin's output: exactly one fact of the provided kind and version, whose payload
/// is a valid `F`.
pub(crate) fn fact_output<F: Fact>(
    manifest: &ProducerManifest,
    stdout: &str,
) -> std::result::Result<F, String> {
    let mut fact = None;
    for event in events(stdout)? {
        let PluginEvent::Fact(reported) = event else {
            return Err(format!(
                "a producer plugin printed a `{}` event",
                event_name(&event)
            ));
        };
        if fact.replace(reported).is_some() {
            return Err("more than one fact".to_owned());
        }
    }
    let fact = fact.ok_or_else(|| "no fact".to_owned())?;
    let provides = manifest.provides;
    if fact.kind != provides.kind {
        return Err(format!("a `{}` fact, not `{}`", fact.kind, provides.kind));
    }
    if fact.version != provides.version {
        return Err(format!(
            "a `{}` fact of version {}, not {}",
            fact.kind, fact.version, provides.version
        ));
    }
    serde_json::from_value(fact.payload).map_err(|error| {
        format!(
            "the fact does not match the `{}` schema: {error}",
            fact.kind
        )
    })
}

fn event_name(event: &PluginEvent) -> &'static str {
    match event {
        PluginEvent::Finding(_) => "finding",
        PluginEvent::Progress { .. } => "progress",
        PluginEvent::Result(_) => "result",
        PluginEvent::Fact(_) => "fact",
    }
}
