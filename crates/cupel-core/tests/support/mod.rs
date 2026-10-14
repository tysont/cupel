// ABOUTME: Shared test support for cupel-core integration tests: throwaway git repositories built with
// ABOUTME: the real `git`, the files a test left under a directory, and the languages, manifests, and
// ABOUTME: manifest-only checks the registry and scheduler tests build.
#![allow(
    dead_code,
    reason = "each test binary uses a different subset of the helpers"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use cupel_core::config::{Config, ConfigRequest};
use cupel_core::{
    Cancellation, Check, CheckCtx, CheckId, CheckRegistry, CheckResult, CostClass, EventSink,
    FactKind, Facts, Job, Lang, LangFamily, LangId, Manifest, Plan, Result, RunId, Scope,
    Stability, Tier, Writer,
};

/// The language `id` as a member of `family`.
pub fn lang(id: &str, family: &str) -> Lang {
    Lang {
        id: LangId::from(id),
        family: LangFamily::from(family),
    }
}

/// A tier-0, quick manifest for `id` that needs `needs` and wants `wants`, with no finding
/// kinds or metrics; tests change the rest with struct-update syntax.
pub fn manifest(id: &str, needs: &[FactKind], wants: &[FactKind]) -> Manifest {
    Manifest {
        id: CheckId::from(id),
        version: 1,
        tier: Tier::try_from(0).unwrap(),
        cost: CostClass::Quick,
        needs: needs.to_vec(),
        wants: wants.to_vec(),
        findings: vec![],
        metrics: vec![],
        stability: Stability::Experimental,
        summary: String::new(),
    }
}

/// The configuration of a repository at `root` whose `cupel.toml` holds `toml`.
pub fn config_at(root: &Path, toml: &str) -> Config {
    fs::write(root.join("cupel.toml"), toml).unwrap();
    Config::load(&ConfigRequest {
        root: root.to_path_buf(),
        ..ConfigRequest::default()
    })
    .unwrap()
}

/// A tree-scope job selecting every check of `checks`, over the files `a.x` and `b.x` in
/// `langs` (each its own family).
pub fn job<'a>(
    writer: &'a Writer,
    config: &'a Config,
    checks: &'a CheckRegistry,
    langs: &[&str],
) -> Job<'a> {
    Job {
        run: RunId::generate(),
        writer,
        cancel: Cancellation::new(),
        config,
        selected: checks.iter().collect(),
        langs: langs.iter().map(|id| lang(id, id)).collect(),
        files: vec!["a.x".to_owned(), "b.x".to_owned()],
        selectors: Vec::new(),
        selectors_must_match: true,
        base: None,
        seed: 0,
    }
}

/// A registered check that plans nothing and is never run; only its manifest matters.
pub struct Declared(pub Manifest);

impl Check for Declared {
    fn manifest(&self) -> &Manifest {
        &self.0
    }

    fn plan(&self, _: &CheckCtx, _: &Scope) -> Result<Plan> {
        Ok(Plan {
            targets: 0,
            est_ms: 0,
        })
    }

    fn run(&self, _: &CheckCtx, _: &Facts, _: &Plan, _: &mut dyn EventSink) -> Result<CheckResult> {
        panic!("a declared check is never run")
    }
}

/// Every file under `root`, relative to it and sorted.
pub fn files_under(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                out.push(path.strip_prefix(root).unwrap().display().to_string());
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// A temporary git repository whose default branch is `main`.
pub struct GitRepo {
    dir: tempfile::TempDir,
}

impl GitRepo {
    /// A fresh repository, or `None` (after printing `SKIPPED: git not installed`) without git.
    pub fn new() -> Option<Self> {
        let repo = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        let ok = Command::new("git")
            .args(["init", "--quiet", "--initial-branch", "main"])
            .current_dir(repo.path())
            .output()
            .is_ok_and(|output| output.status.success());
        if !ok {
            eprintln!("SKIPPED: git not installed");
            return None;
        }
        Some(repo)
    }

    /// A fresh repository holding `files` (path, text) in one commit, with that commit's SHA.
    pub fn committed(files: &[(&str, &str)]) -> Option<(Self, String)> {
        let repo = Self::new()?;
        for (file, text) in files {
            repo.write(file, text);
        }
        let base = repo.commit("base");
        Some((repo, base))
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// The canonical root, as the CLI resolves `--cwd`.
    pub fn root(&self) -> PathBuf {
        self.path().canonicalize().unwrap()
    }

    pub fn write(&self, file: &str, text: &str) {
        let path = self.path().join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// Runs `git <args>` and returns its trimmed standard output.
    pub fn git(&self, args: &[&str]) -> String {
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
            .current_dir(self.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    /// Stages everything and commits it; returns the new commit's SHA.
    pub fn commit(&self, message: &str) -> String {
        self.git(&["add", "--all"]);
        self.git(&["commit", "--quiet", "--allow-empty", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
    }
}
