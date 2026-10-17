// ABOUTME: Property stubs from signatures (ADR-096 §4): a function's stub rendered from its language's
// ABOUTME: `[property_stubs]` table, where it goes, and how stubs merge into a file once each.
//!
//! # `[property_stubs]`
//!
//! ```toml
//! [property_stubs]
//! target = "test"                  # `test`: the nearest paired test file, else `file`; `source`: the function's own file
//! file = "test_{stem}.py"          # a new test file, relative to the source's directory
//! header = ["import {namespace}"]  # lines a file gets once, before its first stub
//! marker = "def test_{name}_property("  # text a file holds when it already has the stub
//! stub = "…"                       # the stub; placeholders below
//! generator = "st.from_type(object)"  # for a parameter `types` does not map; absent: no stub
//! borrow = "&"                     # a parameter type with this prefix is generated as the rest and passed with it
//! import_extensions = [".js"]      # source extensions `{import}` keeps; others are dropped
//!
//! [property_stubs.types]           # a parameter's normalized type → its generator
//! i64 = "any::<i64>()"
//!
//! [[property_stubs.variants]]      # the first variant whose `when` the target (or, new, the source) contains
//! when = "require("
//! header = ["const fc = require(\"fast-check\");"]
//! ```
//!
//! Placeholders: `{name}`, `{namespace}`, `{stem}` and `{ext}` (the source's file stem and
//! extension, with its dot), `{subject}` (an
//! identifier for the imported module, `<stem>Subject`), `{import}` (the source, relative to the
//! target, starting `./`), `{generators}` and `{bindings}` (`a0 in <generator>`, one per
//! parameter), `{arguments}` (`a0, a1`), and `{passed}` (the arguments as the function takes
//! them).
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use crate::language::Language;

/// `[property_stubs]`: how a language's property stubs look and where they go.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct StubRules {
    pub target: StubTarget,
    pub file: String,
    pub header: Vec<String>,
    pub marker: String,
    pub stub: String,
    pub generator: Option<String>,
    pub borrow: Option<String>,
    pub import_extensions: Vec<String>,
    pub types: std::collections::BTreeMap<String, String>,
    pub variants: Vec<StubVariant>,
}

/// Where a function's stub goes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StubTarget {
    /// The nearest test file paired with the source, else a new `file` beside it.
    #[default]
    Test,
    /// The function's own file.
    Source,
}

/// A header for files that contain `when`, such as `require(` in a `CommonJS` module.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StubVariant {
    pub when: String,
    pub header: Vec<String>,
}

/// A function to stub: what `property.missing` says about it.
#[derive(Debug, Clone, Copy)]
pub struct Subject<'a> {
    /// Its file, relative to the repository root.
    pub source: &'a str,
    pub name: &'a str,
    pub namespace: &'a str,
    pub parameters: u32,
    /// Its normalized signature (`ApiSurface`), when known.
    pub signature: Option<&'a str>,
}

/// One rendered stub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stub {
    /// The file it goes in, relative to the repository root.
    pub target: String,
    /// Lines its file needs once, before the first stub.
    pub header: Vec<String>,
    /// Text a file holds when it already has this stub.
    pub marker: String,
    pub text: String,
}

/// The stub of `subject` in `language`, beside `nearest_test` (the first test file paired with
/// its source) when there is one; `read` gives a file's text by repository path. `None` when the
/// language has no stub table or cannot generate a parameter.
pub fn property_stub(
    language: &dyn Language,
    subject: &Subject<'_>,
    nearest_test: Option<&str>,
    read: &dyn Fn(&str) -> Option<String>,
) -> Option<Stub> {
    let rules = language.spec().tables().property_stubs.as_ref()?;
    let target = match (rules.target, nearest_test) {
        (StubTarget::Source, _) => subject.source.to_owned(),
        (StubTarget::Test, Some(test)) => test.to_owned(),
        (StubTarget::Test, None) => beside(subject.source, &fill(&rules.file, &names(subject, ""))),
    };
    let parameters = parameters(subject)
        .iter()
        .map(|kind| parameter(rules, kind))
        .collect::<Option<Vec<_>>>()?;
    let import = relative(&target, subject.source, &rules.import_extensions);
    let mut values = names(subject, &import);
    values.extend(arguments(&parameters));
    let shown = read(&target)
        .or_else(|| read(subject.source))
        .unwrap_or_default();
    let header = rules
        .variants
        .iter()
        .find(|variant| shown.contains(&variant.when))
        .map_or(&rules.header, |variant| &variant.header);
    Some(Stub {
        target,
        header: header.iter().map(|line| fill(line, &values)).collect(),
        marker: fill(&rules.marker, &values),
        text: fill(&rules.stub, &values),
    })
}

/// `existing` (a file's text; `None` for a new file) with every stub of `stubs` it lacks, and
/// each header line those stubs need that it lacks before them; `None` when nothing is new.
pub fn merged_stubs(existing: Option<&str>, stubs: &[&Stub]) -> Option<String> {
    let mut text = existing.unwrap_or_default().to_owned();
    let mut fresh: Vec<&Stub> = Vec::new();
    for stub in stubs {
        if !text.contains(&stub.marker) && !fresh.iter().any(|seen| seen.marker == stub.marker) {
            fresh.push(stub);
        }
    }
    if fresh.is_empty() {
        return None;
    }
    let mut header: Vec<&str> = Vec::new();
    for line in fresh.iter().flat_map(|stub| &stub.header) {
        let present = text.lines().any(|have| have.trim() == line.trim());
        if !present && !header.contains(&line.as_str()) {
            header.push(line);
        }
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    if !header.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&header.join("\n"));
        text.push('\n');
    }
    for stub in fresh {
        text.push_str(&stub.text);
    }
    Some(text)
}

