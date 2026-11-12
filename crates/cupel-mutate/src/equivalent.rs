// ABOUTME: Lexical rules for possibly equivalent mutants (ADR-004): an operand that is both operators' identity, the
// ABOUTME: `extractable` flag of a WebCrypto key never exported, and an initializer assigned again before it is read.
use cupel_core::syntax::{MutationOperator, Site, Span};

use crate::mutant::offset;

/// Why the mutants of `site`, in the symbol at `symbol` of `source`, may be equivalent, when a
/// rule says so.
pub fn reason(source: &str, site: &Site, symbol: &Span) -> Option<String> {
    identity(source, site.operator, &site.span).or_else(|| {
        let body = Body::of(source, symbol)?;
        let start = offset(source, site.span.line, site.span.col)?.checked_sub(body.start)?;
        let end = offset(source, site.span.end_line, site.span.end_col)?.checked_sub(body.start)?;
        (end <= body.text.len()).then_some(()).and_then(|()| {
            unexported_key(&body, start, end).or_else(|| dead_initializer(&body, start, end))
        })
    })
}

/// A symbol's text, where it starts in its file, and which of its bytes are code (not in a
/// string or a comment).
struct Body<'a> {
    text: &'a str,
    start: usize,
    code: Vec<bool>,
}

impl<'a> Body<'a> {
    fn of(source: &'a str, symbol: &Span) -> Option<Self> {
        let start = offset(source, symbol.line, symbol.col)?;
        let end = offset(source, symbol.end_line, symbol.end_col)?;
        let text = source.get(start..end)?;
        Some(Self {
            text,
            start,
            code: code(text),
        })
    }
}

/// The `extractable` argument of `WebCrypto`'s `subtle.importKey(format, data, algorithm,
/// extractable, usages)` only decides whether the key may be exported or wrapped; in a function
/// that calls neither `exportKey` nor `wrapKey`, flipping it changes nothing the function does.
fn unexported_key(body: &Body<'_>, start: usize, end: usize) -> Option<String> {
    let text = body.text;
    if mentions(text, "exportKey") || mentions(text, "wrapKey") {
        return None;
    }
    let marked = text
        .match_indices("importKey")
        .take_while(|(at, _)| *at < start)
        .filter(|(at, _)| body.code[*at] && word_at(text, *at, "importKey"))
        .filter(|(at, _)| {
            text[..*at]
                .trim_end()
                .strip_suffix('.')
                .is_some_and(|before| before.trim_end().ends_with("subtle"))
        })
        .filter_map(|(at, name)| {
            let open = at + name.len() + leading_space(&text[at + name.len()..]);
            (text.as_bytes().get(open) == Some(&b'(')).then(|| arguments(body, open))
        })
        .any(|arguments| arguments.get(3) == Some(&(start, end)));
    marked.then(|| {
        "`importKey`'s `extractable` flag, and the function exports and wraps no key".to_owned()
    })
}

/// The trimmed byte ranges of the arguments of the call whose `(` is at `open`.
fn arguments(body: &Body<'_>, open: usize) -> Vec<(usize, usize)> {
    let bytes = body.text.as_bytes();
    let mut found = Vec::new();
    let mut depth = 0usize;
    let mut from = open + 1;
    for (at, byte) in bytes.iter().enumerate().skip(open + 1) {
        if !body.code[at] {
            continue;
        }
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' if depth > 0 => depth -= 1,
            b',' if depth == 0 => {
                found.push(trimmed(body.text, from, at));
                from = at + 1;
            }
            b')' => {
                found.push(trimmed(body.text, from, at));
                break;
            }
            _ => {}
        }
    }
    found
}

/// `start..end` of `text` without its surrounding whitespace.
fn trimmed(text: &str, start: usize, end: usize) -> (usize, usize) {
    let slice = &text[start..end];
    let first = start + slice.len() - slice.trim_start().len();
    (first, first.max(start + slice.trim_end().len()))
}

