// ABOUTME: Tests for scripts/schemas.sh, which writes every published JSON Schema for GitHub Pages (ADR-127): each
// ABOUTME: `$id` the binary emits maps to a file served at that URL, nothing else is written, and an index lists them.
use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use cupel_core::FactKind;
use serde_json::Value;
use tempfile::TempDir;

/// The GitHub Pages site of this repository, which serves the directory the script writes.
const SITE: &str = "https://tysont.github.io/cupel/";

fn script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/schemas.sh")
}

fn schemas(args: &[&str], cwd: &Path) -> Output {
    Command::new("sh")
        .arg(script())
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("sh runs scripts/schemas.sh")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The JSON document `cupel <args> --format json` prints.
fn cupel(args: &[&str], cwd: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_cupel"))
        .args(args)
        .args(["--format", "json"])
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "cupel {args:?}: {}",
        stderr(&output)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

/// Every `$id` anywhere in `value`.
fn ids(value: &Value, found: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(id)) = map.get("$id") {
                found.insert(id.clone());
            }
            map.values().for_each(|value| ids(value, found));
        }
        Value::Array(items) => items.iter().for_each(|value| ids(value, found)),
        _ => {}
    }
}

/// Every `$id` the binary emits: in `describe --all` and in `describe fact <kind>` for every kind.
fn emitted_ids(cwd: &Path) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    ids(&cupel(&["describe", "--all"], cwd), &mut found);
    for kind in FactKind::all() {
        ids(
            &cupel(&["describe", "fact", kind.as_str()], cwd),
            &mut found,
        );
    }
    found
}

/// Every file under `dir`, as paths relative to it with `/` separators.
fn files(dir: &Path, prefix: &str, found: &mut BTreeSet<String>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
        if entry.file_type().unwrap().is_dir() {
            files(&entry.path(), &format!("{name}/"), found);
        } else {
            found.insert(name);
        }
    }
}

/// The site `scripts/schemas.sh` writes for the built binary, in a directory that lives as long as this does.
struct Site {
    dir: TempDir,
}

impl Site {
    fn write() -> Site {
        let dir = TempDir::new().unwrap();
        let output = schemas(&[env!("CARGO_BIN_EXE_cupel"), "site"], dir.path());
        assert!(output.status.success(), "schemas.sh: {}", stderr(&output));
        assert_eq!(stderr(&output), "");
        Site { dir }
    }

    fn root(&self) -> PathBuf {
        self.dir.path().join("site")
    }

    fn files(&self) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        files(&self.root(), "", &mut found);
        found
    }

    /// The `$id` and `$schema` of the file served at the URL `id`.
    fn served(&self, id: &str) -> (Value, Value) {
        let path = id
            .strip_prefix(SITE)
            .unwrap_or_else(|| panic!("`{id}` is not under the Pages site {SITE}"));
        let file = self.root().join(path);
        let text = fs::read_to_string(&file)
            .unwrap_or_else(|error| panic!("{id}: {}: {error}", file.display()));
        let schema: Value = serde_json::from_str(&text).unwrap();
        (schema["$id"].clone(), schema["$schema"].clone())
    }
}

const DRAFT: &str = "https://json-schema.org/draft/2020-12/schema";

/// The five top-level schemas and one per fact kind.
fn published_count() -> usize {
    5 + FactKind::all().len()
}

#[test]
fn every_emitted_id_is_a_file_whose_id_is_the_url_it_is_served_at() {
    let site = Site::write();
    let ids = emitted_ids(site.dir.path());
    assert_eq!(ids.len(), published_count(), "{ids:?}");
    let served: Vec<(Value, Value)> = ids.iter().map(|id| site.served(id)).collect();
    let expected: Vec<(Value, Value)> = ids
        .iter()
        .map(|id| (Value::from(id.as_str()), Value::from(DRAFT)))
        .collect();
    assert_eq!(served, expected);
}

#[test]
fn the_site_holds_the_emitted_schemas_and_the_index_only() {
    let site = Site::write();
    let mut expected: BTreeSet<String> = emitted_ids(site.dir.path())
        .iter()
        .map(|id| id.strip_prefix(SITE).unwrap().to_owned())
        .collect();
    expected.insert("index.html".to_owned());
    assert_eq!(site.files(), expected);
}

#[test]
fn the_root_schemas_and_each_fact_kind_land_under_schema_v1() {
    let files = Site::write().files();
    let mut expected: BTreeSet<String> = ["run", "finding", "events", "config", "plugin"]
        .iter()
        .map(|name| format!("schema/v1/{name}.json"))
        .chain(
            FactKind::all()
                .iter()
                .map(|kind| format!("schema/v1/fact/{kind}.json")),
        )
        .collect();
    expected.insert("index.html".to_owned());
    assert_eq!(files, expected);
}

#[test]
fn the_index_links_every_schema_by_its_url() {
    let site = Site::write();
    let index = fs::read_to_string(site.root().join("index.html")).unwrap();
    assert_eq!(index.lines().next(), Some("<!doctype html>"));
    let links: BTreeSet<&str> = index
        .lines()
        .filter(|line| line.starts_with("<li>"))
        .collect();
    let expected: BTreeSet<String> = site
        .files()
        .iter()
        .filter(|path| *path != "index.html")
        .map(|path| format!("<li><a href=\"{path}\">{SITE}{path}</a></li>"))
        .collect();
    assert_eq!(links, expected.iter().map(String::as_str).collect());
    assert_eq!(links.len(), published_count());
}

#[test]
fn a_missing_argument_is_a_usage_error() {
    let dir = TempDir::new().unwrap();
    let output = schemas(&[env!("CARGO_BIN_EXE_cupel")], dir.path());
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stderr(&output), "usage: schemas.sh BINARY OUTDIR\n");
}

#[test]
fn an_id_outside_the_pages_site_fails() {
    let dir = TempDir::new().unwrap();
    let binary = dir.path().join("cupel");
    fs::write(
        &binary,
        "#!/bin/sh\n\
         echo '{\"description\":{\"schemas\":[{\"schema\":{\"$id\":\"https://example.com/schema/v1/run.json\"}}]}}'\n",
    )
    .unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    let output = schemas(&[binary.to_str().unwrap(), "site"], dir.path());
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        format!("schemas.sh: https://example.com/schema/v1/run.json is not under {SITE}\n")
    );
}
