// ABOUTME: The sizes of the artifacts `[lang.<id>] artifacts` names (ADR-103 §4): each glob is walked from its literal
// ABOUTME: directory prefix, ignore files disregarded since build outputs are usually ignored, matched files sized.
use std::collections::BTreeMap;
use std::path::Path;

use cupel_core::{Artifact, ProduceCtx, Result};
use globset::Glob;
use serde_json::Value;

use super::{ARTIFACTS_KEY, unreadable};

/// The configured artifacts, by path; `None` when the language configures none.
pub fn configured(ctx: &ProduceCtx) -> Result<Option<Vec<Artifact>>> {
    let Some(patterns) = ctx.config.get(ARTIFACTS_KEY).and_then(Value::as_array) else {
        return Ok(None);
    };
    let root = &ctx.ctx.root;
    let mut sized = BTreeMap::new();
    for pattern in patterns.iter().filter_map(Value::as_str) {
        let glob = Glob::new(pattern)
            .map_err(|error| unreadable(Path::new(pattern), &error.to_string()))?
            .compile_matcher();
        let start = root.join(literal_prefix(pattern));
        for entry in ignore::WalkBuilder::new(&start)
            .standard_filters(false)
            .build()
            .flatten()
        {
            let path = entry.path();
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            let relative = relative.to_string_lossy().replace('\\', "/");
            if !glob.is_match(&relative) {
                continue;
            }
            if let Ok(metadata) = std::fs::metadata(path)
                && metadata.is_file()
            {
                sized.insert(relative, metadata.len());
            }
        }
    }
    Ok(Some(
        sized
            .into_iter()
            .map(|(path, bytes)| Artifact { path, bytes })
            .collect(),
    ))
}

/// The directories of `pattern` before its first glob character: where its matches can be.
fn literal_prefix(pattern: &str) -> String {
    let mut prefix = Vec::new();
    let segments: Vec<&str> = pattern.split('/').collect();
    for segment in &segments[..segments.len().saturating_sub(1)] {
        if segment.contains(['*', '?', '[', '{']) {
            break;
        }
        prefix.push(*segment);
    }
    if prefix.is_empty() {
        ".".to_owned()
    } else {
        prefix.join("/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pattern_is_walked_from_its_literal_directories() {
        assert_eq!(literal_prefix("target/release/app"), "target/release");
        assert_eq!(literal_prefix("dist/**/*.js"), "dist");
        assert_eq!(literal_prefix("*.wasm"), ".");
        assert_eq!(literal_prefix("build/{a,b}/out.bin"), "build");
    }
}
