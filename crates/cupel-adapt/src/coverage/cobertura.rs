// ABOUTME: Parses Cobertura XML (coverage.py's `--cov-report=xml`, and other tools') into a `CoverageMap`:
// ABOUTME: line hits per class file, and branches expanded from each line's `condition-coverage`.
use cupel_core::coverage::{BranchHits, CoverageMap, FileCoverage, LineHits};

use super::error::{ReportError, xml_number};

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
    root.descendants()
        .filter(|node| node.has_tag_name("class"))
        .filter_map(|class| {
            let filename = class.attribute("filename")?;
            let file = match &source {
                Some(source) if !filename.starts_with('/') => format!("{source}/{filename}"),
                _ => filename.to_owned(),
            };
            Some(class_coverage(class, file))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(CoverageMap::new)
}

/// The lines and branches of one `<class>`, which covers `file`.
fn class_coverage(
    class: roxmltree::Node<'_, '_>,
    file: String,
) -> Result<FileCoverage, ReportError> {
    let mut lines = Vec::new();
    let mut branches: Option<Vec<BranchHits>> = None;
    for line in class
        .children()
        .filter(|node| node.has_tag_name("lines"))
        .flat_map(|node| node.children())
        .filter(|node| node.has_tag_name("line"))
    {
        let number: u32 = xml_number(FORMAT, line, "number")?;
        lines.push(LineHits {
            line: number,
            hits: xml_number(FORMAT, line, "hits")?,
        });
        if let Some(conditions) = line.attribute("condition-coverage") {
            let (covered, total) = conditions_of(conditions).ok_or_else(|| {
                ReportError::new(
                    FORMAT,
                    format!("malformed condition-coverage `{conditions}`"),
                )
            })?;
            branches
                .get_or_insert_with(Vec::new)
                .extend((0..total).map(|branch| BranchHits {
                    line: number,
                    branch,
                    hits: u64::from(branch < covered),
                }));
        }
    }
    Ok(FileCoverage {
        file,
        lines,
        branches,
    })
}

/// `(covered, total)` from `50% (1/2)`.
fn conditions_of(text: &str) -> Option<(u32, u32)> {
    let inner = text.split_once('(')?.1.strip_suffix(')')?;
    let (covered, total) = inner.split_once('/')?;
    Some((covered.trim().parse().ok()?, total.trim().parse().ok()?))
}
