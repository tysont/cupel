// ABOUTME: Rust schemata compiled for real (ADR-082): one rustc build of the rewritten program, run once per mutant
// ABOUTME: with `CUPEL_MUTANT`, prints exactly what each mutant built as its own file copy prints.
use std::path::Path;
use std::process::Command;

use cupel_core::syntax::Span;
use cupel_mutate::mutant::{apply, span};
use cupel_mutate::schemata::{Gate, rewriter};

/// A program whose output shows evaluation order, short-circuiting, borrows, and values.
const PROGRAM: &str = r#"use std::cell::Cell;

thread_local!(static CALLS: Cell<u32> = const { Cell::new(0) });

fn seen(value: bool) -> bool {
    CALLS.with(|calls| calls.set(calls.get() + 1));
    value
}

fn discount(total: i64, member: bool) -> i64 {
    if total >= 100 && seen(member) {
        return total - 10;
    }
    total * 1
}

fn longest(a: &String, b: &String) -> usize {
    let mut n = 0;
    n += a.len();
    if a >= b { n } else { n + b.len() }
}

fn classify(n: i64) -> &'static str {
    match n {
        0 => "zero",
        x if x > 1 => "many",
        _ => "one",
    }
}

fn spread(values: &[i64]) -> i64 {
    let total: i64 = values.iter().map(|v| v
        * 2).sum();
    let Some(first) = values.first().map(|f| f + 0) else {
        return -1;
    };
    total - first
}

mod inner {
    pub fn flag(a: bool, b: bool) -> bool {
        a || !b
    }
}

fn main() {
    let (s, t) = (String::from("pear"), String::from("apple"));
    let out = (
        discount(150, true),
        discount(150, false),
        discount(100, true),
        discount(40, true),
        longest(&s, &t),
        longest(&t, &s),
        classify(0),
        classify(5),
        classify(1),
        spread(&[3, 4]),
        inner::flag(false, true),
        CALLS.with(Cell::get),
    );
    println!("{out:?} {s} {t}");
}
"#;

/// Every mutant of the program: (context, from, to).
const MUTANTS: &[(&str, &str, &str)] = &[
    ("total >= 100", ">=", ">"),
    ("&& seen", "&&", "||"),
    ("total - 10", "-", "+"),
    ("total * 1", "*", "/"),
    ("total * 1", "1", "0"),
    ("calls.get() + 1", "+", "-"),
    ("n += a", "+=", "-="),
    ("a >= b", ">=", ">"),
    ("n + b.len()", "+", "-"),
    ("x if x > 1", ">", ">="),
    ("x > 1", "1", "0"),
    ("v\n        * 2", "*", "/"),
    ("f + 0", "+", "-"),
    ("total - first", "-", "+"),
    ("a || !b", "||", "&&"),
    ("value\n}", "value", "!(value)"),
];

/// The span of `from` inside the first `context` of `PROGRAM`.
fn span_of(context: &str, from: &str) -> Span {
    let at = PROGRAM.find(context).expect("context in the program")
        + context.find(from).expect("from in its context");
    span(PROGRAM, at, at + from.len())
}

/// Builds `source` with rustc into `dir/name`; the binary, or `None` when it does not build.
fn build(dir: &Path, name: &str, source: &str) -> Option<std::path::PathBuf> {
    let file = dir.join(format!("{name}.rs"));
    std::fs::write(&file, source).unwrap();
    let binary = dir.join(name);
    let status = Command::new("rustc")
        .args(["--edition", "2024", "-A", "warnings", "-o"])
        .arg(&binary)
        .arg(&file)
        .status()
        .unwrap();
    status.success().then_some(binary)
}

/// What `binary` prints with `CUPEL_MUTANT=id`.
fn run(binary: &Path, id: u32) -> String {
    let output = Command::new(binary)
        .env("CUPEL_MUTANT", id.to_string())
        .output()
        .unwrap();
    format!(
        "{} {}",
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).trim()
    )
}

#[test]
fn every_gated_mutant_behaves_as_its_file_copy() {
    if cupel_core::find_program("rustc").is_none() {
        eprintln!("SKIPPED: rustc not installed; the schemata compile test was not run");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let gates: Vec<Gate<'_>> = (1..)
        .zip(MUTANTS)
        .map(|(id, (context, from, to))| Gate {
            id,
            span: span_of(context, from),
            from,
            to,
        })
        .collect();
    let rewritten = rewriter("rust").unwrap().rewrite(PROGRAM, &gates);
    assert_eq!(rewritten.excluded, std::collections::BTreeMap::new());
    let source = rewritten.source.unwrap();
    assert_eq!(source.lines().count(), PROGRAM.lines().count() + 11);
    let schemata = build(dir.path(), "schemata", &source).expect("the schemata build");
    let original = build(dir.path(), "original", PROGRAM).unwrap();
    assert_eq!(run(&schemata, 0), run(&original, 0));
    let gated: Vec<String> = gates.iter().map(|gate| run(&schemata, gate.id)).collect();
    let copies: Vec<String> = gates
        .iter()
        .map(|gate| {
            let mutated = apply(PROGRAM, &gate.span, gate.from, gate.to).unwrap();
            let copy = build(dir.path(), &format!("copy{}", gate.id), &mutated)
                .unwrap_or_else(|| panic!("mutant {} builds as a file copy", gate.id));
            run(&copy, 0)
        })
        .collect();
    assert_eq!(gated, copies);
}
