// ABOUTME: Redaction of the envelope fields that vary between identical runs (ADR-003 §2),
// ABOUTME: replacing each present value with a placeholder string and leaving nulls alone.
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

/// Volatile fields at fixed positions, as JSON pointers, and their placeholders.
const FIXED: [(&str, &str); 9] = [
    ("/cupel/version", VERSION),
    ("/run/id", RUN_ID),
    ("/run/baseline/id", RUN_ID),
    ("/run/started", TIMESTAMP),
    ("/run/seed", SEED),
    ("/run/git/sha", SHA),
    ("/run/git/base", SHA),
    ("/run/git/branch", BRANCH),
    ("/run/git/dirty", DIRTY),
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
    for (pointer, placeholder) in FIXED {
        mask(&mut value, pointer, placeholder);
    }
    match value
        .pointer("/description/subject")
        .and_then(Value::as_str)
    {
        Some("cli") => mask(&mut value, "/description/version", VERSION),
        Some("all") => mask(&mut value, "/description/cli/version", VERSION),
        _ => {}
    }
    if let Some(Value::Object(tools)) = value.pointer_mut("/run/toolchain") {
        for version in tools.values_mut() {
            *version = VERSION.into();
        }
    }
    mask_durations(&mut value);
    replace_root(&mut value, &root_spellings(root));
    value
}

/// Replaces the value at `pointer` unless it is absent or null.
fn mask(value: &mut Value, pointer: &str, placeholder: &str) {
    if let Some(slot) = value.pointer_mut(pointer).filter(|slot| !slot.is_null()) {
        *slot = placeholder.into();
    }
}

/// Masks every non-null `duration_ms`, at any depth.
fn mask_durations(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                if key == "duration_ms" && !child.is_null() {
                    *child = DURATION.into();
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
