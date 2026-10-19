// ABOUTME: Helpers shared by the CLI integration tests: invoking the binary, reading envelopes, and the
// ABOUTME: registries tests derive their expectations from. Each test file includes it with `mod support;`.
#![allow(
    dead_code,
    reason = "each test crate uses a different subset of the helpers"
)]

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use assert_cmd::Command;
use cupel_core::{CheckId, CheckRegistry, Envelope, FindingId, Writer};
use cupel_store::Store;
use serde_json::{Value, json};
use tempfile::TempDir;

/// The `cupel` binary with a clean environment: no `CUPEL_*` variables leak in from the caller.
pub fn cupel() -> Command {
    let mut command = Command::cargo_bin("cupel").expect("cupel binary is built");
    for (key, _) in std::env::vars() {
        if key.starts_with("CUPEL_") {
            command.env_remove(key);
        }
    }
    command
}

/// What one invocation produced.
pub struct Outcome {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Outcome {
    /// Stdout parsed as JSON, after checking it is a well-typed envelope whose `exit_code`
    /// matches the process exit code.
    pub fn envelope(&self) -> Value {
        let value: Value = serde_json::from_str(&self.stdout)
            .unwrap_or_else(|e| panic!("stdout is not JSON ({e}):\n{}", self.stdout));
        let typed: Envelope = serde_json::from_value(value.clone())
            .unwrap_or_else(|e| panic!("stdout is not an envelope ({e}):\n{}", self.stdout));
        assert_eq!(
            i32::from(typed.exit_code.code()),
            self.code,
            "exit_code field"
        );
        value
    }

    /// The envelope's only finding.
    pub fn finding(&self) -> Value {
        let envelope = self.envelope();
        let findings = envelope["findings"].as_array().expect("findings array");
        assert_eq!(findings.len(), 1, "exactly one finding: {findings:#?}");
        findings[0].clone()
    }
}

pub fn run<I, S>(args: I) -> Outcome
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    run_with(&mut cupel(), args)
}

/// Runs `command` with `args` in a fresh temporary working directory, so a verb that keeps a run
/// record under `.cupel/` never writes into the source tree; tests that inspect what a run
/// wrote pass `--cwd` explicitly.
pub fn run_with<I, S>(command: &mut Command, args: I) -> Outcome
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let dir = tempfile::tempdir().expect("temporary directory");
    let output = command
        .args(args)
        .current_dir(dir.path())
        .output()
        .expect("cupel runs");
    Outcome {
        code: output.status.code().expect("exit code"),
        stdout: String::from_utf8(output.stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(output.stderr).expect("stderr is UTF-8"),
    }
}

/// The `description` of `cupel describe <args>`, which must pass.
pub fn described(args: &[&str]) -> Value {
    let outcome = run(["describe"].into_iter().chain(args.iter().copied()));
    assert_eq!(
        outcome.code, 0,
        "cupel describe {args:?}: {}",
        outcome.stdout
    );
    outcome.envelope()["description"].clone()
}

/// Every native check, registered as the binary registers them. Tests derive the set of checks
/// from here and assert the binary covers it, so a registered check `describe` leaves out fails.
pub fn registered_checks() -> CheckRegistry {
    let mut registry = CheckRegistry::default();
    cupel_checks::register(&mut registry);
    registry
}

/// The registered checks' IDs, in registration (ID) order.
pub fn check_ids() -> Vec<String> {
    registered_checks()
        .iter()
        .map(|check| check.manifest().id.to_string())
        .collect()
}

