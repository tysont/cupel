// ABOUTME: `tool.footprint` (ADR-103) on each language's perf.footprint fixture: dependencies with lines, direct marks,
// ABOUTME: and licenses, OSV advisories, artifact sizes, unconfigured measurements, a root without manifests, and the base.
mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use cupel_core::{
    Availability, Cancellation, FactKind, Footprint, ProduceCtx, ProducerRegistry, Scope,
    Vulnerability, find_program,
};
use cupel_fixtures::copy_repo;
use serde_json::{Value, json};

const ID: &str = "tool.footprint";

fn registry() -> &'static ProducerRegistry {
    static REGISTRY: std::sync::OnceLock<ProducerRegistry> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(support::registry)
}

/// What a producer of `lang` sees in the repository at `root`, with `config` as its
/// `[lang.<id>]` table.
fn ctx(root: &Path, lang: &str, config: Value) -> ProduceCtx {
    let mut ctx = support::produce_ctx(root, lang, Cancellation::new());
    let Value::Object(config) = config else {
        panic!("config is a table")
    };
    ctx.config = config;
    ctx
}

fn produce(ctx: &ProduceCtx, lang: &str) -> Footprint {
    let producer = support::producer(registry(), FactKind::Footprint, ID, lang);
    assert_eq!(producer.availability(&ctx.ctx), Availability::Ready);
    producer
        .produce(ctx, &Scope::default(), &mut Vec::new())
        .unwrap()
        .downcast::<Footprint>()
        .unwrap()
}

fn configured() -> Value {
    json!({"advisories": "advisories", "artifacts": ["dist/*"]})
}

/// A copy of the perf.footprint fixture for `lang`.
fn fixture(lang: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_repo(&format!("perf.footprint/{lang}"), dir.path());
    dir
}

type Listed = Vec<(String, Option<String>, bool, Option<String>, String, u32)>;

fn listed(footprint: &Footprint) -> Listed {
    footprint
        .dependencies
        .iter()
        .map(|d| {
            (
                d.name.clone(),
                d.version.clone(),
                d.direct,
                d.license.clone(),
                d.file.clone(),
                d.line,
            )
        })
        .collect()
}

fn dep(
    name: &str,
    version: &str,
    direct: bool,
    license: Option<&str>,
    file: &str,
    line: u32,
) -> (String, Option<String>, bool, Option<String>, String, u32) {
    (
        name.to_owned(),
        Some(version.to_owned()),
        direct,
        license.map(str::to_owned),
        file.to_owned(),
        line,
    )
}

