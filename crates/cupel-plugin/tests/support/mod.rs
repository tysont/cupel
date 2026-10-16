// ABOUTME: Shared support for the plugin host's tests: purpose-written shell plugins in a temporary directory,
// ABOUTME: valid manifests to start from, and discovery over a `[plugins]` table naming them.
#![allow(
    dead_code,
    reason = "each test binary uses a different part of the support"
)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use cupel_core::config::PluginEntry;
use cupel_core::{Cancellation, CheckId, Lang, LangFamily, LangId};
use cupel_plugin::Discovery;
use serde_json::{Value, json};

/// Writes an executable `sh` script `name` in `dir` running `body`.
pub fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// A script body answering `describe` with `manifest`, `plan` with `plan`, and `run` by saving
/// its request to `request.json` beside the script and then running `run`.
pub fn plugin(dir: &Path, manifest: &Value, plan: &str, run: &str) -> String {
    format!(
        "case \"$1\" in\n\
         describe) cat <<'JSON'\n{manifest}\nJSON\n;;\n\
         plan) {plan} ;;\n\
         run) cat > '{saved}'\n{run}\n;;\n\
         *) exit 64 ;;\n\
         esac",
        saved = dir.join("request.json").display(),
    )
}

/// `lines`, each printed on its own line by a shell `cat`.
pub fn prints(lines: &[Value]) -> String {
    let text: Vec<String> = lines.iter().map(Value::to_string).collect();
    format!("cat <<'JSON'\n{}\nJSON", text.join("\n"))
}

/// The check `ref.sample`: needs a per-test map, reports `ref.sample-file` about a file, and
/// measures `untested` per scope.
pub fn check_manifest() -> Value {
    json!({
        "kind": "check",
        "id": "ref.sample",
        "version": 1,
        "tier": 2,
        "cost": "quick",
        "needs": ["per-test-map"],
        "wants": ["symbols"],
        "findings": [{
            "kind": "ref.sample-file",
            "action": "write_test",
            "summary": "A file no test executes.",
            "evidence": {
                "type": "object",
                "properties": {"file": {"type": "string"}},
                "required": ["file"],
                "additionalProperties": false
            }
        }],
        "metrics": [{"name": "untested", "summary": "Files no test executes.", "per": "scope"}],
        "stability": "experimental",
        "summary": "Files no test executes."
    })
}

/// The producer `plugin.sample`: a per-test map for TypeScript.
pub fn producer_manifest() -> Value {
    json!({
        "kind": "producer",
        "id": "plugin.sample",
        "version": 1,
        "provides": {"kind": "per-test-map", "version": 2},
        "languages": ["typescript"],
        "needs": [],
        "wants": [],
        "cost": "quick",
        "stability": "experimental",
        "summary": "A per-test map."
    })
}

/// `[plugins]` naming each `(name, program)`.
pub fn entries(plugins: &[(&str, &Path)]) -> BTreeMap<String, PluginEntry> {
    plugins
        .iter()
        .map(|(name, program)| {
            (
                (*name).to_owned(),
                PluginEntry::Executable(program.display().to_string()),
            )
        })
        .collect()
}

/// The languages discovery accepts in a producer's manifest.
pub fn languages() -> Vec<LangId> {
    ["python", "typescript"].map(LangId::from).to_vec()
}

/// Discovery of `entries` in `root`, without `PATH`, with `static.complexity` registered.
pub fn discovery<'a>(
    root: &'a Path,
    entries: &'a BTreeMap<String, PluginEntry>,
    languages: &'a [LangId],
    cancel: &'a Cancellation,
) -> Discovery<'a> {
    Discovery {
        root,
        entries,
        path: None,
        languages,
        taken: vec![CheckId::from("static.complexity")],
        cancel,
    }
}

/// TypeScript, which the sample producer serves.
pub fn typescript() -> Lang {
    Lang {
        id: LangId::from("typescript"),
        family: LangFamily::from("ecma"),
    }
}
