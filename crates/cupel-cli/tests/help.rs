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

/// Asserts `command`'s help page and its sub-verbs' pages show exactly their registered flags,
/// summaries, and sub-verbs; returns how many pages it checked.
fn assert_help_pages(command: &Value, global: &BTreeSet<String>) -> usize {
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
    let mut pages = 1;
    for sub in command["subcommands"].as_array().unwrap() {
        assert!(help.contains(sub["name"].as_str().unwrap().rsplit(' ').next().unwrap()));
        pages += assert_help_pages(sub, global);
    }
    pages
}

#[test]
fn every_help_page_mentions_exactly_the_registered_flags() {
    let all = run(["describe", "--all"]).envelope()["description"].clone();
    let global = registered(&all["cli"]["global_flags"]);

    let root = help_page(&[]);
    let mut expected = global.clone();
    expected.extend(registered(&all["cli"]["flags"]));
    assert_eq!(flags_in(&root), expected, "`cupel --help`:\n{root}");
    let summaries = std::iter::once(&all["cli"]).chain(all["cli"]["verbs"].as_array().unwrap());
    let missing: Vec<&str> = summaries
        .map(|described| described["summary"].as_str().unwrap())
        .filter(|summary| !root.contains(summary))
        .collect();
    assert_eq!(missing, Vec::<&str>::new());

    let pages: usize = all["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|command| assert_help_pages(command, &global))
        .sum();
    assert!(pages >= 19, "{pages} verb help pages checked");
}
