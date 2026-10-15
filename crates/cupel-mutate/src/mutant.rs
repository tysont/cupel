// ABOUTME: Sites to mutants (ADR-080 §1): a site's text replaced by each of its replacements, located by the site's
// ABOUTME: span (1-based lines, 1-based byte columns), and only where the source still holds the site's text.
use cupel_core::syntax::{Site, Span};

/// `source` with `from`, at `span`, replaced by `to`; `None` when the span is not in `source`
/// or does not hold `from`.
pub fn apply(source: &str, span: &Span, from: &str, to: &str) -> Option<String> {
    let start = offset(source, span.line, span.col)?;
    let end = offset(source, span.end_line, span.end_col)?;
    if source.get(start..end)? != from {
        return None;
    }
    let mut mutated = String::with_capacity(source.len() + to.len());
    mutated.push_str(&source[..start]);
    mutated.push_str(to);
    mutated.push_str(&source[end..]);
    Some(mutated)
}

/// Each of `site`'s replacements with the source it makes, in the site's order; replacements
/// that cannot be applied are left out.
pub fn replacements(source: &str, site: &Site) -> Vec<(String, String)> {
    site.to
        .iter()
        .filter_map(|to| Some((to.clone(), apply(source, &site.span, &site.from, to)?)))
        .collect()
}

/// The span of bytes `start..end` of `source`, in 1-based lines and byte columns.
pub fn span(source: &str, start: usize, end: usize) -> Span {
    let position = |at: usize| {
        let before = &source[..at];
        let line = before.matches('\n').count() + 1;
        let col = at - before.rfind('\n').map_or(0, |newline| newline + 1) + 1;
        (
            u32::try_from(line).unwrap_or(u32::MAX),
            u32::try_from(col).unwrap_or(u32::MAX),
        )
    };
    let ((line, col), (end_line, end_col)) = (position(start), position(end));
    Span {
        line,
        col,
        end_line,
        end_col,
    }
}

/// The byte offset of 1-based `line` and 1-based byte column `col`.
pub(crate) fn offset(source: &str, line: u32, col: u32) -> Option<usize> {
    let line = usize::try_from(line.checked_sub(1)?).ok()?;
    let col = usize::try_from(col.checked_sub(1)?).ok()?;
    let start = if line == 0 {
        0
    } else {
        source.match_indices('\n').nth(line - 1)?.0 + 1
    };
    let at = start + col;
    (at <= source.len()).then_some(at)
}
