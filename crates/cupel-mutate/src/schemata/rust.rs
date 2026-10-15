// ABOUTME: Rust schemata (ADR-082): each mutant's smallest whole expression becomes an `if cupel_mutant_id() == N`
// ABOUTME: chain, sites where an `if` cannot stand are excluded, and the `CUPEL_MUTANT` reader is appended once.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use tree_sitter::{Node, Parser, Tree};

use super::{Gate, Rewriter, Rewritten};
use crate::mutant::offset;

/// The Rust rewriter.
pub struct Rust;

/// Appended to every schemata file: the mutant this process runs, read once (0 for none).
const READER: &str = "
#[allow(dead_code)]
fn cupel_mutant_id() -> u32 {
    static ID: ::std::sync::OnceLock<u32> = ::std::sync::OnceLock::new();
    *ID.get_or_init(|| {
        ::std::env::var(\"CUPEL_MUTANT\")
            .ok()
            .and_then(|id| id.parse().ok())
            .unwrap_or(0)
    })
}
";

/// Parents under which a gated expression must be parenthesized to parse as one operand.
const OPERAND_PARENTS: &[&str] = &[
    "binary_expression",
    "unary_expression",
    "reference_expression",
    "try_expression",
    "type_cast_expression",
    "field_expression",
    "call_expression",
    "index_expression",
    "range_expression",
    "await_expression",
    "expression_statement",
    "let_chain",
];

/// Ancestors in which no expression may call a function at run time, by why.
const CONSTANT_CONTEXTS: &[(&str, &str)] = &[
    ("const_item", "in a constant context"),
    ("static_item", "in a constant context"),
    ("const_block", "in a constant context"),
    ("const_parameter", "in a constant context"),
    ("enum_variant", "in a constant context"),
    ("array_type", "in a constant context"),
    ("type_arguments", "in a constant context"),
    ("token_tree", "inside a macro"),
    ("macro_definition", "inside a macro"),
    ("attribute_item", "inside an attribute"),
    ("inner_attribute_item", "inside an attribute"),
    ("closure_parameters", "inside a pattern"),
];

/// Operators whose right operand the compiler checks when it is a constant.
const CHECKED_DIVISORS: &[&str] = &["/", "%", "<<", ">>"];

impl Rewriter for Rust {
    fn rewrite(&self, source: &str, gates: &[Gate<'_>]) -> Rewritten {
        let mut rewritten = Rewritten::default();
        let Some(tree) = parse(source) else {
            for gate in gates {
                rewritten
                    .excluded
                    .insert(gate.id, "the file does not parse".to_owned());
            }
            return rewritten;
        };
        let mut targets: BTreeMap<(usize, std::cmp::Reverse<usize>), Target> = BTreeMap::new();
        for gate in gates {
            match target(&tree, source, gate) {
                Ok((target, variant)) => {
                    rewritten.lines.insert(gate.id, target.line);
                    targets
                        .entry((target.start, std::cmp::Reverse(target.end)))
                        .or_insert(target)
                        .variants
                        .push((gate.id, variant));
                }
                Err(reason) => {
                    rewritten.excluded.insert(gate.id, reason);
                }
            }
        }
        if targets.is_empty() {
            return rewritten;
        }
        let targets: Vec<Target> = targets.into_values().collect();
        let mut text = render(source, &targets, 0, source.len());
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(READER);
        rewritten.source = Some(text);
        rewritten
    }

    fn culprits(&self, output: &str, file: &str) -> BTreeSet<u32> {
        let mut lines = BTreeSet::new();
        let mut in_error = false;
        for line in output.lines().map(str::trim_start) {
            if line.starts_with("error") {
                in_error = true;
            } else if line.starts_with("warning") {
                in_error = false;
            } else if let Some(at) = line.strip_prefix("--> ").filter(|_| in_error) {
                let mut parts = at.rsplitn(3, ':');
                let (_, line, path) = (parts.next(), parts.next(), parts.next());
                if let (Some(path), Some(line)) = (path, line.and_then(|l| l.parse().ok()))
                    && same_file(path, file)
                {
                    lines.insert(line);
                }
            }
        }
        lines
    }
}

/// Whether the compiler's `path` (relative to wherever cargo ran) is the repository's `file`.
fn same_file(path: &str, file: &str) -> bool {
    path == file || file.ends_with(&format!("/{path}")) || path.ends_with(&format!("/{file}"))
}

fn parse(source: &str) -> Option<Tree> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .ok()?;
    parser.parse(source, None)
}

/// An expression gated by one or more mutants.
#[derive(Debug)]
struct Target {
    start: usize,
    end: usize,
    /// 1-based line the expression starts on.
    line: u32,
    /// Whether the chain needs parentheses where it stands.
    parens: bool,
    /// Inline modules between the expression and the file's top level.
    depth: usize,
    /// Each mutant's id and the expression's text with that mutant applied.
    variants: Vec<(u32, String)>,
}

/// The expression `gate` is placed on and its variant text, or why it cannot be.
fn target(tree: &Tree, source: &str, gate: &Gate<'_>) -> Result<(Target, String), String> {
    let span = gate.span;
    let not_found = || "its site is not where its span says".to_owned();
    let start = offset(source, span.line, span.col).ok_or_else(not_found)?;
    let end = offset(source, span.end_line, span.end_col).ok_or_else(not_found)?;
    if source.get(start..end) != Some(gate.from) {
        return Err(not_found());
    }
    let mut site = tree
        .root_node()
        .descendant_for_byte_range(start, end)
        .filter(|node| node.start_byte() == start && node.end_byte() == end)
        .ok_or_else(not_found)?;
    // A keyword literal is a token inside its named node of the same extent.
    while let Some(parent) = site.parent().filter(|parent| {
        !site.is_named() && parent.start_byte() == start && parent.end_byte() == end
    }) {
        site = parent;
    }
    let node = if site.is_named() {
        site
    } else {
        site.parent()
            .filter(|parent| {
                matches!(
                    parent.kind(),
                    "binary_expression" | "compound_assignment_expr"
                )
            })
            .ok_or_else(not_found)?
    };
    excluded(node, site, source)?;
    let original = &source[node.start_byte()..node.end_byte()];
    let mut variant = String::with_capacity(original.len() + gate.to.len());
    variant.push_str(&source[node.start_byte()..start]);
    variant.push_str(gate.to);
    variant.push_str(&source[end..node.end_byte()]);
    let variant = folded(node, &variant)?;
    Ok((
        Target {
            start: node.start_byte(),
            end: node.end_byte(),
            line: u32::try_from(node.start_position().row + 1).unwrap_or(u32::MAX),
            parens: needs_parens(node),
            depth: ancestors(node).filter(|a| a.kind() == "mod_item").count(),
            variants: Vec::new(),
        },
        variant,
    ))
}

/// Every ancestor of `node`, nearest first.
fn ancestors(node: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    std::iter::successors(node.parent(), Node::parent)
}

/// Why a gate on `node` (for the site `site`) cannot be compiled behind a run-time switch.
fn excluded(node: Node<'_>, site: Node<'_>, source: &str) -> Result<(), String> {
    let mut child = node;
    // A match guard is an expression inside the arm's pattern.
    let mut guard = false;
    for parent in ancestors(node) {
        let in_guard = guard && parent.kind() == "match_arm";
        guard = parent.kind() == "match_pattern"
            && parent.child_by_field_name("condition") == Some(child);
        if !guard
            && !in_guard
            && let Some(reason) = context(child, parent)
        {
            return Err(reason.to_owned());
        }
        child = parent;
    }
    if checked_constant(node, site, source) {
        return Err("a constant the compiler checks".to_owned());
    }
    Ok(())
}

/// Why `parent`, holding `child`, is no place for a run-time switch; `None` when it is.
fn context(child: Node<'_>, parent: Node<'_>) -> Option<&'static str> {
    let kind = parent.kind();
    let is_field = |name: &str| parent.child_by_field_name(name) == Some(child);
    if (kind.ends_with("_pattern") && !is_field("condition")) || is_field("pattern") {
        return Some("inside a pattern");
    }
    if (kind == "array_expression" && is_field("length")) || is_const_fn(parent) {
        return Some("in a constant context");
    }
    if kind == "field_expression" && is_field("field") {
        return Some("a tuple field index");
    }
    CONSTANT_CONTEXTS
        .iter()
        .find(|(context, _)| *context == kind)
        .map(|(_, reason)| *reason)
}

