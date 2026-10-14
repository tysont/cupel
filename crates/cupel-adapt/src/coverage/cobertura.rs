// ABOUTME: Parses Cobertura XML (coverage.py's `--cov-report=xml`, and other tools') into a `CoverageMap`:
// ABOUTME: line hits per class file, and branches expanded from each line's `condition-coverage`.
use cupel_core::coverage::{BranchHits, CoverageMap, FileCoverage, LineHits};

use super::{ReportError, xml_number};

const FORMAT: &str = "Cobertura XML";

/// The coverage in the Cobertura XML `text`. A class's `filename` is joined to the report's
/// first `<source>` when it has one (`<source>/<filename>`), and kept as written otherwise.
/// Cobertura counts covered conditions without saying which: a line with
/// `condition-coverage="50% (1/2)"` has two branches, the first with 1 hit and the second with
/// none. A file has branches when one of its lines has a `condition-coverage`.
///
/// # Errors
///
/// When `text` is not XML with a `<coverage>` root, or a line's attributes are malformed.
pub fn parse_cobertura(text: &str) -> Result<CoverageMap, ReportError> {
    let document = roxmltree::Document::parse(text)
        .map_err(|error| ReportError::new(FORMAT, error.to_string()))?;
    let root = document.root_element();
    if root.tag_name().name() != "coverage" {
        return Err(ReportError::new(
            FORMAT,
            "the root is not <coverage>".to_owned(),
        ));
    }
    let source = root
        .descendants()
        .find(|node| node.has_tag_name("source"))
        .and_then(|node| node.text())
        .map(|text| text.trim().trim_end_matches('/').to_owned())
        .filter(|text| !text.is_empty());
    let mut files = Vec::new();
    for class in root.descendants().filter(|node| node.has_tag_name("class")) {
        let Some(filename) = class.attribute("filename") else {
            continue;
        };
        let file = match &source {
            Some(source) if !filename.starts_with('/') => format!("{source}/{filename}"),
            _ => filename.to_owned(),
        };
        let mut lines = Vec::new();
        let mut branches = Vec::new();
        let mut measured = false;
        for line in class
            .children()
            .filter(|node| node.has_tag_name("lines"))
            .flat_map(|node| node.children())
            .filter(|node| node.has_tag_name("line"))
        {
            let number: u32 = xml_number(FORMAT, line, "number")?;
            let hits: u64 = xml_number(FORMAT, line, "hits")?;
            lines.push(LineHits { line: number, hits });
            if let Some(conditions) = line.attribute("condition-coverage") {
                measured = true;
                let (covered, total) = conditions_of(conditions).ok_or_else(|| {
                    ReportError::new(
                        FORMAT,
                        format!("malformed condition-coverage `{conditions}`"),
                    )
                })?;
                branches.extend((0..total).map(|branch| BranchHits {
                    line: number,
                    branch,
                    hits: u64::from(branch < covered),
                }));
            }
        }
        files.push(FileCoverage {
            file,
            lines,
            branches: measured.then_some(branches),
        });
    }
    Ok(CoverageMap::new(files))
}

/// `(covered, total)` from `50% (1/2)`.
fn conditions_of(text: &str) -> Option<(u32, u32)> {
    let inner = text.split_once('(')?.1.strip_suffix(')')?;
    let (covered, total) = inner.split_once('/')?;
    Some((covered.trim().parse().ok()?, total.trim().parse().ok()?))
}