/// A parameter: the generator that makes it and how the function takes it.
struct Parameter {
    generator: String,
    borrowed: Option<String>,
}

/// The generator of a parameter of normalized type `kind` (empty when untyped).
fn parameter(rules: &StubRules, kind: &str) -> Option<Parameter> {
    let (inner, borrowed) = match rules.borrow.as_deref() {
        Some(prefix) if kind.starts_with(prefix) => {
            (&kind[prefix.len()..], Some(prefix.to_owned()))
        }
        _ => (kind, None),
    };
    let generator = rules
        .types
        .get(inner.trim())
        .or(rules.generator.as_ref())?
        .clone();
    Some(Parameter {
        generator,
        borrowed,
    })
}

/// The types of the subject's parameters, from its signature (`(_: i64, _: &str) -> i64`,
/// `(_, _)`), else as many untyped ones as it takes.
fn parameters(subject: &Subject<'_>) -> Vec<String> {
    let listed = subject.signature.and_then(|signature| {
        let start = signature.find('(')?;
        let inner = &signature[start + 1..closing(signature, start)?];
        Some(split_top_level(inner))
    });
    listed.unwrap_or_else(|| vec![String::new(); subject.parameters as usize])
}

/// The index of the parenthesis closing the one at `open`.
fn closing(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (at, c) in text.char_indices().skip_while(|(at, _)| *at < open) {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(at);
                }
            }
            _ => {}
        }
    }
    None
}

/// `inner`'s comma-separated parts at nesting depth zero, each without its `_` name: the type
/// after `: `, or empty when untyped.
fn split_top_level(inner: &str) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut depth = 0i32;
    for c in inner.chars() {
        match c {
            '(' | '<' | '[' | '{' => depth += 1,
            ')' | '>' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(String::new());
                continue;
            }
            _ => {}
        }
        if let Some(last) = parts.last_mut() {
            last.push(c);
        }
    }
    parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.split_once(':')
                .map_or("", |(_, kind)| kind)
                .trim()
                .to_owned()
        })
        .collect()
}

/// `{generators}`, `{bindings}`, `{arguments}`, and `{passed}` for `parameters`.
fn arguments(parameters: &[Parameter]) -> Vec<(&'static str, String)> {
    let join = |part: &dyn Fn(usize, &Parameter) -> String| {
        parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| part(index, parameter))
            .collect::<Vec<_>>()
            .join(", ")
    };
    vec![
        ("generators", join(&|_, p| p.generator.clone())),
        ("bindings", join(&|i, p| format!("a{i} in {}", p.generator))),
        ("arguments", join(&|i, _| format!("a{i}"))),
        (
            "passed",
            join(&|i, p| format!("{}a{i}", p.borrowed.as_deref().unwrap_or_default())),
        ),
    ]
}

/// `{name}`, `{namespace}`, `{stem}`, `{subject}`, and `{import}` for `subject`.
fn names(subject: &Subject<'_>, import: &str) -> Vec<(&'static str, String)> {
    let stem = Path::new(subject.source)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let identifier: String = stem
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    vec![
        ("name", subject.name.to_owned()),
        ("namespace", subject.namespace.to_owned()),
        ("subject", format!("{identifier}Subject")),
        ("stem", stem),
        ("ext", extension(subject.source)),
        ("import", import.to_owned()),
    ]
}

/// `path`'s extension with its dot, such as `.js`; empty without one.
fn extension(path: &str) -> String {
    Path::new(path)
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy()))
        .unwrap_or_default()
}

/// `template` with each `{key}` of `values` replaced.
fn fill(template: &str, values: &[(&'static str, String)]) -> String {
    values
        .iter()
        .fold(template.to_owned(), |text, (key, value)| {
            text.replace(&format!("{{{key}}}"), value)
        })
}

/// `file` in the directory of `source`.
fn beside(source: &str, file: &str) -> String {
    let dir = Path::new(source).parent().unwrap_or(Path::new(""));
    slashed(&dir.join(file))
}

/// `source` relative to the directory of `target`, starting `./` or `../`, with its extension
/// when it is one of `extensions`.
fn relative(target: &str, source: &str, extensions: &[String]) -> String {
    let from: Vec<Component<'_>> = Path::new(target)
        .parent()
        .map(|dir| dir.components().collect())
        .unwrap_or_default();
    let source = if extensions.contains(&extension(source)) {
        PathBuf::from(source)
    } else {
        Path::new(source).with_extension("")
    };
    let to: Vec<Component<'_>> = source.components().collect();
    let shared = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut path = PathBuf::new();
    for _ in shared..from.len() {
        path.push("..");
    }
    for part in &to[shared..] {
        path.push(part);
    }
    let path = slashed(&path);
    if path.starts_with("..") {
        path
    } else {
        format!("./{path}")
    }
}

fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