/// Whether `node` is a `const fn`.
fn is_const_fn(node: Node<'_>) -> bool {
    if node.kind() != "function_item" {
        return false;
    }
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .filter(|child| child.kind() == "function_modifiers")
        .any(|modifiers| {
            let mut cursor = modifiers.walk();
            modifiers
                .children(&mut cursor)
                .any(|modifier| modifier.kind() == "const")
        })
}

/// Whether the compiler evaluates the gated value as a constant, so a lint rejects a mutant
/// a file copy builds but cannot see one behind a switch: a literal that is an index, a
/// checked divisor or shift, or beside another literal, and an operator between literals.
fn checked_constant(node: Node<'_>, site: Node<'_>, source: &str) -> bool {
    let literal = |node: Option<Node<'_>>| node.is_some_and(|n| n.kind().ends_with("_literal"));
    if node.kind() == "binary_expression" && !site.is_named() {
        return literal(node.child_by_field_name("left"))
            && literal(node.child_by_field_name("right"));
    }
    if !node.kind().ends_with("_literal") {
        return false;
    }
    let Some(parent) = node.parent() else {
        return false;
    };
    match parent.kind() {
        "index_expression" => parent.named_child(1) == Some(node),
        "binary_expression" => {
            let (left, right) = (
                parent.child_by_field_name("left"),
                parent.child_by_field_name("right"),
            );
            let operator = parent
                .child_by_field_name("operator")
                .map_or("", |op| &source[op.start_byte()..op.end_byte()]);
            let other = if left == Some(node) { right } else { left };
            literal(other) || (right == Some(node) && CHECKED_DIVISORS.contains(&operator))
        }
        _ => false,
    }
}