#[test]
fn each_language_lists_its_dependencies_licenses_and_vulnerabilities() {
    if find_program("cargo").is_none() {
        eprintln!("SKIPPED: cargo not installed; Rust licenses were not read");
        return;
    }
    let gpl = Some("GPL-3.0-only");
    let mit = Some("MIT");
    let cases: [(&str, &str, Listed, bool, &str, &str); 5] = [
        (
            "rust",
            "Cargo.lock",
            vec![
                dep("gplthing", "0.1.0", true, gpl, "Cargo.lock", 6),
                dep("oldsmall", "0.6.3", true, mit, "Cargo.lock", 18),
            ],
            true,
            "oldsmall",
            "0.6.3",
        ),
        (
            "python",
            "requirements.txt",
            vec![
                dep("gplthing", "1.0.0", true, gpl, "requirements.txt", 1),
                dep("oldlib", "2.0.0", true, mit, "requirements.txt", 2),
            ],
            true,
            "oldlib",
            "2.0.0",
        ),
        (
            "javascript",
            "package-lock.json",
            vec![
                dep("gplthing", "1.0.0", true, gpl, "package-lock.json", 15),
                dep("oldlib", "2.0.0", true, mit, "package-lock.json", 19),
            ],
            true,
            "oldlib",
            "2.0.0",
        ),
        (
            "typescript",
            "package-lock.json",
            vec![
                dep("gplthing", "1.0.0", true, gpl, "package-lock.json", 15),
                dep("oldlib", "2.0.0", true, mit, "package-lock.json", 19),
            ],
            true,
            "oldlib",
            "2.0.0",
        ),
        (
            "go",
            "go.mod",
            vec![
                dep("example.com/gplthing", "v1.0.0", true, None, "go.mod", 6),
                dep("example.com/oldlib", "v2.0.0", false, None, "go.mod", 7),
            ],
            false,
            "example.com/oldlib",
            "v2.0.0",
        ),
    ];
    for (lang, manifest, expected, licenses, package, version) in cases {
        let dir = fixture(lang);
        let footprint = produce(&ctx(dir.path(), lang, configured()), lang);
        assert_eq!(footprint.manifests, [manifest], "{lang}");
        assert_eq!(listed(&footprint), expected, "{lang}");
        assert_eq!(footprint.licenses, licenses, "{lang}");
        assert_eq!(
            footprint.vulnerabilities,
            Some(vec![Vulnerability {
                id: "OSV-2099-0001".to_owned(),
                package: package.to_owned(),
                version: version.to_owned(),
                summary: format!("Out-of-bounds write in {package}"),
                fixed: Some(if lang == "rust" { "0.6.10" } else { "2.0.1" }.to_owned()),
            }]),
            "{lang}"
        );
        let artifacts = footprint.artifacts.expect("configured");
        assert_eq!(
            artifacts
                .iter()
                .map(|a| (a.path.as_str(), a.bytes))
                .collect::<Vec<_>>(),
            [("dist/bundle.txt", 68)],
            "{lang}"
        );
        assert_eq!(footprint.base, None, "{lang}");
    }
}

#[test]
fn unconfigured_measurements_are_absent_and_a_root_without_manifests_lists_nothing() {
    let dir = fixture("python");
    let footprint = produce(&ctx(dir.path(), "python", json!({})), "python");
    assert_eq!(footprint.vulnerabilities, None);
    assert_eq!(footprint.artifacts, None);
    let empty = tempfile::tempdir().unwrap();
    for lang in ["rust", "python", "javascript", "typescript", "go"] {
        let footprint = produce(&ctx(empty.path(), lang, json!({})), lang);
        assert_eq!(footprint, Footprint::default(), "{lang}");
    }
}

#[test]
fn an_advisory_that_does_not_parse_is_an_error_naming_it() {
    let dir = fixture("go");
    fs::write(dir.path().join("advisories/broken.json"), "{").unwrap();
    let producer = support::producer(registry(), FactKind::Footprint, ID, "go");
    let error = producer
        .produce(
            &ctx(dir.path(), "go", configured()),
            &Scope::default(),
            &mut Vec::new(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("broken.json"), "{error}");
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn under_a_range_the_same_manifests_are_read_at_the_base_commit() {
    let dir = fixture("go");
    let gomod = dir.path().join("go.mod");
    let head = fs::read_to_string(&gomod).unwrap();
    fs::write(
        &gomod,
        head.replace("\texample.com/oldlib v2.0.0 // indirect\n", ""),
    )
    .unwrap();
    git(dir.path(), &["init", "--quiet"]);
    git(dir.path(), &["add", "--all"]);
    git(dir.path(), &["commit", "--quiet", "-m", "base"]);
    let base = git(dir.path(), &["rev-parse", "HEAD"]);
    fs::write(&gomod, head).unwrap();
    let mut ranged = ctx(dir.path(), "go", json!({}));
    ranged.focus.base = Some(base.clone());
    let footprint = produce(&ranged, "go");
    let before = footprint.base.expect("a base under a range");
    assert_eq!(before.commit, base);
    assert_eq!(
        before
            .dependencies
            .iter()
            .map(cupel_core::Dependency::spelled)
            .collect::<Vec<_>>(),
        ["example.com/gplthing@v1.0.0"]
    );
    assert_eq!(footprint.dependencies.len(), 2);
}