/// A mutant of a variable's whole initializer, when every path assigns the variable again
/// before reading it, stores a value nothing reads. The rule reads layout, so it holds only
/// for a block-scoped declaration (`let`, `let mut`, `:=`) whose initializer is the site
/// itself, a variable named nowhere before it, and an assignment at the declaration's own
/// indentation and nesting, not reading the variable, before anything else names it.
fn dead_initializer(body: &Body<'_>, start: usize, end: usize) -> Option<String> {
    let (name, line_start, line_end) = declaration(body.text, start, end)?;
    let indent = leading_space(&body.text[line_start..]);
    reassigned_before_read(body, name, indent, line_end + 1)
        .then(|| format!("`{name}` is assigned again before it is read"))
}

/// The variable declared on the line holding `start..end`, with that line's bounds, when the
/// site is the line's whole initializer, the variable is named nowhere before the line, and a
/// line follows it.
fn declaration(text: &str, start: usize, end: usize) -> Option<(&str, usize, usize)> {
    let line_start = text[..start].rfind('\n').map_or(0, |at| at + 1);
    let line_end = text[end..].find('\n').map_or(text.len(), |at| end + at);
    if !matches!(text[end..line_end].trim(), "" | ";") {
        return None;
    }
    let name = declared(&text[line_start..start])?;
    (!mentions(&text[..line_start], name) && line_end != text.len())
        .then_some((name, line_start, line_end))
}

/// Whether the lines from byte `from` assign `name` at `indent` and the declaration's nesting,
/// without reading it, before any line names it or the block ends.
fn reassigned_before_read(body: &Body<'_>, name: &str, indent: usize, from: usize) -> bool {
    let text = body.text;
    let mut depth = 0i64;
    let mut at = from;
    for line in text[from..].split_inclusive('\n') {
        if depth == 0 {
            match level(line, name, indent) {
                Level::Outside => return false,
                Level::Assigns(value) => {
                    let value = at + value;
                    return !mentions(&text[value..statement_end(body, value)], name);
                }
                Level::Other => {}
            }
        }
        if mentions(line, name) {
            return false;
        }
        depth += nesting_change(line, &body.code[at..]);
        if depth < 0 {
            return false;
        }
        at += line.len();
    }
    false
}

/// What a line at the declaration's nesting is to the rule.
enum Level {
    /// It is indented less than the declaration: the declaration's block has ended.
    Outside,
    /// It assigns the variable at the declaration's indentation; the value starts at this byte.
    Assigns(usize),
    /// Anything else.
    Other,
}

/// What `line`, at the declaration's nesting, is for the variable `name` declared at `indent`.
fn level(line: &str, name: &str, indent: usize) -> Level {
    let leading = leading_space(line);
    if !line.trim().is_empty() && leading < indent {
        return Level::Outside;
    }
    match assigned(line, name) {
        Some(value) if leading == indent => Level::Assigns(value),
        _ => Level::Other,
    }
}

/// How much `line` opens or closes brackets, counting only bytes `code` marks as code.
fn nesting_change(line: &str, code: &[bool]) -> i64 {
    line.bytes()
        .zip(code)
        .map(|(byte, is_code)| match byte {
            b'{' | b'(' | b'[' if *is_code => 1,
            b'}' | b')' | b']' if *is_code => -1,
            _ => 0,
        })
        .sum()
}

/// The variable a declaration ending in `=` declares: `let x =`, `let mut x =`, `let x: T =`,
/// or `x :=`.
fn declared(prefix: &str) -> Option<&str> {
    let text = prefix.trim();
    if let Some(rest) = text.strip_prefix("let ") {
        let rest = rest.trim_start();
        let rest = rest.strip_prefix("mut ").map_or(rest, str::trim_start);
        let (name, after) = identifier(rest)?;
        let typed = after.trim().strip_suffix('=')?.trim_end();
        (typed.is_empty() || (typed.starts_with(':') && !typed.contains('='))).then_some(name)
    } else {
        let (name, after) = identifier(text)?;
        (after.trim() == ":=").then_some(name)
    }
}

