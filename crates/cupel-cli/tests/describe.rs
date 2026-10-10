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
    let outcome = run(["describe", "check"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["description"]["subject"], json!("command"));
    assert_eq!(envelope["description"]["name"], json!("check"));
    assert_eq!(envelope["description"]["pending_step"], json!("P2.1"));
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
    for (args, step) in [
        (vec!["describe", "matrix"], "P1.1"),
        (vec!["describe", "languages"], "P1.1"),
        (vec!["describe", "language", "rust"], "P1.1"),
        (vec!["describe", "fact", "symbols"], "P1.1"),
        (vec!["describe", "finding", "mutant.survived"], "P2.1"),
        (vec!["describe", "static.complexity"], "P2.1"),
        (vec!["describe", "--agent"], "P8.3"),
    ] {
        let outcome = run(&args);
        assert_eq!(outcome.code, 2, "{args:?}");
        let finding = outcome.finding();
        assert_eq!(finding["kind"], json!("cli.not-implemented"), "{args:?}");
        let detail = finding["detail"].as_str().unwrap();
        assert!(detail.contains(step), "{args:?}: {detail}");
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
