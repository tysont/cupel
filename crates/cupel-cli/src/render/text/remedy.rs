// ABOUTME: Text rendering of the remediation hints in `action.hints` (ADR-133): one line under a finding,
// ABOUTME: and in full under `explain`: every location, parameter, and imported name.
use std::fmt::Write as _;

use cupel_core::hint::{self, CheapestCut, CommonSubtree, DeepestBlock};
use serde_json::{Map, Value};

use super::{Paint, location, name};

/// Width of the row names under `remedy`.
const ROW: usize = 12;

/// The one-line form of the hint `key`, when `value` is a remediation hint of that key.
pub fn line(key: &str, value: &Value) -> Option<String> {
    let mut single = Map::new();
    single.insert(key.to_owned(), value.clone());
    if let Some(common) = hint::read::<CommonSubtree>(&single) {
        return Some(format!(
            "{} nodes at {}, shared by {}, {}",
            common.nodes,
            location(&common.location),
            count(common.others.len(), "other"),
            count(common.parameters.len(), "parameter")
        ));
    }
    if let Some(block) = hint::read::<DeepestBlock>(&single) {
        return Some(format!(
            "{} at {}, nesting {}",
            path(&block),
            location(&block.location),
            block.nesting
        ));
    }
    hint::read::<CheapestCut>(&single).map(|cut| {
        format!(
            "{} -> {} at line {}, {} and {} whole-module import{}",
            cut.from,
            cut.to,
            cut.line,
            count(cut.names.len(), "name"),
            cut.unnamed,
            if cut.unnamed == 1 { "" } else { "s" }
        )
    })
}

/// Every remediation hint in `hints`, in full, under a `remedy` label.
pub fn block(out: &mut String, hints: &Map<String, Value>, paint: &Paint) {
    let mut remedy = |summary: String, rows: Vec<(String, String)>| {
        let _ = writeln!(
            out,
            "  {}{summary}",
            paint.bold(&format!("{:<10}", "remedy"))
        );
        for (key, value) in rows {
            let _ = writeln!(out, "    {key:<ROW$}  {value}");
        }
    };
    if let Some(common) = hint::read::<CommonSubtree>(hints) {
        let mut rows = vec![("here".to_owned(), location(&common.location))];
        rows.extend(
            common
                .others
                .iter()
                .map(|other| (other.symbol.clone(), location(&other.location))),
        );
        rows.extend(common.parameters.iter().map(|parameter| {
            let places = if parameter.occurrences > 1 {
                format!(" ({} places)", parameter.occurrences)
            } else {
                String::new()
            };
            (
                "parameter".to_owned(),
                format!(
                    "{} {}{places}",
                    name(parameter.kind),
                    parameter.values.join(" | ")
                ),
            )
        }));
        remedy(
            format!(
                "extract the {}-node subtree the cluster shares ({} of this symbol)",
                common.nodes, common.share
            ),
            rows,
        );
    }
    if let Some(block) = hint::read::<DeepestBlock>(hints) {
        remedy(
            format!(
                "extract the deepest block, {} at nesting {}, which carries cyclomatic {} and \
                 cognitive {}",
                path(&block),
                block.nesting,
                block.cyclomatic,
                block.cognitive
            ),
            vec![("here".to_owned(), location(&block.location))],
        );
    }
    if let Some(cut) = hint::read::<CheapestCut>(hints) {
        let breaks = if cut.breaks_component {
            "breaks every cycle among these modules"
        } else {
            "breaks this cycle, though others among these modules remain"
        };
        let mut rows = vec![("imports".to_owned(), cut.specifiers.join(", "))];
        if !cut.names.is_empty() {
            rows.push(("names".to_owned(), cut.names.join(", ")));
        }
        if cut.unnamed > 0 {
            rows.push(("whole-module".to_owned(), cut.unnamed.to_string()));
        }
        remedy(
            format!(
                "cut the imports of {} from {} (line {}), which {breaks}",
                cut.to, cut.from, cut.line
            ),
            rows,
        );
    }
}

/// The construct kinds from the outermost block to the deepest, as `branch > loop > branch`.
fn path(block: &DeepestBlock) -> String {
    block
        .path
        .iter()
        .map(|kind| name(*kind))
        .collect::<Vec<_>>()
        .join(" > ")
}

/// `n` and `noun`, plural unless `n` is 1.
fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}
