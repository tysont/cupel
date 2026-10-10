// ABOUTME: Golden-store tests in a temporary directory: comparison, readable diffs that name the
// ABOUTME: file and the update command, update mode, missing goldens, and name validation.
use std::fs;

use cupel_fixtures::{GoldenError, GoldenMode, Goldens, canonical_json};
use serde_json::{Value, json};

const UPDATE_COMMAND: &str = "UPDATE_GOLDENS=1 make test";

fn sample() -> Value {
    json!({"status": "pass", "exit_code": 0, "run": {"verb": "describe", "seed": "<seed>"}})
}

fn write(dir: &tempfile::TempDir, name: &str, value: &Value) {
    let path = dir.path().join(format!("{name}.json"));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, canonical_json(value)).unwrap();
}

#[test]
fn a_matching_golden_passes() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir, "cli/sample", &sample());

    let result = Goldens::new(dir.path(), GoldenMode::Compare).check("cli/sample", &sample());

    assert!(result.is_ok(), "{}", result.unwrap_err());
}

#[test]
fn a_changed_field_fails_with_a_unified_diff_naming_the_golden_and_the_update_command() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir, "cli/sample", &sample());
    let mut actual = sample();
    actual["status"] = json!("fail");
    let goldens = Goldens::new(dir.path(), GoldenMode::Compare);
    let path = goldens.path("cli/sample").unwrap();

    let error = goldens.check("cli/sample", &actual).unwrap_err();

    assert!(matches!(error, GoldenError::Mismatch { .. }), "{error:?}");
    let message = error.to_string();
    for expected in [
        format!("--- {}", path.display()),
        "+++ actual".to_owned(),
        "@@".to_owned(),
        r#"-  "status": "pass""#.to_owned(),
        r#"+  "status": "fail""#.to_owned(),
        UPDATE_COMMAND.to_owned(),
    ] {
        assert!(
            message.contains(&expected),
            "{expected:?} missing from:\n{message}"
        );
    }
    assert!(
        !message.contains(r#""exit_code""#),
        "the diff should show only nearby context, not the whole document:\n{message}"
    );
}

#[test]
fn a_missing_golden_fails_with_how_to_create_it_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let goldens = Goldens::new(dir.path(), GoldenMode::Compare);
    let path = goldens.path("cli/absent").unwrap();

    let error = goldens.check("cli/absent", &sample()).unwrap_err();

    assert!(matches!(error, GoldenError::Missing { .. }), "{error:?}");
    let message = error.to_string();
    assert!(message.contains(&path.display().to_string()), "{message}");
    assert!(message.contains(UPDATE_COMMAND), "{message}");
    assert!(message.contains(r#""verb": "describe""#), "{message}");
    assert!(!path.exists());
}

#[test]
fn update_mode_writes_a_missing_golden_canonically() {
    let dir = tempfile::tempdir().unwrap();
    let goldens = Goldens::new(dir.path(), GoldenMode::Update);

    goldens.check("static.complexity/rust", &sample()).unwrap();

    let written = fs::read_to_string(dir.path().join("static.complexity/rust.json")).unwrap();
    assert_eq!(
        written,
        "{\n  \"exit_code\": 0,\n  \"run\": {\n    \"seed\": \"<seed>\",\n    \"verb\": \"describe\"\n  },\n  \"status\": \"pass\"\n}\n"
    );
    Goldens::new(dir.path(), GoldenMode::Compare)
        .check("static.complexity/rust", &sample())
        .unwrap();
}

#[test]
fn update_mode_rewrites_a_golden_that_differs() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir, "cli/sample", &sample());
    let mut actual = sample();
    actual["status"] = json!("fail");

    Goldens::new(dir.path(), GoldenMode::Update)
        .check("cli/sample", &actual)
        .unwrap();

    Goldens::new(dir.path(), GoldenMode::Compare)
        .check("cli/sample", &actual)
        .unwrap();
}

#[test]
fn comparison_ignores_key_order_and_whitespace() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("cli")).unwrap();
    fs::write(
        dir.path().join("cli/sample.json"),
        r#"{"run":{"seed":"<seed>","verb":"describe"},"status":"pass","exit_code":0}"#,
    )
    .unwrap();

    let result = Goldens::new(dir.path(), GoldenMode::Compare).check("cli/sample", &sample());

    assert!(result.is_ok(), "{}", result.unwrap_err());
}

#[test]
fn a_golden_that_is_not_json_is_reported_with_its_path() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("cli")).unwrap();
    fs::write(dir.path().join("cli/sample.json"), "not json").unwrap();
    let goldens = Goldens::new(dir.path(), GoldenMode::Compare);

    let message = goldens
        .check("cli/sample", &sample())
        .unwrap_err()
        .to_string();

    let path = goldens.path("cli/sample").unwrap();
    assert!(message.contains(&path.display().to_string()), "{message}");
    assert!(message.contains(UPDATE_COMMAND), "{message}");
}

#[test]
fn names_must_be_relative_slash_separated_and_extensionless() {
    let goldens = Goldens::new("/goldens", GoldenMode::Compare);

    for bad in [
        "",
        "/abs",
        "../escape",
        "a/../b",
        "a//b",
        "a/",
        "cli/describe.json",
    ] {
        assert!(
            matches!(goldens.path(bad), Err(GoldenError::InvalidName(_))),
            "{bad:?} was accepted"
        );
    }
    assert_eq!(
        goldens.path("static.complexity/rust").unwrap(),
        std::path::Path::new("/goldens/static.complexity/rust.json")
    );
}

#[test]
fn only_update_goldens_equal_to_one_selects_update_mode() {
    assert_eq!(GoldenMode::parse(Some("1")), GoldenMode::Update);
    for other in [None, Some(""), Some("0"), Some("true")] {
        assert_eq!(GoldenMode::parse(other), GoldenMode::Compare, "{other:?}");
    }
}

#[test]
fn canonical_json_sorts_keys_at_every_depth_and_ends_with_a_newline() {
    let value = json!({"b": 1, "a": {"z": [{"y": 1, "x": 2}], "c": null}});

    assert_eq!(
        canonical_json(&value),
        "{\n  \"a\": {\n    \"c\": null,\n    \"z\": [\n      {\n        \"x\": 2,\n        \"y\": 1\n      }\n    ]\n  },\n  \"b\": 1\n}\n"
    );
}
