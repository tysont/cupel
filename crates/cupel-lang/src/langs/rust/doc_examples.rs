// ABOUTME: Rust doctests in a definition's own doc comment (ADR-231): the `///` and `/** */` comments before it, and
// ABOUTME: whether one of their fenced code blocks is an example rustdoc runs that names the definition.
use tree_sitter::Node;

use crate::parse::Parsed;

/// Info-string words that make rustdoc compile a block without running it, or not compile it.
const NOT_RUN: [&str; 3] = ["ignore", "no_run", "compile_fail"];

/// Info-string words of a Rust block rustdoc runs, besides `edition…` and error codes.
const RUN: [&str; 4] = ["rust", "should_panic", "test_harness", "standalone_crate"];

/// Whether the doc comment of the definition `node` of `parsed` holds a fenced block rustdoc runs
/// whose code names `name`.
pub(super) fn runs_one(parsed: &Parsed, node: Node<'_>, name: &str) -> bool {
    let doc = doc_lines(parsed, node);
    blocks(&doc)
        .into_iter()
        .any(|(info, code)| runs(info) && names(&code, name))
}

/// The lines of the outer doc comments before `node`, through its attributes and plain comments,
/// in order and without their `///`, `/**`, or `*/`.
fn doc_lines(parsed: &Parsed, node: Node<'_>) -> Vec<String> {
    let mut comments = Vec::new();
    let mut sibling = node.prev_sibling();
    while let Some(previous) = sibling {
        let text = parsed.text(previous);
        match previous.kind() {
            "line_comment" if text.starts_with("///") && !text.starts_with("////") => {
                comments.push(text["///".len()..].to_owned());
            }
            "block_comment" if text.starts_with("/**") && !text.starts_with("/***") => {
                let inner = text["/**".len()..].trim_end_matches("*/");
                comments.push(inner.to_owned());
            }
            "attribute_item" | "line_comment" | "block_comment" => {}
            _ => break,
        }
        sibling = previous.prev_sibling();
    }
    comments.reverse();
    comments
        .iter()
        .flat_map(|comment| comment.lines().map(str::to_owned).collect::<Vec<_>>())
        .collect()
}

/// Each fenced code block of `lines` (opened and closed by three or more backquotes or tildes):
/// its info string and its code.
fn blocks(lines: &[String]) -> Vec<(&str, String)> {
    let mut blocks = Vec::new();
    let mut open: Option<(&str, &str, String)> = None;
    for line in lines {
        let trimmed = line.trim_start().trim_start_matches('*').trim_start();
        let fence = ["```", "~~~"]
            .into_iter()
            .find(|fence| trimmed.starts_with(fence));
        match (&mut open, fence) {
            (Some((opened, _, _)), Some(fence)) if *opened == fence => {
                let (_, info, code) = open.take().unwrap_or_default();
                blocks.push((info, code));
            }
            (Some((_, _, code)), _) => {
                code.push_str(line);
                code.push('\n');
            }
            (None, Some(fence)) => {
                let info = trimmed.trim_start_matches(&fence[..1]).trim();
                open = Some((fence, info, String::new()));
            }
            (None, None) => {}
        }
    }
    blocks
}

/// Whether rustdoc runs a block with the info string `info`: no word, or only words of a Rust
/// block it runs; any other word is another language or a mode that does not run it.
fn runs(info: &str) -> bool {
    info.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|word| !word.is_empty())
        .all(|word| {
            !NOT_RUN.contains(&word)
                && !word.starts_with("ignore-")
                && (RUN.contains(&word) || word.starts_with("edition") || is_error_code(word))
        })
}

/// Whether `word` is a rustc error code such as `E0277`.
fn is_error_code(word: &str) -> bool {
    word.strip_prefix('E')
        .is_some_and(|digits| digits.len() == 4 && digits.chars().all(|c| c.is_ascii_digit()))
}

/// Whether `code` holds `name` as a whole identifier.
fn names(code: &str, name: &str) -> bool {
    let part = |c: char| c.is_alphanumeric() || c == '_';
    code.match_indices(name).any(|(at, _)| {
        !code[..at].chars().next_back().is_some_and(part)
            && !code[at + name.len()..].chars().next().is_some_and(part)
    })
}