/// Whether a chain in `node`'s place needs parentheses: as an operand, a receiver, a
/// statement, the left of an assignment, or the value of a `let … else`.
fn needs_parens(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    let kind = parent.kind();
    OPERAND_PARENTS.contains(&kind)
        || (matches!(kind, "assignment_expression" | "compound_assignment_expr")
            && parent.child_by_field_name("left") == Some(node))
        || (kind == "let_declaration" && parent.child_by_field_name("alternative").is_some())
}

/// `variant` of `node` on one line, so the file keeps its lines; why not when folding would
/// change it (a line comment, or a literal spanning lines).
fn folded(node: Node<'_>, variant: &str) -> Result<String, String> {
    if !variant.contains('\n') {
        return Ok(variant.to_owned());
    }
    if spans_lines(node) {
        return Err("a multi-line expression holding a comment or multi-line literal".to_owned());
    }
    Ok(variant
        .split('\n')
        .map(str::trim)
        .collect::<Vec<_>>()
        .join(" "))
}

/// Whether `node` holds a line comment or a literal written over more than one line.
fn spans_lines(node: Node<'_>) -> bool {
    let kind = node.kind();
    if kind == "line_comment"
        || (kind.ends_with("_literal") && node.start_position().row != node.end_position().row)
    {
        return true;
    }
    let mut cursor = node.walk();
    node.children(&mut cursor).any(spans_lines)
}

/// `source[start..end]` with every target in it (sorted by start, outermost first) gated.
fn render(source: &str, targets: &[Target], start: usize, end: usize) -> String {
    let mut out = String::with_capacity(end - start);
    let mut at = start;
    let mut index = 0;
    while index < targets.len() {
        let target = &targets[index];
        let inner = 1 + targets[index + 1..]
            .iter()
            .take_while(|inner| inner.end <= target.end)
            .count();
        out.push_str(&source[at..target.start]);
        let original = render(
            source,
            &targets[index + 1..index + inner],
            target.start,
            target.end,
        );
        out.push_str(&chain(target, &original));
        at = target.end;
        index += inner;
    }
    out.push_str(&source[at..end]);
    out
}