/// The compiled-in languages' IDs, in the order `describe` lists them.
pub fn language_ids() -> Vec<String> {
    cupel_lang::LanguageRegistry::builtin()
        .ids()
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// The envelope with run-specific values replaced, for snapshots.
pub fn redacted(mut envelope: Value) -> Value {
    envelope["run"]["id"] = json!("[run id]");
    envelope["run"]["started"] = json!("[started]");
    envelope["run"]["duration_ms"] = json!(0);
    envelope["run"]["seed"] = json!("[seed]");
    envelope["cupel"]["version"] = json!("[version]");
    if let Some(description) = envelope["description"].as_object_mut()
        && description.contains_key("version")
    {
        description.insert("version".into(), json!("[version]"));
    }
    envelope
}

/// Text output with run-specific values replaced, for snapshots: the `run` line (ID, start time,
/// duration) and the version.
pub fn redacted_text(text: &str) -> String {
    text.lines()
        .map(|line| {
            if line.starts_with("run       run_") {
                "run       [run id, start, duration, seed]".to_owned()
            } else {
                line.replace(env!("CARGO_PKG_VERSION"), "[version]")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// `field` of every finding in `envelope`, in order.
pub fn finding_fields(envelope: &Value, field: &str) -> Vec<String> {
    envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| finding[field].as_str().unwrap().to_owned())
        .collect()
}

/// The finding `id` in `envelope`.
pub fn finding<'a>(envelope: &'a Value, id: &str) -> &'a Value {
    envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["id"] == json!(id))
        .unwrap_or_else(|| panic!("no {id} in {envelope}"))
}

/// The argument a usage error (exit 2, one `cli.usage` finding) names.
pub fn usage_argument(outcome: &Outcome) -> Value {
    assert_eq!(outcome.code, 2, "{}", outcome.stdout);
    let finding = outcome.finding();
    assert_eq!(finding["kind"], json!("cli.usage"));
    finding["evidence"]["argument"].clone()
}

/// The ID ADR-016 assigns a CLI finding about `subject`.
pub fn cli_finding_id(kind: &str, subject: &str) -> String {
    FindingId::for_subject(&CheckId::from("cli"), kind, subject).to_string()
}

/// A temporary repository the binary is pointed at with `--cwd`.
pub struct Repo {
    dir: TempDir,
}

impl Repo {
    pub fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("temporary directory"),
        }
    }

    pub fn root(&self) -> PathBuf {
        self.dir.path().canonicalize().unwrap()
    }

    /// A copy of the fixture repository `<check>/python`; `None` (reported as skipped) without
    /// pytest, which runs its suite.
    pub fn python_fixture(check: &str) -> Option<Self> {
        if cupel_core::find_program("pytest").is_none() {
            eprintln!("SKIPPED: pytest not installed; {check} was not run");
            return None;
        }
        let repo = Self::new();
        cupel_fixtures::copy_repo(&format!("{check}/python"), &repo.root());
        Some(repo)
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root().join(relative)
    }

    /// Writes `text` to `relative`, creating parent directories.
    pub fn write(&self, relative: &str, text: &str) {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// Runs `git <args>` in this repository with a throwaway identity and no signing; whether it
    /// succeeded (false also when git is not installed).
    pub fn git(&self, args: &[&str]) -> bool {
        std::process::Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
            .args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"])
            .args(args)
            .current_dir(self.root())
            .output()
            .is_ok_and(|output| output.status.success())
    }

    /// Runs cupel with `--cwd` set to this repository.
    pub fn run(&self, args: &[&str]) -> Outcome {
        self.run_with(&mut cupel(), args)
    }

    /// Runs `command` with `args` and `--cwd` set to this repository.
    pub fn run_with(&self, command: &mut Command, args: &[&str]) -> Outcome {
        let root = self.root().display().to_string();
        let mut all: Vec<&str> = args.to_vec();
        all.extend(["--cwd", root.as_str()]);
        run_with(command, all)
    }

    /// Every path under the root with its content, sorted.
    pub fn tree(&self) -> Vec<(String, Vec<u8>)> {
        fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, Vec<u8>)>) {
            for entry in fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                let name = path.strip_prefix(root).unwrap().display().to_string();
                if path.is_dir() {
                    out.push((format!("{name}/"), Vec::new()));
                    walk(&path, root, out);
                } else {
                    out.push((name, fs::read(&path).unwrap()));
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.root(), &self.root(), &mut out);
        out.sort();
        out
    }

    /// The paths of `tree()` outside `.cupel/`, with their content.
    pub fn tree_outside_cupel(&self) -> Vec<(String, Vec<u8>)> {
        self.tree()
            .into_iter()
            .filter(|(name, _)| !name.starts_with(".cupel/"))
            .collect()
    }

    /// This repository's fact store, opened (and created if needed) as the CLI opens it.
    pub fn store(&self) -> Store {
        Store::open(&Writer::new(&self.root(), false).unwrap()).unwrap()
    }

    /// Records `envelopes` in this repository's fact store, as if those runs had happened.
    pub fn seed(&self, envelopes: &[Envelope]) {
        let store = self.store();
        for envelope in envelopes {
            store.record_run(envelope).unwrap();
        }
    }
}

/// A run ID whose last two characters are `n`, so higher `n` is a later run.
pub fn run_id(n: u8) -> String {
    format!("run_01JA2B3C4D5E6F7G8H9JKMNP{n:02}")
}

/// A finished `verb` run `run` with `findings` and `checks` (JSON values of the envelope types).
pub fn envelope(run: &str, verb: &str, findings: &[Value], checks: &[Value]) -> Envelope {
    serde_json::from_value(json!({
        "cupel": {"version": "0.1.0", "schema": "https://cupel.dev/schema/v1/run.json"},
        "run": {
            "id": run, "verb": verb, "started": "2026-10-13T10:00:00Z", "duration_ms": 1,
            "git": null, "scope": null, "depth": "standard", "budget_ms": null, "seed": 7,
            "toolchain": {}, "config": {"effective": {}, "sources": []}, "baseline": null
        },
        "status": "pass", "exit_code": 0, "checks": checks,
        "total_findings": findings.len(), "findings": findings,
        "summary": {"findings": {"fail": 0, "warn": findings.len(), "info": 0}, "new": 0,
                    "resolved": 0, "suppressed": 0, "gate": null},
        "next_actions": [], "truncated": false
    }))
    .expect("a valid envelope")
}

