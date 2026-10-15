// ABOUTME: The lexical identity rule for possibly equivalent mutants (ADR-004): swapping `+`/`-` before a literal
// ABOUTME: `0`, or `*`/`/` before a literal `1`, never changes a value, so such a mutant is marked.
use cupel_core::syntax::{MutationOperator, Span};

/// Why the mutant replacing the operator at `span` of `source` may be equivalent, when the rule
/// says so: the operand after it is the identity of both the original and the replacement.
pub fn reason(source: &str, operator: MutationOperator, span: &Span) -> Option<String> {
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
                reason(add, MutationOperator::ArithAddSub, &at(7)),
                reason(scale, MutationOperator::ArithMulDiv, &at(7))
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
            reason("x = a + 10\n", MutationOperator::ArithAddSub, &at(7)),
            None
        );
        assert_eq!(
            reason("x = a * 0\n", MutationOperator::ArithMulDiv, &at(7)),
            None
        );
        assert_eq!(
            reason("x = a < 0\n", MutationOperator::Relational, &at(7)),
            None
        );
    }
}