/// `target`'s chain: one branch per mutant, then `original`.
fn chain(target: &Target, original: &str) -> String {
    let reader = format!("{}cupel_mutant_id()", "super::".repeat(target.depth));
    let mut chain = String::new();
    for (id, variant) in &target.variants {
        let _ = write!(chain, "if {reader} == {id} {{ {variant} }} else ");
    }
    let _ = write!(chain, "{{ {original} }}");
    if target.parens {
        format!("({chain})")
    } else {
        chain
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutant::span;

    /// A gate on the first `from` inside the first `context` of `source`.
    fn gate<'a>(source: &str, id: u32, context: &str, from: &'a str, to: &'a str) -> Gate<'a> {
        let at = source.find(context).expect("context in source")
            + context.find(from).expect("from in context");
        Gate {
            id,
            span: span(source, at, at + from.len()),
            from,
            to,
        }
    }

    fn rewritten(source: &str, gates: &[Gate<'_>]) -> String {
        Rust.rewrite(source, gates)
            .source
            .expect("a schemata source")
    }

    /// The rewritten source without the reader.
    fn body(source: &str, gates: &[Gate<'_>]) -> String {
        rewritten(source, gates).replace(READER, "")
    }

    #[test]
    fn a_relational_operator_gates_its_comparison() {
        let source = "fn f(a: i64) -> bool {\n    a >= 1\n}\n";
        assert_eq!(
            body(source, &[gate(source, 1, "a >= 1", ">=", ">")]),
            "fn f(a: i64) -> bool {\n    if cupel_mutant_id() == 1 { a > 1 } else { a >= 1 }\n}\n"
        );
    }

    #[test]
    fn every_operator_class_gates_its_expression() {
        let source = "fn f(a: i64, b: bool) -> bool {\n    let mut x = a + 2;\n    x *= 3;\n    \
                      let y = x == 4;\n    y || b || false\n}\n";
        let gates = [
            gate(source, 1, "a + 2", "+", "-"),
            gate(source, 2, "x *= 3", "*=", "/="),
            gate(source, 3, "x == 4", "==", "!="),
            gate(source, 4, "y || b", "||", "&&"),
            gate(source, 5, "false", "false", "true"),
        ];
        assert_eq!(
            body(source, &gates),
            "fn f(a: i64, b: bool) -> bool {\n    \
             let mut x = if cupel_mutant_id() == 1 { a - 2 } else { a + 2 };\n    \
             (if cupel_mutant_id() == 2 { x /= 3 } else { x *= 3 });\n    \
             let y = if cupel_mutant_id() == 3 { x != 4 } else { x == 4 };\n    \
             (if cupel_mutant_id() == 4 { y && b } else { y || b }) || \
             (if cupel_mutant_id() == 5 { true } else { false })\n}\n"
        );
    }

    #[test]
    fn nested_sites_keep_the_original_text_in_every_other_branch_and_share_a_chain() {
        let source = "fn f(t: i64, m: bool) -> bool {\n    t >= 100 && m\n}\n";
        let gates = [
            gate(source, 1, "t >= 100", ">=", ">"),
            gate(source, 2, "&& m", "&&", "||"),
            gate(
                source,
                3,
                "t >= 100 && m",
                "t >= 100 && m",
                "!(t >= 100 && m)",
            ),
        ];
        assert_eq!(
            body(source, &gates),
            "fn f(t: i64, m: bool) -> bool {\n    \
             if cupel_mutant_id() == 2 { t >= 100 || m } else if cupel_mutant_id() == 3 { \
             !(t >= 100 && m) } else { \
             (if cupel_mutant_id() == 1 { t > 100 } else { t >= 100 }) && m }\n}\n"
        );
    }

    #[test]
    fn sites_where_an_if_cannot_stand_are_excluded_with_why() {
        let source = "const C: i64 = 1 + 2;\nstatic S: bool = true;\nconst fn k(a: i64) -> i64 { a + 1 }\n\
                      fn f(a: i64, t: (i64, i64)) -> i64 {\n    let v = [0; 1];\n    \
                      let w = vec![a + 1];\n    match a { 0 => 1, _ => t.0 }\n}\n";
        let gates = [
            gate(source, 1, "1 + 2", "+", "-"),
            gate(source, 2, "true", "true", "false"),
            gate(source, 3, "a + 1 }", "+", "-"),
            gate(source, 4, "[0; 1]", "1", "0"),
            gate(source, 5, "vec![a + 1]", "1", "0"),
            gate(source, 6, "0 => 1", "0", "1"),
            gate(source, 7, "t.0", "0", "1"),
            gate(source, 8, "0 => 1", "1", "0"),
        ];
        let rewritten = Rust.rewrite(source, &gates);
        let reasons: Vec<(u32, &str)> = rewritten
            .excluded
            .iter()
            .map(|(id, why)| (*id, why.as_str()))
            .collect();
        assert_eq!(
            reasons,
            [
                (1, "in a constant context"),
                (2, "in a constant context"),
                (3, "in a constant context"),
                (4, "in a constant context"),
                (5, "inside a macro"),
                (6, "inside a pattern"),
                (7, "a tuple field index"),
            ]
        );
        assert!(
            rewritten
                .source
                .unwrap()
                .contains("0 => if cupel_mutant_id() == 8 { 0 } else { 1 },")
        );
    }

    #[test]
    fn a_guard_is_an_expression_not_a_pattern() {
        let source = "fn f(a: i64) -> i64 {\n    match a { x if x > 1 => x, _ => 0 }\n}\n";
        let rewritten = Rust.rewrite(source, &[gate(source, 1, "x > 1", ">", ">=")]);
        assert_eq!(rewritten.excluded, BTreeMap::new());
    }

    #[test]
    fn constants_the_compiler_checks_are_excluded() {
        let source = "fn f(a: i64, v: [i64; 2]) -> i64 {\n    a / 1 + v[1] + (2 - 1)\n}\n";
        let gates = [
            gate(source, 1, "a / 1", "1", "0"),
            gate(source, 2, "v[1]", "1", "0"),
            gate(source, 3, "2 - 1", "-", "+"),
            gate(source, 4, "2 - 1", "1", "0"),
            gate(source, 5, "a / 1", "/", "*"),
        ];
        let rewritten = Rust.rewrite(source, &gates);
        assert_eq!(
            rewritten.excluded.keys().copied().collect::<Vec<_>>(),
            [1, 2, 3, 4]
        );
        assert_eq!(rewritten.lines, BTreeMap::from([(5, 2)]));
    }

    #[test]
    fn a_multi_line_expression_is_folded_so_no_line_moves() {
        let source = "fn f(a: i64,\n     b: i64) -> i64 {\n    let x = a\n        + b;\n    x\n}\n";
        let text = body(source, &[gate(source, 1, "+ b", "+", "-")]);
        assert_eq!(
            text,
            "fn f(a: i64,\n     b: i64) -> i64 {\n    let x = if cupel_mutant_id() == 1 { a - b } \
             else { a\n        + b };\n    x\n}\n"
        );
        assert_eq!(text.lines().count(), source.lines().count());
    }

    #[test]
    fn a_multi_line_expression_with_a_line_comment_is_excluded() {
        let source = "fn f(a: i64, b: i64) -> i64 {\n    a // why\n        + b\n}\n";
        let rewritten = Rust.rewrite(source, &[gate(source, 1, "+ b", "+", "-")]);
        assert_eq!(rewritten.source, None);
        assert_eq!(
            rewritten.excluded.get(&1).map(String::as_str),
            Some("a multi-line expression holding a comment or multi-line literal")
        );
    }

    #[test]
    fn a_gate_in_an_inline_module_reaches_the_reader_through_super() {
        let source = "mod inner {\n    pub fn f(a: i64) -> bool {\n        a < 1\n    }\n}\n";
        assert_eq!(
            body(source, &[gate(source, 1, "a < 1", "<", "<=")]),
            "mod inner {\n    pub fn f(a: i64) -> bool {\n        \
             if super::cupel_mutant_id() == 1 { a <= 1 } else { a < 1 }\n    }\n}\n"
        );
    }

    #[test]
    fn the_reader_is_appended_once_after_the_last_line() {
        let source = "fn f(a: i64) -> i64 {\n    a + 1 - 2\n}";
        let gates = [
            gate(source, 1, "a + 1", "+", "-"),
            gate(source, 2, "1 - 2", "-", "+"),
        ];
        let text = rewritten(source, &gates);
        assert_eq!(text.matches("fn cupel_mutant_id()").count(), 1);
        assert!(text.ends_with(READER));
        assert_eq!(
            text.lines().take(3).collect::<Vec<_>>().join("\n"),
            body(source, &gates).trim_end()
        );
    }

    #[test]
    fn a_site_that_is_not_where_its_span_says_is_excluded() {
        let source = "fn f(a: i64) -> i64 {\n    a + 1\n}\n";
        let mut moved = gate(source, 1, "a + 1", "+", "-");
        moved.from = "-";
        let rewritten = Rust.rewrite(source, &[moved]);
        assert_eq!(rewritten.source, None);
        assert_eq!(rewritten.excluded.len(), 1);
    }

    #[test]
    fn culprits_are_the_error_lines_the_compiler_names_in_the_file() {
        let output = "warning: unused variable\n  --> src/lib.rs:2:9\n\
                      error[E0369]: cannot subtract `&str` from `String`\n  --> src/lib.rs:7:23\n   |\n\
                      error[E0308]: mismatched types\n --> crates/x/src/lib.rs:9:1\n\
                      error: aborting\n --> src/other.rs:3:1\n";
        assert_eq!(
            Rust.culprits(output, "crates/x/src/lib.rs"),
            BTreeSet::from([7, 9])
        );
        assert_eq!(Rust.culprits(output, "src/main.rs"), BTreeSet::new());
    }
}
