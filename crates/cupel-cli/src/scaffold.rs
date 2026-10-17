// ABOUTME: `check --scaffold` (ADR-096 §4): a property stub for each `property.missing` finding, rendered by its
// ABOUTME: language; written with `--write`, else previewed as the envelope's `plan.would_write` diffs.
use std::collections::BTreeMap;
use std::path::Path;

use cupel_core::{ActionKind, Envelope, Finding, NextAction, RunPlan, Writer};
use cupel_lang::{LanguageRegistry, Stub, Subject, merged_stubs, property_stub};
use serde_json::Value;

use crate::error::CupelError;

/// The finding kind whose functions get stubs.
const MISSING: &str = "property.missing";

/// Writes (or, without `--write`, previews) the stubs of `envelope`'s unsuppressed
/// `property.missing` findings through `writer`, and says what to do next; `repro` is the
/// command that ran.
pub fn apply(
    envelope: &mut Envelope,
    writer: &Writer,
    languages: &LanguageRegistry,
    repro: &str,
    write: bool,
) -> Result<(), CupelError> {
    let root = writer.root().to_path_buf();
    let read = |path: &str| std::fs::read_to_string(root.join(path)).ok();
    let by_file = stubs_by_file(&envelope.findings, languages, &read);
    let preview = Writer::new(writer.root(), true)?.recording();
    let target = if write { writer } else { &preview };
    let mut files = Vec::new();
    for (file, stubs) in &by_file {
        let existing = read(file);
        let stubs: Vec<&Stub> = stubs.iter().collect();
        if let Some(text) = merged_stubs(existing.as_deref(), &stubs) {
            target.write(Path::new(file), text.as_bytes())?;
            files.push(file.clone());
        }
    }
    if files.is_empty() {
        return Ok(());
    }
    if write {
        envelope
            .next_actions
            .extend(files.iter().map(|file| NextAction {
                priority: 1,
                finding: None,
                kind: ActionKind::AddProperty,
                summary: format!(
                    "State each scaffolded property in `{file}`, then remove its skip marker."
                ),
                command: None,
            }));
    } else {
        envelope.plan = Some(RunPlan {
            would_write: preview.planned(),
            ..RunPlan::default()
        });
        envelope.next_actions.push(NextAction {
            priority: 1,
            finding: None,
            kind: ActionKind::AddProperty,
            summary: "Nothing was written; rerun with `--write` to write these property stubs."
                .to_owned(),
            command: Some(format!("{repro} --write")),
        });
    }
    Ok(())
}

/// The stubs of the unsuppressed `property.missing` findings among `findings`, by the file each
/// goes in.
fn stubs_by_file(
    findings: &[Finding],
    languages: &LanguageRegistry,
    read: &dyn Fn(&str) -> Option<String>,
) -> BTreeMap<String, Vec<Stub>> {
    let mut by_file: BTreeMap<String, Vec<Stub>> = BTreeMap::new();
    for finding in findings
        .iter()
        .filter(|finding| finding.kind == MISSING && !finding.suppressed)
    {
        if let Some(stub) = stub(finding, languages, read) {
            by_file.entry(stub.target.clone()).or_default().push(stub);
        }
    }
    by_file
}

/// The stub of the function `finding` is about, by the language of its file.
fn stub(
    finding: &Finding,
    languages: &LanguageRegistry,
    read: &dyn Fn(&str) -> Option<String>,
) -> Option<Stub> {
    let source = &finding.location.as_ref()?.file;
    let language = languages.for_path(source)?;
    let evidence = &finding.evidence.0;
    let text = |key: &str| evidence.get(key).and_then(Value::as_str);
    let subject = Subject {
        source,
        name: text("name")?,
        namespace: text("namespace").unwrap_or_default(),
        parameters: evidence
            .get("parameters")
            .and_then(Value::as_u64)
            .and_then(|count| u32::try_from(count).ok())
            .unwrap_or_default(),
        signature: text("signature"),
    };
    let nearest = finding
        .action
        .hints
        .get("nearest_test_file")
        .and_then(Value::as_str)
        .filter(|test| read(test).is_some());
    property_stub(language.as_ref(), &subject, nearest, read)
}
