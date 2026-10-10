// ABOUTME: Proves clap's `--help` pages and `describe` come from the same registry: every flag a
// ABOUTME: help page mentions is registered for that command, and every registered flag is mentioned.
mod support;

use std::collections::BTreeSet;

use serde_json::Value;
use support::run;

/// Every `--flag` token on a help page.
fn flags_in(help: &str) -> BTreeSet<String> {
    let mut flags = BTreeSet::new();
    for token in help.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')) {
        if token.len() > 2
            && token.starts_with("--")
            && token[2..].starts_with(|c: char| c.is_ascii_lowercase())
        {
            flags.insert(token.to_owned());
        }
    }
    flags
}

fn registered(arguments: &Value) -> BTreeSet<String> {
    arguments
        .as_array()
        .unwrap()
        .iter()
        .filter(|arg| arg["positional"] == false)
        .map(|arg| arg["name"].as_str().unwrap().to_owned())
        .collect()
}

fn help_page(words: &[&str]) -> String {
    let mut args: Vec<&str> = words.to_vec();
    args.push("--help");
    let outcome = run(&args);
    assert_eq!(outcome.code, 0, "{args:?}: {}", outcome.stderr);
    outcome.stdout
}

fn check_command(command: &Value, global: &BTreeSet<String>, pages: &mut usize) {
    let name = command["name"].as_str().unwrap();
    let words: Vec<&str> = name.split(' ').collect();
    let help = help_page(&words);
    let mut expected = registered(&command["arguments"]);
    expected.extend(global.iter().cloned());
    assert_eq!(flags_in(&help), expected, "`cupel {name} --help`:\n{help}");
    assert!(
        help.contains(command["summary"].as_str().unwrap()),
        "{name} summary"
    );
    for argument in command["arguments"].as_array().unwrap() {
        let summary = argument["summary"].as_str().unwrap();
        assert!(help.contains(summary), "`{name}` help lacks: {summary}");
    }
    *pages += 1;
    for sub in command["subcommands"].as_array().unwrap() {
        assert!(help.contains(sub["name"].as_str().unwrap().rsplit(' ').next().unwrap()));
        check_command(sub, global, pages);
    }
}

#[test]
fn every_help_page_mentions_exactly_the_registered_flags() {
    let all = run(["describe", "--all"]).envelope()["description"].clone();
    let global = registered(&all["cli"]["global_flags"]);

    let root = help_page(&[]);
    let mut expected = global.clone();
    expected.extend(registered(&all["cli"]["flags"]));
    assert_eq!(flags_in(&root), expected, "`cupel --help`:\n{root}");
    assert!(root.contains(all["cli"]["summary"].as_str().unwrap()));
    for verb in all["cli"]["verbs"].as_array().unwrap() {
        assert!(root.contains(verb["summary"].as_str().unwrap()), "{verb}");
    }

    let mut pages = 1;
    for command in all["commands"].as_array().unwrap() {
        check_command(command, &global, &mut pages);
    }
    assert!(pages >= 20, "{pages} help pages checked");
}
