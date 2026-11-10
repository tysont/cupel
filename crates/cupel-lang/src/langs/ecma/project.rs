// ABOUTME: npm, pnpm, and yarn roots for `ProjectInfo`: `package.json`, TypeScript when the root or a member has a
// ABOUTME: `tsconfig.json` and JavaScript otherwise, and the files the root's manifests say it runs (ADR-021, ADR-037, ADR-163).
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use cupel_core::syntax::ProjectRoot;
use serde_json::Value;

use crate::project::{Candidate, Ecosystem};

pub const ECOSYSTEM: Ecosystem = Ecosystem {
    manifests: &["package.json"],
    skip_dirs: &["node_modules", "dist", "coverage"],
    describe,
};

/// The extensions of the files a JavaScript or TypeScript project runs.
const SOURCES: &[&str] = &["js", "mjs", "cjs", "jsx", "ts", "mts", "cts", "tsx"];

/// The programs a `package.json` script runs a source file with.
const LAUNCHERS: &[&str] = &["node", "tsx", "ts-node", "bun"];

fn describe(candidate: &Candidate) -> ProjectRoot {
    let typescript = candidate
        .dirs()
        .any(|dir| dir.join("tsconfig.json").is_file());
    let lang = if typescript {
        "typescript"
    } else {
        "javascript"
    };
    ProjectRoot {
        entries: entries(candidate),
        ..candidate.root(lang, Vec::new())
    }
}

/// Every file the root's and its members' manifests name as one the project runs or ships, that
/// exists, relative to the root and sorted: `package.json`'s `main`, `bin`, and the files its
/// scripts run with a launcher; a `wrangler*.toml`'s `main`; and the sources a `Dockerfile` copies
/// into its image (ADR-163).
fn entries(candidate: &Candidate) -> Vec<String> {
    let mut found = BTreeSet::new();
    for dir in candidate.dirs() {
        let named = package_entries(dir)
            .into_iter()
            .chain(wrangler_entries(dir))
            .chain(docker_entries(dir));
        for name in named {
            let path = dir.join(name.trim_start_matches("./"));
            let is_source = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| SOURCES.contains(&extension));
            if is_source
                && path.is_file()
                && let Ok(relative) = path.strip_prefix(&candidate.dir)
            {
                found.insert(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    found.into_iter().collect()
}

/// `package.json`'s `main`, its `bin` (one path or a map of them), and each script's argument
/// that follows a launcher.
fn package_entries(dir: &Path) -> Vec<String> {
    let Some(package) = fs::read_to_string(dir.join("package.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    else {
        return Vec::new();
    };
    let strings = |value: Option<&Value>| -> Vec<String> {
        match value {
            Some(Value::String(path)) => vec![path.clone()],
            Some(Value::Object(paths)) => paths
                .values()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            _ => Vec::new(),
        }
    };
    let scripts = strings(package.get("scripts"))
        .into_iter()
        .flat_map(|script| launched(&script));
    strings(package.get("main"))
        .into_iter()
        .chain(strings(package.get("bin")))
        .chain(scripts)
        .collect()
}

/// The first argument after each launcher word in `script` that is not a flag.
fn launched(script: &str) -> Vec<String> {
    let words: Vec<&str> = script.split_whitespace().collect();
    words
        .iter()
        .enumerate()
        .filter(|(_, word)| LAUNCHERS.contains(word))
        .filter_map(|(at, _)| {
            words[at + 1..]
                .iter()
                .find(|word| !word.starts_with('-'))
                .map(|word| (*word).to_owned())
        })
        .collect()
}

/// The `main` of each `wrangler*.toml` in `dir`: the module a Cloudflare Worker runs.
fn wrangler_entries(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with("wrangler") && name.ends_with(".toml")
        })
        .filter_map(|entry| fs::read_to_string(entry.path()).ok())
        .filter_map(|text| text.parse::<toml::Table>().ok())
        .filter_map(|table| table.get("main")?.as_str().map(str::to_owned))
        .collect()
}

/// The sources each `COPY` of `dir`'s `Dockerfile` takes from the build context (not from another
/// stage): the files the image ships, which it runs or loads.
fn docker_entries(dir: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(dir.join("Dockerfile")) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            words
                .next()
                .is_some_and(|word| word.eq_ignore_ascii_case("COPY"))
                .then_some(words)
        })
        .flat_map(|words| {
            let words: Vec<&str> = words.collect();
            if words.iter().any(|word| word.starts_with("--from")) {
                return Vec::new();
            }
            let paths: Vec<&str> = words
                .into_iter()
                .filter(|word| !word.starts_with("--"))
                .collect();
            paths.split_last().map_or_else(Vec::new, |(_, sources)| {
                sources.iter().map(|source| (*source).to_owned()).collect()
            })
        })
        .collect()
}
