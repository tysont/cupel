// ABOUTME: Staleness by function (ADR-084 §7): a changed file compared line by line with the file as measured, each
// ABOUTME: change placed in the function holding it, the functions' measured spans, and where unchanged lines moved.
use std::collections::BTreeMap;

use similar::{Algorithm, DiffOp, capture_diff_slices};

/// A hash of each line of a file, in order: what a cache entry keeps of a file as measured.
pub(crate) fn line_hashes(text: &str) -> Vec<u64> {
    text.lines()
        .map(|line| {
            let hash = blake3::hash(line.as_bytes());
            let mut first = [0; 8];
            first.copy_from_slice(&hash.as_bytes()[..8]);
            u64::from_le_bytes(first)
        })
        .collect()
}

/// What a change to one file did to the functions in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FunctionEdit {
    /// The changed functions' spans in the file as measured: 1-based, inclusive.
    stale: Vec<(u32, u32)>,
    /// Each unchanged line's number as measured, and its number now.
    moved: BTreeMap<u32, u32>,
}

impl FunctionEdit {
    /// Whether any of `lines` (as measured) is inside a changed function.
    pub fn touches(&self, lines: &[u32]) -> bool {
        lines.iter().any(|line| {
            self.stale
                .iter()
                .any(|(start, end)| (*start..=*end).contains(line))
        })
    }

    /// `lines` (as measured) as they are numbered now; a line that did not survive is dropped.
    pub fn moved(&self, lines: &[u32]) -> Vec<u32> {
        lines
            .iter()
            .filter_map(|line| self.moved.get(line).copied())
            .collect()
    }
}

/// One run of changed lines: 0-based, end-exclusive ranges as measured and now.
struct Hunk {
    old: (usize, usize),
    new: (usize, usize),
}

/// What changing a file from `measured` (its line hashes) to `now` did to `functions` (the
/// spans of its functions now, 1-based and inclusive); `None` when the file rule applies: a
/// change outside every function, inside a function with no unchanged line (new, or rewritten
/// whole), or no line changed at all.
pub(crate) fn edit(measured: &[u64], now: &str, functions: &[(u32, u32)]) -> Option<FunctionEdit> {
    let current = line_hashes(now);
    let ops = capture_diff_slices(Algorithm::Myers, measured, &current);
    let mut moved = BTreeMap::new();
    let mut to_old: Vec<Option<u32>> = vec![None; current.len()];
    for op in &ops {
        if let DiffOp::Equal {
            old_index,
            new_index,
            len,
        } = *op
        {
            for offset in 0..len {
                let (old, new) = (number(old_index + offset), number(new_index + offset));
                moved.insert(old, new);
                to_old[new_index + offset] = Some(old);
            }
        }
    }
    let hunks = hunks(&ops);
    if hunks.is_empty() {
        return None;
    }
    let mut stale = Vec::new();
    for hunk in &hunks {
        let (start, end) = innermost(functions, hunk)?;
        let kept: Vec<u32> = (start..=end)
            .filter_map(|line| to_old.get(line as usize - 1).copied().flatten())
            .collect();
        let first = kept.iter().min()?;
        let last = kept.iter().max()?;
        let (changed_first, changed_last) = (number(hunk.old.0), count(hunk.old.1));
        stale.push((
            (*first).min(changed_first),
            (*last).max(changed_last.max(changed_first)),
        ));
    }
    stale.sort_unstable();
    stale.dedup();
    Some(FunctionEdit { stale, moved })
}

/// The 1-based number of the 0-based line `index`.
fn number(index: usize) -> u32 {
    count(index + 1)
}

/// `value` as a line number.
fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// The changed runs of a diff: consecutive operations that are not `Equal`, merged.
fn hunks(ops: &[DiffOp]) -> Vec<Hunk> {
    let mut hunks: Vec<Hunk> = Vec::new();
    let mut open = false;
    for op in ops {
        if matches!(op, DiffOp::Equal { .. }) {
            open = false;
            continue;
        }
        let (old, new) = (op.old_range(), op.new_range());
        match hunks.last_mut() {
            Some(hunk) if open => {
                hunk.old.1 = old.end;
                hunk.new.1 = new.end;
            }
            _ => hunks.push(Hunk {
                old: (old.start, old.end),
                new: (new.start, new.end),
            }),
        }
        open = true;
    }
    hunks
}

/// The smallest of `functions` (1-based, inclusive) holding `hunk` now: every inserted line, or
/// for a deletion, the lines on both sides of it.
fn innermost(functions: &[(u32, u32)], hunk: &Hunk) -> Option<(u32, u32)> {
    let (first, last) = if hunk.new.1 > hunk.new.0 {
        (number(hunk.new.0), count(hunk.new.1))
    } else {
        (count(hunk.new.0), number(hunk.new.0))
    };
    functions
        .iter()
        .filter(|(start, end)| *start <= first && last <= *end)
        .min_by_key(|(start, end)| end - start)
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two functions, `add` on lines 2–4 and `scale` on lines 6–8, after a header line.
    const MEASURED: &str = "// calc\nfn add() {\n    a + b\n}\n\nfn scale() {\n    v * f\n}\n";

    fn edited(now: &str, functions: &[(u32, u32)]) -> Option<FunctionEdit> {
        edit(&line_hashes(MEASURED), now, functions)
    }

    #[test]
    fn a_line_added_inside_one_function_makes_only_its_measured_span_stale_and_moves_the_rest() {
        let now = "// calc\nfn add() {\n    // why\n    a + b\n}\n\nfn scale() {\n    v * f\n}\n";
        let edit = edited(now, &[(2, 5), (7, 9)]).unwrap();
        assert_eq!(
            (
                edit.touches(&[3]),
                edit.touches(&[7]),
                edit.moved(&[6, 7, 8])
            ),
            (true, false, vec![7, 8, 9])
        );
    }

    #[test]
    fn a_changed_line_inside_one_function_is_that_function_alone() {
        let now = MEASURED.replace("v * f", "f * v");
        let edit = edited(&now, &[(2, 4), (6, 8)]).unwrap();
        assert_eq!(
            (edit.touches(&[2, 3, 4]), edit.touches(&[6])),
            (false, true)
        );
    }

    #[test]
    fn a_deleted_line_inside_a_function_is_that_function() {
        let now = MEASURED.replace("    a + b\n", "");
        let edit = edited(&now, &[(2, 3), (5, 7)]).unwrap();
        assert_eq!(
            (edit.touches(&[3]), edit.touches(&[6]), edit.moved(&[6, 7])),
            (true, false, vec![5, 6])
        );
    }

    #[test]
    fn a_change_outside_every_function_or_a_new_function_is_the_whole_file() {
        let header = MEASURED.replace("// calc", "// calculator");
        let new_function = format!("{MEASURED}\nfn twice() {{\n    2 * x\n}}\n");
        assert_eq!(
            (
                edited(&header, &[(2, 4), (6, 8)]),
                edited(&new_function, &[(2, 4), (6, 8), (10, 12)]),
                edited(MEASURED, &[(2, 4), (6, 8)]),
            ),
            (None, None, None)
        );
    }

    #[test]
    fn a_change_in_a_nested_function_is_the_innermost_one() {
        let measured = "fn outer() {\n    fn inner() {\n        1\n    }\n    inner()\n}\n";
        let now = measured.replace("        1\n", "        2\n");
        let edit = edit(&line_hashes(measured), &now, &[(1, 6), (2, 4)]).unwrap();
        assert_eq!((edit.touches(&[3]), edit.touches(&[5])), (true, false));
    }
}