/// Where the assigned value starts in `line`, when `line` is a plain assignment `name = …`.
fn assigned(line: &str, name: &str) -> Option<usize> {
    let indent = leading_space(line);
    let rest = line[indent..].strip_prefix(name)?;
    let after = rest.trim_start();
    let value = after.strip_prefix('=')?;
    if value.starts_with(['=', '>']) || rest.starts_with(is_identifier) {
        return None;
    }
    Some(line.len() - value.len())
}

/// Where the statement whose code continues at `from` ends: the first line break or `;`
/// outside brackets.
fn statement_end(body: &Body<'_>, from: usize) -> usize {
    let mut depth = 0usize;
    for (at, byte) in body.text.bytes().enumerate().skip(from) {
        if !body.code[at] {
            continue;
        }
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' if depth > 0 => depth -= 1,
            b'\n' | b';' if depth == 0 => return at,
            _ => {}
        }
    }
    body.text.len()
}

/// The identifier `text` starts with, and the text after it.
fn identifier(text: &str) -> Option<(&str, &str)> {
    let end = text.find(|c: char| !is_identifier(c)).unwrap_or(text.len());
    let first = text.chars().next()?;
    (end > 0 && !first.is_ascii_digit()).then(|| text.split_at(end))
}

fn is_identifier(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// Whether `text` names `name` as a whole word anywhere, strings and comments included.
fn mentions(text: &str, name: &str) -> bool {
    text.match_indices(name)
        .any(|(at, _)| word_at(text, at, name))
}

/// Whether `name` at `at` of `text` is a whole word.
fn word_at(text: &str, at: usize, name: &str) -> bool {
    !text[..at].ends_with(is_identifier) && !text[at + name.len()..].starts_with(is_identifier)
}

fn leading_space(text: &str) -> usize {
    text.len() - text.trim_start_matches([' ', '\t']).len()
}

/// Per byte of `text`, whether it is code: not inside a `"`, `` ` ``, or same-line `'` string,
/// or a `//` or `/* */` comment.
fn code(text: &str) -> Vec<bool> {
    let bytes = text.as_bytes();
    let mut code = vec![true; bytes.len()];
    let mut at = 0;
    while at < bytes.len() {
        let end = match (bytes[at], bytes.get(at + 1)) {
            (b'"' | b'`', _) => Some(closing(bytes, at, false).unwrap_or(bytes.len())),
            (b'\'', _) => closing(bytes, at, true),
            (b'/', Some(b'/')) => Some(text[at..].find('\n').map_or(bytes.len(), |n| at + n)),
            (b'/', Some(b'*')) => Some(
                text[at + 2..]
                    .find("*/")
                    .map_or(bytes.len(), |n| at + n + 4),
            ),
            _ => None,
        };
        match end {
            Some(end) => {
                code[at..end].fill(false);
                at = end;
            }
            None => at += 1,
        }
    }
    code
}

/// The end of the string opened at `open`, past its closing quote; `None` when it is not
/// closed (on the same line, when `same_line`).
fn closing(bytes: &[u8], open: usize, same_line: bool) -> Option<usize> {
    let quote = bytes[open];
    let mut at = open + 1;
    while at < bytes.len() {
        match bytes[at] {
            b'\\' => at += 1,
            b'\n' if same_line => return None,
            byte if byte == quote => return Some(at + 1),
            _ => {}
        }
        at += 1;
    }
    None
}

/// Why the mutant replacing the operator at `span` of `source` may be equivalent, when the rule
/// says so: the operand after it is the identity of both the original and the replacement.
fn identity(source: &str, operator: MutationOperator, span: &Span) -> Option<String> {
    let identity = match operator {
        MutationOperator::ArithAddSub => "0",
        MutationOperator::ArithMulDiv => "1",
        _ => return None,
    };
    let operand = right_operand(source, span)?;
    let value = operand.strip_suffix(".0").unwrap_or(operand);
    (value == identity)
        .then(|| format!("the right operand is `{operand}`, the identity of both operators"))
}

/// The token after `span` on its line: a run of letters, digits, `_`, and `.`.
fn right_operand<'a>(source: &'a str, span: &Span) -> Option<&'a str> {
    let line = source
        .lines()
        .nth(usize::try_from(span.end_line).ok()?.checked_sub(1)?)?;
    let rest = line.get(usize::try_from(span.end_col).ok()?.checked_sub(1)?..)?;
    let rest = rest.trim_start();
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.'))
        .unwrap_or(rest.len());
    let token = &rest[..end];
    (!token.is_empty()).then_some(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(col: u32) -> Span {
        Span {
            line: 1,
            col,
            end_line: 1,
            end_col: col + 1,
        }
    }

    #[test]
    fn adding_or_subtracting_zero_and_scaling_by_one_are_marked() {
        let add = "x = a + 0\n";
        let scale = "y = b * 1.0;\n";
        assert_eq!(
            (
                identity(add, MutationOperator::ArithAddSub, &at(7)),
                identity(scale, MutationOperator::ArithMulDiv, &at(7))
            ),
            (
                Some("the right operand is `0`, the identity of both operators".to_owned()),
                Some("the right operand is `1.0`, the identity of both operators".to_owned())
            )
        );
    }

    #[test]
    fn other_operands_and_operators_are_not() {
        assert_eq!(
            identity("x = a + 10\n", MutationOperator::ArithAddSub, &at(7)),
            None
        );
        assert_eq!(
            identity("x = a * 0\n", MutationOperator::ArithMulDiv, &at(7)),
            None
        );
        assert_eq!(
            identity("x = a < 0\n", MutationOperator::Relational, &at(7)),
            None
        );
    }

    /// The reason `reason` gives for the site `from` on 1-based `line` of `source` (its first
    /// occurrence there), the whole source being one symbol.
    fn marked(source: &str, operator: MutationOperator, line: u32, from: &str) -> Option<String> {
        let text = source.lines().nth(line as usize - 1).unwrap();
        let col = u32::try_from(text.find(from).unwrap()).unwrap() + 1;
        let site = Site {
            operator,
            set: cupel_core::syntax::OperatorSet::Default,
            from: from.to_owned(),
            to: Vec::new(),
            site_hash: "0000000000000000".parse().unwrap(),
            span: Span {
                line,
                col,
                end_line: line,
                end_col: col + u32::try_from(from.len()).unwrap(),
            },
        };
        let lines = u32::try_from(source.lines().count()).unwrap();
        let last = u32::try_from(source.lines().last().unwrap().len()).unwrap();
        let symbol = Span {
            line: 1,
            col: 1,
            end_line: lines,
            end_col: last + 1,
        };
        reason(source, &site, &symbol)
    }

    const SIGN: &str = "async function sign(secret, data) {\n  const key = await crypto.subtle.importKey(\n    \"raw\",\n    secret,\n    { name: \"HMAC\", hash: \"SHA-256\" },\n    false,\n    [\"sign\"],\n  );\n  return crypto.subtle.sign(\"HMAC\", key, data);\n}";

    #[test]
    fn the_extractable_flag_of_a_key_the_function_never_exports_is_marked() {
        let one_line = "async function verify(raw) {\n  const key = await crypto.subtle.importKey(\"raw\", raw, { name: \"Ed25519\" }, false, [\"verify\"]);\n  return key;\n}";
        let flag = Some(
            "`importKey`'s `extractable` flag, and the function exports and wraps no key"
                .to_owned(),
        );
        assert_eq!(
            (
                marked(SIGN, MutationOperator::BooleanLiteral, 6, "false"),
                marked(one_line, MutationOperator::BooleanLiteral, 2, "false"),
            ),
            (flag.clone(), flag)
        );
    }

    #[test]
    fn the_extractable_flag_of_a_key_the_function_exports_or_another_argument_is_not() {
        let exported = SIGN.replace(
            "  return crypto",
            "  await crypto.subtle.exportKey(\"raw\", key);\n  return crypto",
        );
        let wrapped = SIGN.replace("  return crypto", "  wrapKey(key);\n  return crypto");
        let other = "function f(a) {\n  return importer.importKey(1, 2, 3, false);\n}";
        let third = "function f(a) {\n  return crypto.subtle.importKey(1, 2, false, true);\n}";
        assert_eq!(
            [
                marked(&exported, MutationOperator::BooleanLiteral, 6, "false"),
                marked(&wrapped, MutationOperator::BooleanLiteral, 6, "false"),
                marked(other, MutationOperator::BooleanLiteral, 2, "false"),
                marked(third, MutationOperator::BooleanLiteral, 2, "false"),
            ],
            [None, None, None, None]
        );
    }

    #[test]
    fn the_initializer_of_a_variable_assigned_again_before_it_is_read_is_marked() {
        let ts = "function pick(a) {\n  let label = 0;\n  const base = a * 2;\n  if (base > 3) {\n    log(\"}\");\n  }\n  label = base + 1;\n  return label;\n}";
        let typed =
            "function pick(a) {\n  let done: boolean = false;\n  done = a > 1;\n  return done;\n}";
        let rust = "fn total(a: i64) -> i64 {\n    let mut sum = 0;\n    sum = a * 2;\n    sum\n}";
        let go = "func total(a int) int {\n\tn := 1\n\tn = a * 2\n\treturn n\n}";
        assert_eq!(
            [
                marked(ts, MutationOperator::ZeroOne, 2, "0"),
                marked(typed, MutationOperator::BooleanLiteral, 2, "false"),
                marked(rust, MutationOperator::ZeroOne, 2, "0"),
                marked(go, MutationOperator::ZeroOne, 2, "1"),
            ],
            [
                Some("`label` is assigned again before it is read".to_owned()),
                Some("`done` is assigned again before it is read".to_owned()),
                Some("`sum` is assigned again before it is read".to_owned()),
                Some("`n` is assigned again before it is read".to_owned()),
            ]
        );
    }

    #[test]
    fn an_initializer_read_on_some_path_is_not() {
        let cases = [
            // Read before it is assigned again.
            (
                "function f(a) {\n  let x = 0;\n  log(x);\n  x = a;\n  return x;\n}",
                2,
            ),
            // Assigned again on one path only.
            (
                "function f(a) {\n  let x = 0;\n  if (a) {\n    x = a;\n  }\n  return x;\n}",
                2,
            ),
            // The braceless branch's assignment is indented under its condition.
            (
                "function f(a) {\n  let x = 0;\n  if (a)\n    x = a;\n  return x;\n}",
                2,
            ),
            // The assignment reads it.
            (
                "function f(a) {\n  let x = 0;\n  x = x + a;\n  return x;\n}",
                2,
            ),
            // The assignment's value, over several lines, reads it.
            (
                "function f(a) {\n  let x = 0;\n  x = g(\n    x,\n  );\n  return x;\n}",
                2,
            ),
            // A compound assignment reads it.
            (
                "function f(a) {\n  let x = 0;\n  x += a;\n  return x;\n}",
                2,
            ),
        ];
        assert_eq!(
            cases.map(|(source, line)| marked(source, MutationOperator::ZeroOne, line, "0")),
            [const { None }; 6]
        );
    }

    #[test]
    fn an_initializer_outside_the_rule_s_shape_is_not() {
        let cases = [
            // Its block ends before it is assigned again.
            (
                "function f(a) {\n  {\n    let x = 0;\n  }\n  x = a;\n  return x;\n}",
                3,
            ),
            // Named before the declaration (a closure may read it).
            (
                "function f(a) {\n  const g = () => x;\n  let x = 0;\n  g();\n  x = a;\n  return x;\n}",
                3,
            ),
            // Not a declaration.
            ("function f(a) {\n  x = 0;\n  x = a;\n  return x;\n}", 2),
            // Never assigned again.
            ("function f(a) {\n  let x = 0;\n  return x + a;\n}", 2),
        ];
        assert_eq!(
            cases.map(|(source, line)| marked(source, MutationOperator::ZeroOne, line, "0")),
            [const { None }; 4]
        );
        // The site is only part of the initializer.
        assert_eq!(
            marked(
                "function f(a) {\n  let x = g(0);\n  x = a;\n  return x;\n}",
                MutationOperator::ZeroOne,
                2,
                "0"
            ),
            None
        );
    }
}
