// ABOUTME: Integration tests for `cupel describe` and `<verb> --describe`: what each subject returns,
// ABOUTME: snapshots of the CLI and verb descriptions, and stability on every described entity.
mod support;

use cupel_core::{Verb, schema};
use serde_json::{Value, json};
use support::{cli_finding_id, redacted, run};

fn verb_names() -> Vec<String> {
    Verb::ALL
        .iter()
        .map(|verb| {
            serde_json::to_value(verb)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

#[test]
fn describe_without_a_subject_describes_the_cli() {
    let outcome = run(["describe"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    assert_eq!(outcome.stderr, "");
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("pass"));
    assert_eq!(envelope["run"]["verb"], json!("describe"));
    assert_eq!(envelope["description"]["subject"], json!("cli"));
    insta::assert_snapshot!(serde_json::to_string_pretty(&redacted(envelope)).unwrap());
}

#[test]
fn describe_cli_lists_every_verb_exit_code_and_schema() {
    let description = run(["describe"]).envelope()["description"].clone();
    let verbs: Vec<&str> = description["verbs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|verb| verb["name"].as_str().unwrap())
        .collect();
    assert_eq!(verbs, verb_names());
    let codes: Vec<u64> = description["exit_codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|code| code["code"].as_u64().unwrap())
        .collect();
    assert_eq!(codes, [0, 1, 2, 3, 4, 5, 6]);
    let schemas: Vec<(String, String)> = description["schemas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["name"].as_str().unwrap().to_owned(),
                s["url"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let published: Vec<(String, String)> = schema::PUBLISHED
        .iter()
        .map(|(name, _)| ((*name).to_owned(), schema::url(name)))
        .collect();
    assert_eq!(schemas, published);
    assert_eq!(
        description["config_precedence"],
        json!(["flags", "env", "profile", "cupel.toml", "defaults"])
    );
}

#[test]
fn describe_verb_describes_its_arguments_and_pending_step() {
    let outcome = run(["describe", "survey"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["description"]["subject"], json!("command"));
    assert_eq!(envelope["description"]["name"], json!("survey"));
    assert_eq!(envelope["description"]["pending_step"], json!("P2.9"));
    insta::assert_snapshot!(serde_json::to_string_pretty(&redacted(envelope)).unwrap());
}

#[test]
fn verb_describe_flag_matches_describe_verb() {
    for verb in verb_names() {
        let by_flag = run([verb.as_str(), "--describe"]);
        assert_eq!(by_flag.code, 0, "{verb} --describe: {}", by_flag.stdout);
        let by_verb = run(["describe", verb.as_str()]);
        assert_eq!(
            by_flag.envelope()["description"],
            by_verb.envelope()["description"],
            "{verb}"
        );
    }
}

#[test]
fn describe_flag_works_even_when_required_arguments_are_missing() {
    let outcome = run(["suppress", "--describe"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    assert_eq!(outcome.envelope()["description"]["name"], json!("suppress"));
}

#[test]
fn sub_verbs_are_described_by_their_full_name() {
    let by_flag = run(["baseline", "set", "--describe"]);
    assert_eq!(by_flag.code, 0, "{}", by_flag.stdout);
    let by_verb = run(["describe", "baseline", "set"]);
    assert_eq!(by_verb.code, 0, "{}", by_verb.stdout);
    for outcome in [by_flag, by_verb] {
        assert_eq!(
            outcome.envelope()["description"]["name"],
            json!("baseline set")
        );
    }
}

#[test]
fn describe_flag_without_a_verb_describes_the_cli() {
    let outcome = run(["--describe"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["description"]["subject"], json!("cli"));
    assert_eq!(envelope["run"]["verb"], Value::Null);
}

#[test]
fn describe_schema_returns_each_published_schema() {
    for (name, generate) in schema::PUBLISHED {
        let outcome = run(["describe", "schema", name]);
        assert_eq!(outcome.code, 0, "{name}: {}", outcome.stdout);
        let description = outcome.envelope()["description"].clone();
        assert_eq!(description["subject"], json!("schema"));
        assert_eq!(description["name"], json!(name));
        assert_eq!(description["url"], json!(schema::url(name)));
        assert_eq!(&description["schema"], generate().as_value());
    }
}

#[test]
fn describe_schema_with_an_unknown_name_names_the_valid_ones() {
    for args in [
        vec!["describe", "schema", "nope"],
        vec!["describe", "schema"],
    ] {
        let outcome = run(&args);
        assert_eq!(outcome.code, 2, "{args:?}");
        let finding = outcome.finding();
        assert_eq!(finding["kind"], json!("cli.usage"));
        assert_eq!(
            finding["id"],
            json!(cli_finding_id("cli.usage", "describe schema"))
        );
        assert_eq!(
            finding["evidence"]["valid"],
            json!(["run", "finding", "events", "config", "plugin"])
        );
        let fix = finding["action"]["summary"].as_str().unwrap();
        assert!(
            fix.contains("run, finding, events, config, plugin"),
            "{fix}"
        );
    }
}

#[test]
fn describe_unknown_subject_is_a_usage_error_listing_subjects() {
    let outcome = run(["describe", "frobnicate"]);
    assert_eq!(outcome.code, 2);
    let finding = outcome.finding();
    assert_eq!(finding["kind"], json!("cli.usage"));
    let valid = finding["evidence"]["valid"].as_array().unwrap();
    for subject in ["check", "schema", "matrix"] {
        assert!(valid.contains(&json!(subject)), "{subject} in {valid:?}");
    }
}

#[test]
fn describe_subjects_from_later_steps_are_not_implemented_yet() {
    let outcome = run(["describe", "--agent"]);
    assert_eq!(outcome.code, 2);
    let finding = outcome.finding();
    assert_eq!(finding["kind"], json!("cli.not-implemented"));
    let detail = finding["detail"].as_str().unwrap();
    assert!(detail.contains("P8.3"), "{detail}");
}

#[test]
fn describe_check_shows_its_manifest_and_support_per_language() {
    let outcome = run(["describe", "static.complexity"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let description = outcome.envelope()["description"].clone();
    assert_eq!(description["subject"], json!("check"));
    assert_eq!(description["manifest"]["id"], json!("static.complexity"));
    assert_eq!(
        description["manifest"]["needs"],
        json!(["symbols", "decisions", "norm-tree"])
    );
    let languages = description["languages"].as_array().unwrap();
    assert_eq!(languages.len(), 5);
    assert!(
        languages
            .iter()
            .all(|entry| entry["support"]["status"] == json!("supported"))
    );
}

#[test]
fn describe_finding_shows_its_check_action_and_evidence_schema() {
    let outcome = run(["describe", "finding", "complexity.high"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let description = outcome.envelope()["description"].clone();
    assert_eq!(description["subject"], json!("finding"));
    assert_eq!(description["check"], json!("static.complexity"));
    assert_eq!(description["action"], json!("reduce_complexity"));
    assert_eq!(
        description["evidence"]["required"],
        json!(["cyclomatic", "cognitive", "cyclomatic_max", "cognitive_max"])
    );
}

#[test]
fn unknown_checks_and_finding_kinds_are_usage_errors_listing_the_known_ones() {
    for (args, valid) in [
        (vec!["describe", "static.complexty"], "static.complexity"),
        (
            vec!["describe", "finding", "mutant.survived"],
            "complexity.high",
        ),
    ] {
        let outcome = run(&args);
        assert_eq!(outcome.code, 2, "{args:?}");
        let finding = outcome.finding();
        assert_eq!(finding["kind"], json!("cli.usage"), "{args:?}");
        assert!(
            finding["evidence"]["valid"]
                .as_array()
                .unwrap()
                .contains(&json!(valid)),
            "{args:?}"
        );
    }
}

#[test]
fn describe_all_is_one_document_with_everything() {
    let outcome = run(["describe", "--all"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let description = outcome.envelope()["description"].clone();
    assert_eq!(description["subject"], json!("all"));
    assert_eq!(
        description["cli"],
        run(["describe"]).envelope()["description"]
            .as_object()
            .map(|cli| {
                let mut cli = cli.clone();
                cli.remove("subject");
                Value::Object(cli)
            })
            .unwrap()
    );
    let commands: Vec<&str> = description["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|command| command["name"].as_str().unwrap())
        .collect();
    assert_eq!(commands, verb_names());
    for command in description["commands"].as_array().unwrap() {
        let name = command["name"].as_str().unwrap();
        let mut by_verb = run(["describe", name]).envelope()["description"].clone();
        by_verb.as_object_mut().unwrap().remove("subject");
        assert_eq!(command, &by_verb, "{name}");
    }
    let schemas = description["schemas"].as_array().unwrap();
    assert_eq!(schemas.len(), schema::PUBLISHED.len());
    for (described, (name, generate)) in schemas.iter().zip(schema::PUBLISHED) {
        assert_eq!(described["name"], json!(name));
        assert_eq!(&described["schema"], generate().as_value());
    }
}

/// Every object `describe --all` names — CLI, commands, arguments, exit codes, schemas — states
/// its stability.
#[test]
fn every_described_entity_carries_stability() {
    fn walk(value: &Value, path: &str, count: &mut usize) {
        match value {
            Value::Object(object) => {
                if object.contains_key("name") {
                    let stability = object
                        .get("stability")
                        .unwrap_or_else(|| panic!("{path} has no stability"));
                    assert!(
                        stability == "stable" || stability == "experimental",
                        "{path}: {stability}"
                    );
                    *count += 1;
                }
                // A schema body is data being described, not a described entity.
                for (key, child) in object.iter().filter(|(key, _)| *key != "schema") {
                    walk(child, &format!("{path}.{key}"), count);
                }
            }
            Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    walk(child, &format!("{path}[{index}]"), count);
                }
            }
            _ => {}
        }
    }
    let description = run(["describe", "--all"]).envelope()["description"].clone();
    let mut count = 0;
    walk(&description, "$", &mut count);
    assert!(count > 100, "only {count} described entities");
}

#[test]
fn exit_codes_are_stable_and_everything_else_is_experimental() {
    let description = run(["describe"]).envelope()["description"].clone();
    for code in description["exit_codes"].as_array().unwrap() {
        assert_eq!(code["stability"], json!("stable"));
    }
    assert_eq!(description["stability"], json!("experimental"));
}

#[test]
fn describe_languages_lists_every_language_with_its_family_and_extensions() {
    let outcome = run(["describe", "languages"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let description = outcome.envelope()["description"].clone();
    assert_eq!(description["subject"], json!("languages"));
    let languages = description["languages"].as_array().unwrap();
    let ids: Vec<&str> = languages
        .iter()
        .map(|language| language["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["go", "javascript", "python", "rust", "typescript"]);
    let typescript = &languages[4];
    assert_eq!(typescript["family"], json!("ecma"));
    assert_eq!(
        typescript["extensions"],
        json!([".ts", ".mts", ".cts", ".tsx"])
    );
    assert_eq!(typescript["stability"], json!("experimental"));
    assert!(typescript["facts"].is_array());
}

#[test]
fn describe_language_shows_identity_facts_and_check_support() {
    let outcome = run(["describe", "language", "rust"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let description = outcome.envelope()["description"].clone();
    assert_eq!(description["subject"], json!("language"));
    assert_eq!(description["id"], json!("rust"));
    assert_eq!(description["family"], json!("rust"));
    assert_eq!(description["extensions"], json!([".rs"]));
    assert!(description["facts"].is_array());
    assert_eq!(
        description["checks"],
        json!([
            {"check": "static.clones", "support": {"status": "supported"}},
            {"check": "static.complexity", "support": {"status": "supported"}}
        ])
    );
}

#[test]
fn describe_language_shows_its_toolchain_row_in_default_order() {
    let description = run(["describe", "language", "python"]).envelope()["description"].clone();
    let toolchain = &description["toolchain"];
    let ids = |list: &str, key: &str| -> Vec<String> {
        toolchain[list]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry[key].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(ids("launchers", "tool"), ["uv", "poetry", "pip"]);
    assert_eq!(ids("runners", "producer"), ["tool.pytest", "tool.unittest"]);
    assert_eq!(
        ids("coverage", "producer"),
        ["tool.pytest-cov", "tool.coverage-py"]
    );
    assert_eq!(toolchain["coverage"][0]["format"], json!("lcov"));
    assert_eq!(
        toolchain["coverage"][0]["commands"],
        json!([{"runner": "tool.pytest", "command": "<m> pytest --cov --cov-report=lcov:{out}"}])
    );
    assert_eq!(ids("bench", "producer"), ["tool.pytest-benchmark"]);
    assert_eq!(toolchain["property"], json!(["hypothesis"]));
    assert_eq!(toolchain["fuzz"], json!(["atheris"]));
}

#[test]
fn describe_language_with_an_unknown_id_is_a_usage_error_listing_languages() {
    let outcome = run(["describe", "language", "cobol"]);
    assert_eq!(outcome.code, 2);
    let finding = outcome.finding();
    assert_eq!(finding["kind"], json!("cli.usage"));
    assert_eq!(
        finding["evidence"]["valid"],
        json!(["go", "javascript", "python", "rust", "typescript"])
    );
    assert_eq!(finding["evidence"]["value"], json!("cobol"));
}

#[test]
fn describe_fact_shows_version_schema_and_producers_for_every_language() {
    let outcome = run(["describe", "fact", "norm-tree"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let description = outcome.envelope()["description"].clone();
    assert_eq!(description["subject"], json!("fact"));
    assert_eq!(description["kind"], json!("norm-tree"));
    assert_eq!(description["syntactic"], json!(true));
    assert!(description["version"].as_u64().is_some());
    assert_eq!(
        description["schema"]["$id"],
        json!("https://cupel.dev/schema/v1/fact/norm-tree.json")
    );
    let langs: Vec<&str> = description["producers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["lang"].as_str().unwrap())
        .collect();
    assert_eq!(langs, ["go", "javascript", "python", "rust", "typescript"]);
    assert_eq!(
        description["consumers"],
        json!(["static.complexity", "static.clones"])
    );
}

#[test]
fn describe_fact_without_a_payload_type_yet_has_no_version_or_schema() {
    let description = run(["describe", "fact", "coverage-map"]).envelope()["description"].clone();
    assert_eq!(description["syntactic"], json!(false));
    assert_eq!(description["version"], Value::Null);
    assert_eq!(description["schema"], Value::Null);
}

#[test]
fn describe_fact_with_an_unknown_kind_is_a_usage_error_listing_kinds() {
    let outcome = run(["describe", "fact", "vibes"]);
    assert_eq!(outcome.code, 2);
    let finding = outcome.finding();
    let valid = finding["evidence"]["valid"].as_array().unwrap();
    assert_eq!(valid.len(), cupel_core::FactKind::ALL.len());
    assert_eq!(valid[0], json!("symbols"));
}

#[test]
fn describe_matrix_has_every_language_and_a_row_per_check() {
    let outcome = run(["describe", "matrix"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let description = outcome.envelope()["description"].clone();
    assert_eq!(description["subject"], json!("matrix"));
    assert_eq!(
        description["languages"],
        json!(["go", "javascript", "python", "rust", "typescript"])
    );
    assert_eq!(
        description["checks"],
        json!(["static.complexity", "static.clones"])
    );
    let cells = description["cells"].as_array().unwrap();
    assert_eq!(cells.len(), 10);
    assert!(
        cells
            .iter()
            .all(|cell| cell["support"]["status"] == json!("supported"))
    );
}

#[test]
fn describe_matrix_renders_as_text() {
    let outcome = run(["describe", "matrix", "--format", "text"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    insta::assert_snapshot!(support::redacted_text(&outcome.stdout));
}

#[test]
fn describe_language_renders_as_text() {
    let outcome = run(["describe", "language", "typescript", "--format", "text"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    insta::assert_snapshot!(support::redacted_text(&outcome.stdout));
}

#[test]
fn describe_language_text_lists_the_toolchain_with_rules_and_commands() {
    let outcome = run(["describe", "language", "rust", "--format", "text"]);
    let text = support::redacted_text(&outcome.stdout);
    let toolchain = text
        .split_once("\ntoolchain\n")
        .expect("a toolchain section")
        .1;
    for line in [
        "  runners",
        "    tool.cargo-nextest  when file `.config/nextest.toml` exists",
        "      cargo nextest run --workspace",
        "    tool.cargo-test  otherwise",
        "    tool.cargo-llvm-cov  lcov · always",
        "  race      miri, loom",
    ] {
        assert!(
            toolchain.lines().any(|l| l == line),
            "{line:?} in\n{toolchain}"
        );
    }
}

#[test]
fn describe_subjects_that_need_an_argument_say_which() {
    for (args, subject) in [
        (vec!["describe", "language"], "describe language"),
        (vec!["describe", "fact"], "describe fact"),
    ] {
        let outcome = run(&args);
        assert_eq!(outcome.code, 2, "{args:?}");
        let finding = outcome.finding();
        assert_eq!(finding["kind"], json!("cli.usage"), "{args:?}");
        assert!(
            finding["title"].as_str().unwrap().contains(subject),
            "{args:?}: {}",
            finding["title"]
        );
    }
}
