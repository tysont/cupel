// ABOUTME: A Clojure project's API is what its published source paths hold (ADR-215): `deps.edn`'s top-level
// ABOUTME: `:paths` or `project.clj`'s `:source-paths`, `src` by default; a var in any other file is internal.
//!
//! - **The project** of a file is the nearest directory above it with a `deps.edn` or a
//!   `project.clj`. A file under neither has no project, and its public vars stay API.
//! - **Published paths** are the strings of the vector the top-level key names: `:paths` of
//!   `deps.edn`'s map and `:source-paths` of `project.clj`'s `defproject`, relative to the
//!   project; `["src"]` when the key is absent, as tools.deps and Leiningen default. A directory
//!   with both manifests publishes what either does. Alias and profile paths (`:extra-paths`,
//!   a profile's `:source-paths`), `bb.edn`'s `:paths`, and alias keywords inside `:paths` are
//!   not published.
use std::collections::HashMap;

use cupel_core::syntax::ApiSurface;

use crate::imports::{Imports, join};

/// Each manifest a project directory may hold, with its top-level key naming published paths.
const MANIFESTS: [(&str, &str); 2] = [("deps.edn", ":paths"), ("project.clj", ":source-paths")];

/// The source path a manifest publishes when it names none.
const DEFAULT: &str = "src";

/// Marks each item of `surface` whose file is outside its project's published paths.
pub(super) fn mark_internal(surface: &mut ApiSurface, files: &Imports<'_>) {
    let mut published: HashMap<String, Vec<String>> = HashMap::new();
    for item in &mut surface.items {
        let Some(dir) = project_of(files, &item.file) else {
            continue;
        };
        let paths = published
            .entry(dir.clone())
            .or_insert_with(|| source_paths(files, &dir));
        item.internal = !paths.iter().any(|path| holds(path, &item.file));
    }
}

/// The nearest directory above `file` with a project manifest; `None` when none has one.
fn project_of(files: &Imports<'_>, file: &str) -> Option<String> {
    nearest_holding(files, file, &MANIFESTS.map(|(manifest, _)| manifest))
}

/// The nearest directory above `file` (empty for the root) holding one of `manifests`;
/// `None` when none does.
pub(super) fn nearest_holding(
    files: &Imports<'_>,
    file: &str,
    manifests: &[&str],
) -> Option<String> {
    std::path::Path::new(file)
        .ancestors()
        .skip(1)
        .map(|dir| dir.to_string_lossy().into_owned())
        .find(|dir| {
            manifests
                .iter()
                .any(|manifest| files.is_file(&join(dir, manifest)))
        })
}

/// The published paths of the project in `dir`, relative to the root.
fn source_paths(files: &Imports<'_>, dir: &str) -> Vec<String> {
    MANIFESTS
        .iter()
        .filter_map(|(manifest, key)| {
            let text = files.read(&join(dir, manifest))?;
            Some(strings_under(&text, key).unwrap_or_else(|| vec![DEFAULT.to_owned()]))
        })
        .flatten()
        .map(|path| {
            let path = path.trim_start_matches("./").trim_end_matches('/');
            if path.is_empty() || path == "." {
                dir.to_owned()
            } else {
                join(dir, path)
            }
        })
        .collect()
}

/// Whether the directory `path` (relative to the root, `.` or empty for it) holds `file`.
fn holds(path: &str, file: &str) -> bool {
    path.is_empty()
        || path == "."
        || file
            .strip_prefix(path)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// One token of EDN or Clojure text.
#[derive(Debug, PartialEq, Eq)]
enum Token {
    Open,
    Close,
    Text(String),
    Atom(String),
}

/// The strings of the vector that `key` maps to directly inside the first top-level form of
/// `text`; `None` when no such key is there, empty when its value is no vector.
fn strings_under(text: &str, key: &str) -> Option<Vec<String>> {
    let tokens = tokens(text);
    let at = tokens
        .iter()
        .position(|(depth, token)| *depth == 1 && *token == Token::Atom(key.to_owned()))?;
    let mut value = tokens[at + 1..].iter().skip_while(|(depth, token)| {
        *depth == 1 && matches!(token, Token::Atom(meta) if meta.starts_with('^'))
    });
    if value.next().map(|(_, token)| token) != Some(&Token::Open) {
        return Some(Vec::new());
    }
    Some(
        value
            .take_while(|(depth, _)| *depth > 1)
            .filter_map(|(depth, token)| match token {
                Token::Text(text) if *depth == 2 => Some(text.clone()),
                _ => None,
            })
            .collect(),
    )
}

/// The tokens of `text`, each with the nesting depth it is at (an opening or closing
/// delimiter is at the depth outside it). Comments, commas, and character literals are
/// skipped; a dispatch (`#{`, `#(`) is an atom `#` before its delimiter.
fn tokens(text: &str) -> Vec<(usize, Token)> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ';' => while chars.next_if(|&next| next != '\n').is_some() {},
            '"' => out.push((depth, Token::Text(string(&mut chars)))),
            '\\' => {
                chars.next();
            }
            '(' | '[' | '{' => {
                out.push((depth, Token::Open));
                depth += 1;
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                out.push((depth, Token::Close));
            }
            c if c.is_whitespace() || c == ',' => {}
            c => {
                let mut atom = String::from(c);
                while let Some(next) = chars.next_if(|&next| !ends_atom(next)) {
                    atom.push(next);
                }
                out.push((depth, Token::Atom(atom)));
            }
        }
    }
    out
}

/// Whether `c` ends an atom: whitespace, a comma, a delimiter, a string, or a comment.
fn ends_atom(c: char) -> bool {
    c.is_whitespace() || matches!(c, ',' | '(' | ')' | '[' | ']' | '{' | '}' | '"' | ';')
}

/// The rest of a string literal whose opening quote was read, escapes kept as written but
/// for the escaped character.
fn string(chars: &mut impl Iterator<Item = char>) -> String {
    let mut text = String::new();
    while let Some(c) = chars.next() {
        match c {
            '"' => break,
            '\\' => text.extend(chars.next()),
            c => text.push(c),
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::strings_under;

    #[test]
    fn a_top_level_key_names_its_vectors_strings_and_nothing_nested_or_commented() {
        let deps = "{:paths [\"src\" :extra \"res\\\"x\"] ; :paths [\"no\"]\n \
                    :aliases {:a {:paths [\"alias\"]}}}";
        assert_eq!(
            strings_under(deps, ":paths"),
            Some(vec!["src".to_owned(), "res\"x".to_owned()])
        );
        assert_eq!(
            strings_under("{:aliases {:a {:paths [\"alias\"]}}}", ":paths"),
            None
        );
        assert_eq!(strings_under("{:paths []}", ":paths"), Some(Vec::new()));
        assert_eq!(strings_under("{:paths :none}", ":paths"), Some(Vec::new()));
        let lein = "(defproject p \"1\" :source-paths ^:replace [\"src/clj\"] \
                    :profiles {:dev {:source-paths [\"dev\"]}})";
        assert_eq!(
            strings_under(lein, ":source-paths"),
            Some(vec!["src/clj".to_owned()])
        );
    }
}
