// ABOUTME: Redaction of the envelope fields that vary between identical runs (ADR-003 §2, and an oracle's verification),
// ABOUTME: in an envelope or where a SARIF log renders them, each present value becoming a placeholder.
use std::path::Path;

use cupel_core::Envelope;
use serde_json::Value;

const VERSION: &str = "<version>";
const RUN_ID: &str = "<run-id>";
const TIMESTAMP: &str = "<timestamp>";
const DURATION: &str = "<duration-ms>";
const SEED: &str = "<seed>";
const SHA: &str = "<sha>";
const BRANCH: &str = "<branch>";
const DIRTY: &str = "<dirty>";
const ROOT: &str = "<root>";

/// Volatile fields of the envelope's `run`, as JSON pointers relative to it, and their
/// placeholders.
const RUN_FIXED: [(&str, &str); 8] = [
    ("/id", RUN_ID),
    ("/baseline/id", RUN_ID),
    ("/started", TIMESTAMP),
    ("/seed", SEED),
    ("/git/sha", SHA),
    ("/git/base", SHA),
    ("/git/branch", BRANCH),
    ("/git/dirty", DIRTY),
];

/// Volatile fields a SARIF log adds where it renders them, and their placeholders.
const SARIF_FIXED: [(&str, &str); 5] = [
    ("/runs/0/tool/driver/version", VERSION),
    ("/runs/0/tool/driver/semanticVersion", VERSION),
    ("/runs/0/invocations/0/startTimeUtc", TIMESTAMP),
    ("/runs/0/versionControlProvenance/0/revisionId", SHA),
    ("/runs/0/versionControlProvenance/0/branch", BRANCH),
];

/// The envelope as JSON with every volatile value replaced by its placeholder.
///
/// `root` is the directory the run treated as the repository root; every occurrence of it
/// inside a string becomes `<root>`.
///
/// # Panics
///
/// Only if the envelope cannot be serialized, which its types rule out.
pub fn redact(envelope: &Envelope, root: &Path) -> Value {
    let mut value = serde_json::to_value(envelope).expect("an envelope always serializes");
    if let Some(run) = value.get_mut("run") {
        mask_run(run);
    }
    mask_outside_run(&mut value);
    mask_durations(&mut value);
    replace_root(&mut value, &root_spellings(root));
    value
}

/// A `--format sarif` log with every volatile value replaced by its placeholder: the envelope's
/// `run` (the invocation's properties) and the rest of it (the run's properties) as [`redact`]
/// masks them, the run ID inside the automation ID, and the version, start time, and
/// revision where SARIF shows them; `root` as in [`redact`].
pub fn redact_sarif(log: &Value, root: &Path) -> Value {
    let mut value = log.clone();
    let run_id = value
        .pointer("/runs/0/invocations/0/properties/id")
        .and_then(Value::as_str)
        .map(str::to_owned);
    if let (Some(run_id), Some(Value::String(id))) =
        (run_id, value.pointer_mut("/runs/0/automationDetails/id"))
    {
        *id = id.replace(&run_id, RUN_ID);
    }
    mask_version_repeats(
        &mut value,
        "/runs/0/tool/driver/version",
        "/runs/0/results",
        "/properties/evidence/actual",
    );
    for (pointer, placeholder) in SARIF_FIXED {
        mask(&mut value, pointer, placeholder);
    }
    if let Some(run) = value.pointer_mut("/runs/0/invocations/0/properties") {
        mask_run(run);
    }
    if let Some(rest) = value.pointer_mut("/runs/0/properties") {
        mask_outside_run(rest);
    }
    mask_durations(&mut value);
    replace_root(&mut value, &root_spellings(root));
    value
}

/// Masks the volatile fields of an envelope's `run`, the toolchain versions included.
fn mask_run(run: &mut Value) {
    for (pointer, placeholder) in RUN_FIXED {
        mask(run, pointer, placeholder);
    }
    if let Some(Value::Object(tools)) = run.get_mut("toolchain") {
        for version in tools.values_mut() {
            *version = VERSION.into();
        }
    }
}

/// Masks the volatile fields of an envelope outside its `run`: the binary's version, where a
/// finding's evidence repeats it as `actual` too, the CLI version `describe` reports, and each
/// oracle's verification.
fn mask_outside_run(value: &mut Value) {
    mask_version_repeats(value, "/cupel/version", "/findings", "/evidence/actual");
    mask(value, "/cupel/version", VERSION);
    match value
        .pointer("/description/subject")
        .and_then(Value::as_str)
    {
        Some("cli") => mask(value, "/description/version", VERSION),
        Some("all") => mask(value, "/description/cli/version", VERSION),
        _ => {}
    }
    if let Some(Value::Array(oracles)) = value.get_mut("oracles") {
        for oracle in oracles {
            mask(oracle, "/verified_at", TIMESTAMP);
            mask(oracle, "/verified_run", RUN_ID);
        }
    }
}

/// Masks `repeat` in each item of the array at `items` where it equals the binary's version, the
/// value at `version`: a finding's evidence that repeats it, as `doctor.version-*` findings do.
fn mask_version_repeats(value: &mut Value, version: &str, items: &str, repeat: &str) {
    let Some(version) = value.pointer(version).cloned() else {
        return;
    };
    if let Some(Value::Array(items)) = value.pointer_mut(items) {
        for item in items {
            if item.pointer(repeat) == Some(&version) {
                mask(item, repeat, VERSION);
            }
        }
    }
}

/// Replaces the value at `pointer` unless it is absent or null.
fn mask(value: &mut Value, pointer: &str, placeholder: &str) {
    if let Some(slot) = value.pointer_mut(pointer).filter(|slot| !slot.is_null()) {
        *slot = placeholder.into();
    }
}

/// Masks every non-null `duration_ms`, at any depth, and every metric named `*_ms` (a check's
/// report of time spent, such as `tests.mutation`'s phases, ADR-150 §3).
fn mask_durations(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                if key == "duration_ms" && !child.is_null() {
                    *child = DURATION.into();
                } else if key == "metrics"
                    && let Value::Object(metrics) = child
                {
                    for (name, metric) in metrics.iter_mut() {
                        if name.ends_with("_ms") {
                            *metric = DURATION.into();
                        }
                    }
                } else {
                    mask_durations(child);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(mask_durations),
        _ => {}
    }
}

/// The ways `root` can be spelled in output, longest first so a longer spelling is replaced
/// before a shorter one that is its prefix.
fn root_spellings(root: &Path) -> Vec<String> {
    let mut spellings: Vec<String> = [Some(root.to_path_buf()), root.canonicalize().ok()]
        .into_iter()
        .flatten()
        .filter_map(|path| path.to_str().map(str::to_owned))
        .filter(|spelling| Path::new(spelling).parent().is_some())
        .collect();
    spellings.sort_by_key(|spelling| std::cmp::Reverse(spelling.len()));
    spellings.dedup();
    spellings
}

/// Replaces every spelling of the root inside every string value.
fn replace_root(value: &mut Value, spellings: &[String]) {
    match value {
        Value::String(text) => {
            for spelling in spellings {
                if text.contains(spelling.as_str()) {
                    *text = text.replace(spelling.as_str(), ROOT);
                }
            }
        }
        Value::Object(map) => map
            .values_mut()
            .for_each(|child| replace_root(child, spellings)),
        Value::Array(items) => items
            .iter_mut()
            .for_each(|child| replace_root(child, spellings)),
        _ => {}
    }
}
