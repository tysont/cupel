// ABOUTME: Builds a plugin's `RunRequest` from what a check or producer sees (ADR-089 §2), and writes the facts
// ABOUTME: it declared as JSON files in its scratch directory (§3).
use std::collections::BTreeMap;
use std::path::Path;

use cupel_core::plugin::{PROTOCOL_VERSION, RequestScope, RunRequest, Toolchain};
use cupel_core::{Cancellation, FactKind, Result, Scope, Writer};
use serde_json::{Map, Value};

/// What differs between a check's request and a producer's.
pub(crate) struct Inputs<'a> {
    /// The repository root.
    pub root: &'a Path,
    pub scope: &'a Scope,
    pub config: &'a Map<String, Value>,
    pub cancel: &'a Cancellation,
    pub scratch: &'a Path,
    /// The run's depth and seed, which only producers see.
    pub run: Option<(cupel_core::CostClass, u64)>,
}

/// The request for `inputs`, naming `facts` (each kind's file).
pub(crate) fn request(inputs: &Inputs<'_>, facts: BTreeMap<FactKind, String>) -> RunRequest {
    RunRequest {
        protocol: PROTOCOL_VERSION,
        root: inputs.root.display().to_string(),
        scope: RequestScope {
            files: inputs.scope.files.clone(),
            symbols: inputs
                .scope
                .symbols
                .as_ref()
                .map(|keys| keys.iter().cloned().collect()),
        },
        depth: inputs.run.map(|(depth, _)| depth),
        budget_ms: inputs
            .cancel
            .remaining()
            .map(|left| u64::try_from(left.as_millis()).unwrap_or(u64::MAX)),
        seed: inputs.run.map(|(_, seed)| seed),
        config: inputs.config.clone(),
        facts,
        scratch: inputs.scratch.display().to_string(),
        toolchain: Toolchain {
            cupel: env!("CARGO_PKG_VERSION").to_owned(),
        },
        write: false,
    }
}

/// Creates `scratch` and writes each of `facts` there as `<kind>.json`, returning each file.
pub(crate) fn write_facts(
    writer: &Writer,
    scratch: &Path,
    facts: impl Iterator<Item = (FactKind, Value)>,
) -> Result<BTreeMap<FactKind, String>> {
    writer.create_dir(scratch)?;
    facts
        .map(|(kind, json)| {
            let text = serde_json::to_vec(&json).expect("a JSON value serializes");
            let file = writer.write(&scratch.join(format!("{kind}.json")), &text)?;
            Ok((kind, file.display().to_string()))
        })
        .collect()
}
