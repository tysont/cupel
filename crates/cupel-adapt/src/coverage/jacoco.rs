// ABOUTME: Parses JaCoCo XML reports into a `CoverageMap`: per source file, a line ran when it has covered
// ABOUTME: instructions, and its branches are its covered and missed branch counts (no execution counts).
use cupel_core::coverage::{BranchHits, CoverageMap, FileCoverage, LineHits};

use super::{ReportError, xml_number};

const FORMAT: &str = "JaCoCo XML";

/// The coverage in the `JaCoCo` XML `text`. Each `<sourcefile>` is named `<package>/<name>`, the
/// path below a source root (`grade/Grade.java`). `JaCoCo` reports no counts: a line has 1 hit
/// when it has covered instructions (`ci`) and 0 otherwise; a line with branches (`mb` + `cb`)
/// has `cb` branches with 1 hit followed by `mb` with none. A file has branches when one of its
/// lines does.
///
/// # Errors
///
/// When `text` is not XML with a `<report>` root, or a line's attributes are malformed.
pub fn parse_jacoco(text: &str) -> Result<CoverageMap, ReportError> {
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..roxmltree::ParsingOptions::default()
    };
    let document = roxmltree::Document::parse_with_options(text, options)
        .map_err(|error| ReportError::new(FORMAT, error.to_string()))?;
    let root = document.root_element();
    if root.tag_name().name() != "report" {
        return Err(ReportError::new(
            FORMAT,
            "the root is not <report>".to_owned(),
        ));
    }
    let mut files = Vec::new();
    for package in root.children().filter(|node| node.has_tag_name("package")) {
        let prefix = package.attribute("name").unwrap_or_default();
        for source in package
            .children()
            .filter(|node| node.has_tag_name("sourcefile"))
        {
            let name = source.attribute("name").unwrap_or_default();
            let file = if prefix.is_empty() {
                name.to_owned()
            } else {
                format!("{prefix}/{name}")
            };
            let mut lines = Vec::new();
            let mut branches = Vec::new();
            for line in source.children().filter(|node| node.has_tag_name("line")) {
                let number: u32 = xml_number(FORMAT, line, "nr")?;
                let covered: u64 = xml_number(FORMAT, line, "ci")?;
                let hit: u32 = xml_number(FORMAT, line, "cb")?;
                let missed: u32 = xml_number(FORMAT, line, "mb")?;
                lines.push(LineHits {
                    line: number,
                    hits: u64::from(covered > 0),
                });
                branches.extend((0..hit + missed).map(|branch| BranchHits {
                    line: number,
                    branch,
                    hits: u64::from(branch < hit),
                }));
            }
            let measured = !branches.is_empty();
            files.push(FileCoverage {
                file,
                lines,
                branches: measured.then_some(branches),
            });
        }
    }
    Ok(CoverageMap::new(files))
}
