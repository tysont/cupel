// ABOUTME: `tool.footprint` (ADR-103) on each language's perf.footprint fixture: dependencies with lines, direct marks,
// ABOUTME: and licenses, OSV advisories, artifact sizes, unconfigured measurements, a root without manifests, the base, and pnpm workspaces.
mod support;

use std::fs;
use std::path::Path;

use cupel_core::{
    Availability, Cancellation, Dependency, FactKind, Footprint, ProduceCtx, Scope, Vulnerability,
    find_program,
};
use cupel_fixtures::write_file;
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
    support::git(dir.path(), &["init", "--quiet"]);
    support::git(dir.path(), &["add", "--all"]);
    support::git(dir.path(), &["commit", "--quiet", "-m", "base"]);
    let base = support::git(dir.path(), &["rev-parse", "HEAD"]);
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

/// A pnpm workspace's lockfile: the root's own tooling, member `apps/a` needing `left-pad`, and
/// member `apps/b` needing a scoped package (resolved with a peer) and linking `apps/a`; the
/// two packages need each other.
const PNPM_LOCK: &str = "lockfileVersion: '9.0'

settings:
  autoInstallPeers: true

importers:

  .:
    devDependencies:
      prettier:
        specifier: ^3.0.0
        version: 3.0.0

  apps/a:
    dependencies:
      left-pad:
        specifier: ^1.3.0
        version: 1.3.0

  apps/b:
    dependencies:
      '@scope/tiny':
        specifier: ^0.1.0
        version: 0.1.0(left-pad@1.3.0)
      a:
        specifier: workspace:*
        version: link:../a

packages:

  '@scope/tiny@0.1.0':
    resolution: {integrity: sha512-x}
    peerDependencies:
      left-pad: '*'

  left-pad@1.3.0:
    resolution: {integrity: sha512-y}

  prettier@3.0.0:
    resolution: {integrity: sha512-z}

snapshots:

  '@scope/tiny@0.1.0(left-pad@1.3.0)':
    dependencies:
      left-pad: 1.3.0

  left-pad@1.3.0:
    dependencies:
      '@scope/tiny': 0.1.0(left-pad@1.3.0)

  prettier@3.0.0: {}
";

/// A pnpm workspace with `PNPM_LOCK` at its root.
fn pnpm_workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write_file(dir.path(), "pnpm-workspace.yaml", "packages:\n  - apps/*\n");
    write_file(dir.path(), "pnpm-lock.yaml", PNPM_LOCK);
    dir
}

/// What `tool.footprint` lists for the TypeScript roots `dirs` of the repository at `root`.
fn produce_roots(root: &Path, dirs: &[&str]) -> Footprint {
    let mut ctx = ctx(root, "typescript", json!({}));
    ctx.upstream
        .insert(cupel_core::Produced::new(support::project_info("typescript", dirs)).share());
    produce(&ctx, "typescript")
}

#[test]
fn a_pnpm_member_lists_what_its_importer_locks_once_across_members_with_installed_licenses() {
    let dir = pnpm_workspace();
    let write = |path: &str, text: &str| write_file(dir.path(), path, text);
    write(
        "package.json",
        "{\"devDependencies\": {\"prettier\": \"^3.0.0\"}}\n",
    );
    write(
        "apps/a/package.json",
        "{\"dependencies\": {\"left-pad\": \"^1.3.0\"}}\n",
    );
    write(
        "apps/b/package.json",
        "{\"dependencies\": {\"@scope/tiny\": \"^0.1.0\", \"a\": \"workspace:*\"}}\n",
    );
    write(
        "node_modules/.pnpm/left-pad@1.3.0/node_modules/left-pad/package.json",
        "{\"name\": \"left-pad\", \"version\": \"1.3.0\", \"license\": \"WTFPL\"}\n",
    );
    let footprint = produce_roots(dir.path(), &["apps/a", "apps/b"]);
    let listed: Vec<_> = footprint
        .dependencies
        .iter()
        .map(|d| {
            (
                d.spelled(),
                d.direct,
                d.license.clone(),
                d.file.as_str(),
                d.line,
            )
        })
        .collect();
    assert_eq!(
        (footprint.manifests, listed, footprint.licenses),
        (
            vec!["pnpm-lock.yaml".to_owned()],
            vec![
                (
                    "left-pad@1.3.0".to_owned(),
                    true,
                    Some("WTFPL".to_owned()),
                    "pnpm-lock.yaml",
                    36
                ),
                (
                    "@scope/tiny@0.1.0".to_owned(),
                    true,
                    None,
                    "pnpm-lock.yaml",
                    31
                ),
            ],
            true,
        )
    );
}

#[test]
fn a_workspace_member_reads_the_workspace_lockfile_and_a_directory_it_does_not_lock_its_manifest() {
    let dir = pnpm_workspace();
    write_file(
        dir.path(),
        "apps/a/package.json",
        "{\"dependencies\": {\"left-pad\": \"^1.3.0\"}}\n",
    );
    // pnpm installs a member from the workspace's lockfile; one left in the member is stale.
    write_file(
        dir.path(),
        "apps/a/pnpm-lock.yaml",
        PNPM_LOCK.replace("1.3.0", "1.0.0").as_str(),
    );
    write_file(
        dir.path(),
        "apps/a/echo/package.json",
        "{\"dependencies\": {\"express\": \"^4\"}}\n",
    );
    let footprint = produce_roots(dir.path(), &["apps/a", "apps/a/echo"]);
    let listed: Vec<_> = footprint
        .dependencies
        .iter()
        .map(|d| (d.spelled(), d.file.as_str()))
        .collect();
    assert_eq!(
        listed,
        [
            ("left-pad@1.3.0".to_owned(), "pnpm-lock.yaml"),
            ("@scope/tiny@0.1.0".to_owned(), "pnpm-lock.yaml"),
            ("express".to_owned(), "apps/a/echo/package.json"),
        ]
    );
}
