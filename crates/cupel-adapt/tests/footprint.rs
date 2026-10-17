// ABOUTME: `tool.footprint` (ADR-103) on each language's perf.footprint fixture: dependencies with lines, direct marks,
// ABOUTME: and licenses, OSV advisories, artifact sizes, unconfigured measurements, a root without manifests, and the base.
mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use cupel_core::{
    Availability, Cancellation, Dependency, FactKind, Footprint, ProduceCtx, Scope, Vulnerability,
    find_program,
};
use serde_json::{Value, json};

const ID: &str = "tool.footprint";

/// One language's fixture as the producer should read it.
struct Expected {
    lang: &'static str,
    manifest: &'static str,
    /// (name, version, direct, license, line) of each dependency, all listed in `manifest`.
    dependencies: [(&'static str, &'static str, bool, Option<&'static str>, u32); 2],
    licenses: bool,
    /// The vulnerable dependency's name and version, and the version that fixes it.
    vulnerable: (&'static str, &'static str, &'static str),
}

const GPL: Option<&str> = Some("GPL-3.0-only");
const MIT: Option<&str> = Some("MIT");

const FIXTURES: [Expected; 5] = [
    Expected {
        lang: "rust",
        manifest: "Cargo.lock",
        dependencies: [
            ("gplthing", "0.1.0", true, GPL, 6),
            ("oldsmall", "0.6.3", true, MIT, 18),
        ],
        licenses: true,
        vulnerable: ("oldsmall", "0.6.3", "0.6.10"),
    },
    Expected {
        lang: "python",
        manifest: "requirements.txt",
        dependencies: [
            ("gplthing", "1.0.0", true, GPL, 1),
            ("oldlib", "2.0.0", true, MIT, 2),
        ],
        licenses: true,
        vulnerable: ("oldlib", "2.0.0", "2.0.1"),
    },
    Expected {
        lang: "javascript",
        manifest: "package-lock.json",
        dependencies: [
            ("gplthing", "1.0.0", true, GPL, 15),
            ("oldlib", "2.0.0", true, MIT, 19),
        ],
        licenses: true,
        vulnerable: ("oldlib", "2.0.0", "2.0.1"),
    },
    Expected {
        lang: "typescript",
        manifest: "package-lock.json",
        dependencies: [
            ("gplthing", "1.0.0", true, GPL, 15),
            ("oldlib", "2.0.0", true, MIT, 19),
        ],
        licenses: true,
        vulnerable: ("oldlib", "2.0.0", "2.0.1"),
    },
    Expected {
        lang: "go",
        manifest: "go.mod",
        dependencies: [
            ("example.com/gplthing", "v1.0.0", true, None, 6),
            ("example.com/oldlib", "v2.0.0", false, None, 7),
        ],
        licenses: false,
        vulnerable: ("example.com/oldlib", "v2.0.0", "2.0.1"),
    },
];

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

fn produced(ctx: &ProduceCtx, lang: &str) -> cupel_core::Result<Footprint> {
    let producer = support::producer(support::shared_registry(), FactKind::Footprint, ID, lang);
    assert_eq!(producer.availability(&ctx.ctx), Availability::Ready);
    producer
        .produce(ctx, &Scope::default(), &mut Vec::new())
        .map(|made| made.downcast::<Footprint>().unwrap())
}

fn produce(ctx: &ProduceCtx, lang: &str) -> Footprint {
    produced(ctx, lang).unwrap()
}

fn configured() -> Value {
    json!({"advisories": "advisories", "artifacts": ["dist/*"]})
}

fn fixture(lang: &str) -> tempfile::TempDir {
    support::own_fixture("perf.footprint", lang)
}

impl Expected {
    fn dependencies(&self) -> Vec<Dependency> {
        self.dependencies
            .iter()
            .map(|(name, version, direct, license, line)| Dependency {
                name: (*name).to_owned(),
                version: Some((*version).to_owned()),
                direct: *direct,
                license: license.map(str::to_owned),
                file: self.manifest.to_owned(),
                line: *line,
            })
            .collect()
    }

    fn vulnerabilities(&self) -> Vec<Vulnerability> {
        let (package, version, fixed) = self.vulnerable;
        vec![Vulnerability {
            id: "OSV-2099-0001".to_owned(),
            package: package.to_owned(),
            version: version.to_owned(),
            summary: format!("Out-of-bounds write in {package}"),
            fixed: Some(fixed.to_owned()),
        }]
    }
}

#[test]
fn each_language_lists_its_dependencies_licenses_and_vulnerabilities() {
    if find_program("cargo").is_none() {
        eprintln!("SKIPPED: cargo not installed; Rust licenses were not read");
        return;
    }
    for expected in &FIXTURES {
        let lang = expected.lang;
        let dir = fixture(lang);
        let footprint = produce(&ctx(dir.path(), lang, configured()), lang);
        let artifacts: Vec<(String, u64)> = footprint
            .artifacts
            .iter()
            .flatten()
            .map(|artifact| (artifact.path.clone(), artifact.bytes))
            .collect();
        assert_eq!(
            (
                footprint.manifests,
                footprint.dependencies,
                footprint.licenses,
                footprint.vulnerabilities,
                artifacts,
                footprint.base,
            ),
            (
                vec![expected.manifest.to_owned()],
                expected.dependencies(),
                expected.licenses,
                Some(expected.vulnerabilities()),
                vec![("dist/bundle.txt".to_owned(), 68)],
                None,
            ),
            "{lang}"
        );
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
    let broken = dir.path().join("advisories/broken.json");
    fs::write(&broken, "{").unwrap();
    let error = produced(&ctx(dir.path(), "go", configured()), "go").unwrap_err();
    assert_eq!(
        error.to_string(),
        format!(
            "cannot run `tool.footprint`: cannot read {}: EOF while parsing an object at line 1 \
             column 1",
            broken.display()
        )
    );
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
            .map(Dependency::spelled)
            .collect::<Vec<_>>(),
        ["example.com/gplthing@v1.0.0"]
    );
    assert_eq!(footprint.dependencies.len(), 2);
}
