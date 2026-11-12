// ABOUTME: The `strings` group's regex edits (ADR-180): each quantifier of a pattern edited (`*`↔`+`, `?` removed,
// ABOUTME: `{n,m}` to `{n}`) and each class edited (a negated class un-negated, a range narrowed to its first member).

/// One part of a regex pattern the edits read: a quantifier outside a class, or a class.
enum Part {
    /// `*`, `+`, `?`, or `{n,m}`, at `start..end`.
    Quantifier { start: usize, end: usize },
    /// `[…]` from its `[` at `open`: negated when it starts `[^`, with the ranges `a-z` at
    /// each `start..end`.
    Class {
        open: usize,
        negated: bool,
        ranges: Vec<(usize, usize)>,
    },
}

/// Each pattern `pattern` becomes with one quantifier edited, in the order they appear: `*` to
/// `+`, `+` to `*`, a `?` removed (an optional atom made required, or a lazy quantifier made
/// greedy), and `{n,m}` or `{n,}` to `{n}`.
pub fn quantifier_edits(pattern: &str) -> Vec<String> {
    parts(pattern)
        .into_iter()
        .filter_map(|part| match part {
            Part::Quantifier { start, end } => {
                let to = match &pattern[start..end] {
                    "*" => "+".to_owned(),
                    "+" => "*".to_owned(),
                    "?" => String::new(),
                    braces => {
                        let (low, _) = braces[1..braces.len() - 1].split_once(',')?;
                        format!("{{{low}}}")
                    }
                };
                Some(splice(pattern, start, end, &to))
            }
            Part::Class { .. } => None,
        })
        .collect()
}

/// Each pattern `pattern` becomes with one class edited, in the order they appear: a negated
/// class un-negated, and each range of a class narrowed to its first member.
pub fn class_edits(pattern: &str) -> Vec<String> {
    parts(pattern)
        .into_iter()
        .flat_map(|part| match part {
            Part::Class {
                open,
                negated,
                ranges,
            } => negated
                .then(|| splice(pattern, open + 1, open + 2, ""))
                .into_iter()
                .chain(ranges.into_iter().map(|(start, end)| {
                    let first = &pattern[start..=start];
                    splice(pattern, start, end, first)
                }))
                .collect(),
            Part::Quantifier { .. } => Vec::new(),
        })
        .collect()
}

/// `pattern` with `start..end` replaced by `to`.
fn splice(pattern: &str, start: usize, end: usize, to: &str) -> String {
    format!("{}{to}{}", &pattern[..start], &pattern[end..])
}

/// The quantifiers and classes of `pattern`, in order. An escaped character is a literal; a
/// `?` right after `(` opens a group's syntax (`(?:`, `(?=`, `(?<name>`), not a quantifier; a
/// `{` that does not open `{n}`, `{n,}`, or `{n,m}` is a literal.
fn parts(pattern: &str) -> Vec<Part> {
    let bytes = pattern.as_bytes();
    let mut found = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'\\' => at += 2,
            b'[' => {
                let (part, end) = class(pattern, at);
                found.push(part);
                at = end;
            }
            b'*' | b'+' => {
                found.push(Part::Quantifier {
                    start: at,
                    end: at + 1,
                });
                at += 1;
            }
            b'?' => {
                if at > 0 && bytes[at - 1] != b'(' {
                    found.push(Part::Quantifier {
                        start: at,
                        end: at + 1,
                    });
                }
                at += 1;
            }
            b'{' => match braces(pattern, at) {
                Some(end) => {
                    found.push(Part::Quantifier { start: at, end });
                    at = end;
                }
                None => at += 1,
            },
            _ => at += pattern[at..].chars().next().map_or(1, char::len_utf8),
        }
    }
    found
}

/// The class whose `[` is at `open`, and where the pattern continues after its `]`.
fn class(pattern: &str, open: usize) -> (Part, usize) {
    let bytes = pattern.as_bytes();
    let negated = bytes.get(open + 1) == Some(&b'^');
    let first = open + 1 + usize::from(negated);
    let mut ranges = Vec::new();
    let mut at = first;
    while at < bytes.len() && bytes[at] != b']' {
        if bytes[at] == b'\\' {
            at += 2;
            continue;
        }
        let width = pattern[at..].chars().next().map_or(1, char::len_utf8);
        let dash = at + width;
        let is_range = bytes.get(dash) == Some(&b'-')
            && bytes
                .get(dash + 1)
                .is_some_and(|byte| !matches!(byte, b']' | b'\\'));
        if is_range {
            let last = pattern[dash + 1..].chars().next().map_or(1, char::len_utf8);
            ranges.push((at, dash + 1 + last));
            at = dash + 1 + last;
        } else {
            at = dash;
        }
    }
    (
        Part::Class {
            open,
            negated,
            ranges,
        },
        (at + 1).min(bytes.len()),
    )
}

/// The end of the `{n}`, `{n,}`, or `{n,m}` whose `{` is at `open`, past its `}`.
fn braces(pattern: &str, open: usize) -> Option<usize> {
    let close = open + pattern[open..].find('}')?;
    let inside = &pattern[open + 1..close];
    let (low, high) = inside.split_once(',').unwrap_or((inside, "0"));
    let digits = |text: &str| text.bytes().all(|byte| byte.is_ascii_digit());
    (!low.is_empty() && digits(low) && digits(high)).then_some(close + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_quantifier_is_edited_once() {
        assert_eq!(
            quantifier_edits(r"^a*b+c?(?:d){2,4}e{3,}f{5}\*[*+?]$"),
            [
                r"^a+b+c?(?:d){2,4}e{3,}f{5}\*[*+?]$",
                r"^a*b*c?(?:d){2,4}e{3,}f{5}\*[*+?]$",
                r"^a*b+c(?:d){2,4}e{3,}f{5}\*[*+?]$",
                r"^a*b+c?(?:d){2}e{3,}f{5}\*[*+?]$",
                r"^a*b+c?(?:d){2,4}e{3}f{5}\*[*+?]$",
            ]
        );
    }

    #[test]
    fn a_lazy_quantifiers_question_mark_is_removed() {
        assert_eq!(quantifier_edits("a+?"), ["a*?", "a+"]);
    }

    #[test]
    fn each_class_is_un_negated_and_each_range_narrowed() {
        assert_eq!(
            class_edits(r"[^a-z0-9_][\w-][A-F\]]x{2}"),
            [
                r"[a-z0-9_][\w-][A-F\]]x{2}",
                r"[^a0-9_][\w-][A-F\]]x{2}",
                r"[^a-z0_][\w-][A-F\]]x{2}",
                r"[^a-z0-9_][\w-][A\]]x{2}",
            ]
        );
    }

    #[test]
    fn a_pattern_without_quantifiers_or_classes_has_no_edits() {
        assert_eq!(
            (quantifier_edits(r"abc\d\{1\}"), class_edits("abc{x}")),
            (Vec::<String>::new(), Vec::<String>::new())
        );
    }
}
