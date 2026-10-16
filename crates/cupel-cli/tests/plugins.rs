// ABOUTME: Plugins through the CLI (ADR-088, ADR-089): a configured or PATH plugin joins `describe`, `check`, and
// ABOUTME: configuration validation like a native check; `--no-path-plugins` hides PATH plugins; broken ones are usage errors.
#![cfg(unix)]

mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use serde_json::{Value, json};
use support::{cupel, run_with};

/// The check `ref.sample`: needs the symbols of each language and reports one finding.
fn manifest() -> Value {
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

fn finding() -> Value {
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
fn plugin(dir: &Path, name: &str, manifest: &Value) {
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
        finding = finding(),
    );
    let path = dir.join(name);
    fs::write(&path, body).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// A Python repository whose `cupel.toml` holds `config`.
fn repository(config: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("src/a.py"),
        "def add(a, b):\n    return a + b\n",
    )
    .unwrap();
    fs::write(dir.path().join("cupel.toml"), config).unwrap();
    dir
}

fn configured() -> tempfile::TempDir {
    let dir = repository("[plugins]\nsample = \"./sample.sh\"\n");
    plugin(dir.path(), "sample.sh", &manifest());
    dir
}

fn cupel_in(dir: &Path, args: &[&str]) -> support::Outcome {
    let mut args: Vec<&str> = args.to_vec();
    let cwd = dir.display().to_string();
    args.extend(["--cwd", &cwd, "--format", "json"]);
    run_with(&mut cupel(), args)
}

#[test]
fn a_configured_check_plugin_runs_with_its_facts_as_files() {
    let dir = configured();
    let out = cupel_in(dir.path(), &["check", "--only", "ref.sample"]);
    let envelope = out.envelope();
    let result = &envelope["checks"][0];
    assert_eq!(
        (&result["id"], &result["status"], &result["findings"]),
        (&json!("ref.sample"), &json!("fail"), &json!(1)),
        "{}",
        out.stdout
    );
    assert_eq!(envelope["findings"][0]["kind"], "ref.sample-file");
    let request: Value = serde_json::from_str(
        &fs::read_to_string(dir.path().join("sample.sh.request.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(request["scope"]["files"], json!(["src/a.py"]));
    let symbols: Value = serde_json::from_str(
        &fs::read_to_string(request["facts"]["symbols"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    assert!(symbols.to_string().contains("\"add\""), "{symbols}");
}

#[test]
fn a_check_plugin_is_described_planned_and_configured_like_a_native_check() {
    let dir = configured();
    let described = cupel_in(dir.path(), &["describe", "ref.sample"]).envelope();
    assert_eq!(
        described["description"]["manifest"]["summary"],
        "A sample check plugin."
    );
    let kind = cupel_in(dir.path(), &["describe", "finding", "ref.sample-file"]).envelope();
    assert_eq!(kind["description"]["check"], "ref.sample");
    let plan = cupel_in(dir.path(), &["check", "--only", "ref.sample", "--dry-run"]).envelope();
    assert_eq!(plan["plan"]["would_run"][0]["id"], "ref.sample", "{plan}");
    assert_eq!(plan["plan"]["would_run"][0]["est_ms"], 25, "{plan}");

    fs::write(
        dir.path().join("cupel.toml"),
        "[plugins]\nsample = \"./sample.sh\"\n\n[checks.\"ref.sample\"]\nmax = 3\n\n\
         [gate.tree]\n\"ref.sample.files\" = \"<= 1\"\n",
    )
    .unwrap();
    let out = cupel_in(dir.path(), &["check", "--only", "ref.sample", "--gate"]);
    assert_eq!(
        out.envelope()["checks"][0]["status"],
        "fail",
        "{}",
        out.stdout
    );
    let request: Value = serde_json::from_str(
        &fs::read_to_string(dir.path().join("sample.sh.request.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(request["config"], json!({"max": 3}));
}

#[test]
fn plugins_on_path_join_unless_no_path_plugins_is_given() {
    let dir = repository("");
    let bin = tempfile::tempdir().unwrap();
    plugin(bin.path(), "cupel-check-sample", &manifest());
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let cwd = dir.path().display().to_string();
    let describe = |extra: &[&str]| {
        let mut args = vec!["describe", "ref.sample", "--cwd", &cwd, "--format", "json"];
        args.extend(extra);
        run_with(cupel().env("PATH", &path), args)
    };
    assert_eq!(describe(&[]).code, 0, "{}", describe(&[]).stdout);
    let hidden = describe(&["--no-path-plugins"]);
    assert_eq!(hidden.code, 2);
    assert!(
        hidden.finding()["title"]
            .as_str()
            .unwrap()
            .contains("ref.sample")
    );
}

#[test]
fn a_plugin_that_cannot_describe_itself_is_a_usage_error_naming_the_fix() {
    let dir = repository("[plugins]\nbroken = \"./broken.sh\"\n");
    let broken = dir.path().join("broken.sh");
    fs::write(&broken, "#!/bin/sh\necho 'cannot start' >&2\nexit 3\n").unwrap();
    fs::set_permissions(&broken, fs::Permissions::from_mode(0o755)).unwrap();
    let out = cupel_in(dir.path(), &["check"]);
    assert_eq!(out.code, 2, "{}", out.stdout);
    let finding = out.finding();
    assert_eq!(finding["kind"], "cli.usage");
    let title = finding["title"].as_str().unwrap();
    assert!(
        title.contains("plugin `broken`") && title.contains("exited with code 3: cannot start"),
        "{title}"
    );
    let fix = finding["action"]["summary"].as_str().unwrap();
    assert!(fix.contains("[plugins] broken"), "{fix}");

    let bin = tempfile::tempdir().unwrap();
    fs::copy(&broken, bin.path().join("cupel-check-broken")).unwrap();
    let clean = repository("");
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let cwd = clean.path().display().to_string();
    let out = run_with(
        cupel().env("PATH", &path),
        ["check", "--cwd", &cwd, "--format", "json"],
    );
    assert_eq!(out.code, 2, "{}", out.stdout);
    let fix = out.finding()["action"]["summary"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(fix.contains("--no-path-plugins"), "{fix}");
}