/// A `static.complexity` finding `id` about `src/lib.rs` lines `line..=end_line`, with test hints.
pub fn complexity_finding(id: &str, line: u32, end_line: u32) -> Value {
    json!({
        "id": id, "check": "static.complexity", "kind": "complexity.high",
        "severity": "warn", "confidence": 1.0, "tier": 1,
        "symbol": {"display": "lib::tangled", "hash": "sh_00000000000000aa", "lang": "rust"},
        "location": {"file": "src/lib.rs", "line": line, "col": 1, "end_line": end_line, "end_col": 2},
        "title": "Cyclomatic complexity 12 exceeds 10", "detail": null,
        "evidence": {"cyclomatic": 12, "cognitive": 9},
        "action": {"kind": "reduce_complexity", "priority": 1, "summary": "Split tangled.",
                   "hints": {"nearest_test_file": "tests/lib.rs", "covering_tests": ["tangled_works"]}},
        "repro": "cupel check --only static.complexity",
        "suppress": format!("cupel suppress {id} --reason '…'"),
        "new_since_baseline": false, "suppressed": false
    })
}

/// A `static.complexity` check result scored `score`.
pub fn complexity_check(score: f64) -> Value {
    json!({
        "id": "static.complexity", "status": "pass", "tier": 1, "cost": "quick",
        "score": {"value": score, "unit": "ratio", "n": 4}, "metrics": {},
        "delta": null, "findings": 1, "duration_ms": 3, "skipped": null, "oracle": null
    })
}

/// The check plugin `ref.sample` (ADR-088): needs the symbols of each language and reports one finding.
pub fn manifest() -> Value {
    json!({
        "kind": "check", "id": "ref.sample", "version": 1, "tier": 1, "cost": "quick",
        "needs": ["symbols"], "wants": [],
        "findings": [{
            "kind": "ref.sample-file", "action": "review", "summary": "A sample finding.",
            "evidence": {"type": "object", "properties": {"file": {"type": "string"}}}
        }],
        "metrics": [{"name": "files", "summary": "Files seen.", "per": "scope"}],
        "stability": "experimental", "summary": "A sample check plugin."
    })
}

fn sample_finding() -> Value {
    json!({
        "ev": "finding", "id": "fnd_0a1b2c", "check": "ref.sample", "kind": "ref.sample-file",
        "severity": "warn", "confidence": 1.0, "tier": 1, "symbol": null,
        "location": {"file": "src/a.py", "line": 1, "col": null, "end_line": 1, "end_col": null},
        "title": "src/a.py was seen", "detail": null, "evidence": {"file": "src/a.py"},
        "action": {"kind": "review", "priority": 1, "summary": "Look at it.", "hints": {}},
        "repro": "cupel check --only ref.sample", "suppress": "cupel suppress fnd_0a1b2c",
        "new_since_baseline": false, "suppressed": false
    })
}

/// Writes the plugin script `name` in `dir`: it describes `manifest`, plans 1 target in 25 ms,
/// and runs by saving its request beside itself, then reporting one finding and a failing result.
pub fn plugin(dir: &Path, name: &str, manifest: &Value) {
    let result = json!({
        "ev": "result", "status": "fail", "score": null, "metrics": {"files": 1}, "notes": []
    });
    let body = format!(
        "#!/bin/sh\ncase \"$1\" in\n\
         describe) echo '{manifest}' ;;\n\
         plan) echo '{{\"targets\": 1, \"est_ms\": 25}}' ;;\n\
         run) cat > '{saved}'; echo '{finding}'; echo '{result}' ;;\n\
         esac\n",
        saved = dir.join(format!("{name}.request.json")).display(),
        finding = sample_finding(),
    );
    let path = dir.join(name);
    fs::write(&path, body).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// The tools `cupel serve --mcp` lists, asked over its stdin (ADR-121).
pub fn listed_tools() -> Vec<Value> {
    let dir = tempfile::tempdir().expect("temporary directory");
    let mut command = std::process::Command::new(assert_cmd::cargo::cargo_bin("cupel"));
    for (key, _) in std::env::vars() {
        if key.starts_with("CUPEL_") {
            command.env_remove(key);
        }
    }
    let mut server = command
        .args(["serve", "--mcp", "--quiet"])
        .current_dir(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("cupel serve --mcp starts");
    let mut input = server.stdin.take().expect("stdin is piped");
    writeln!(input, r#"{{"jsonrpc":"2.0","id":1,"method":"tools/list"}}"#).expect("it reads");
    drop(input);
    let output = server.wait_with_output().expect("it answers and exits");
    let stdout = String::from_utf8(output.stdout).expect("UTF-8");
    let response: Value = serde_json::from_str(stdout.lines().next().expect("a response"))
        .expect("a JSON-RPC response");
    response["result"]["tools"]
        .as_array()
        .expect("tools")
        .clone()
}
