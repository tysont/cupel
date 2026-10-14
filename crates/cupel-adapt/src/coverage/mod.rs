// ABOUTME: Coverage report parsers, one per format (LCOV, Cobertura XML, Istanbul JSON, Go coverprofile, JaCoCo
// ABOUTME: XML), format recognition, and placing a report's paths in the repository (design §8.7, ADR-077).
//! Coverage reports.
//!
//! A format parser knows a format, not a tool, and touches neither processes nor files: each
//! `parse_*` takes a report's text and returns a [`CoverageMap`] whose paths are exactly as the
//! report spells them. [`parse`] recognizes the format from the text. [`placed`] then makes
//! the paths relative to the repository root, given where the tool ran. Each parser is tested
//! against real reports committed under `cupel-fixtures/captures/coverage/`.
//!
//! ```text
//! let map = cupel_adapt::coverage::parse_lcov(&text)?;          // paths as written
//! let map = cupel_adapt::coverage::placed(map, &[ran_in], "web"); // relative to the repository
//! ```

mod cobertura;
mod coverprofile;
mod istanbul;
mod jacoco;
mod lcov;

use std::fmt;
use std::path::Path;

use cupel_core::coverage::CoverageMap;

pub use cobertura::parse_cobertura;
pub use coverprofile::parse_coverprofile;
pub use istanbul::parse_istanbul;
pub use jacoco::parse_jacoco;
pub use lcov::parse_lcov;

/// A report that could not be read as its format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportError {
    /// The format it was read as, such as `LCOV`.
    pub format: &'static str,
    pub message: String,
}

impl ReportError {
    pub fn new(format: &'static str, message: String) -> Self {
        Self { format, message }
    }
}

impl fmt::Display for ReportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "not a readable {} report: {}", self.format, self.message)
    }
}

impl std::error::Error for ReportError {}

/// A coverage report format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Lcov,
    Cobertura,
    Istanbul,
    Coverprofile,
    Jacoco,
}

impl Format {
    /// The format of the report `text`, recognized from its first significant characters: a
    /// `mode:` line (coverprofile), a JSON object (Istanbul), XML whose root is `<coverage>`
    /// (Cobertura) or `<report>` (`JaCoCo`), or LCOV records.
    pub fn of(text: &str) -> Option<Self> {
        let start = text.trim_start();
        if start.starts_with("mode:") {
            Some(Self::Coverprofile)
        } else if start.starts_with('{') {
            Some(Self::Istanbul)
        } else if start.starts_with('<') {
            if start.contains("<coverage") {
                Some(Self::Cobertura)
            } else if start.contains("<report") {
                Some(Self::Jacoco)
            } else {
                None
            }
        } else if start.starts_with("TN:") || start.starts_with("SF:") {
            Some(Self::Lcov)
        } else {
            None
        }
    }

    /// Reads `text` as this format.
    ///
    /// # Errors
    ///
    /// When `text` is not a readable report of this format.
    pub fn parse(self, text: &str) -> Result<CoverageMap, ReportError> {
        match self {
            Self::Lcov => parse_lcov(text),
            Self::Cobertura => parse_cobertura(text),
            Self::Istanbul => parse_istanbul(text),
            Self::Coverprofile => parse_coverprofile(text),
            Self::Jacoco => parse_jacoco(text),
        }
    }
}

/// The coverage in `text`, whatever its format.
///
/// # Errors
///
/// When the format is not recognized or the report is not readable as it.
pub fn parse(text: &str) -> Result<CoverageMap, ReportError> {
    Format::of(text)
        .ok_or_else(|| ReportError::new("coverage", "the format is not recognized".to_owned()))?
        .parse(text)
}

/// `map` with paths relative to the repository root, for a report a tool wrote while running
/// in the project root `dir` (relative to the repository root, `.` for itself). An absolute
/// path below one of `ran_in` (the directory as given, and its canonical form) is made relative
/// to it; a relative path is already relative to the root; both are then prefixed with `dir`.
/// Files outside the root (dependencies, generated code elsewhere) are dropped.
pub fn placed(map: CoverageMap, ran_in: &[&Path], dir: &str) -> CoverageMap {
    let files = map
        .files
        .into_iter()
        .filter_map(|mut file| {
            let relative = relative(&file.file, ran_in)?;
            file.file = if dir == "." {
                relative
            } else {
                format!("{dir}/{relative}")
            };
            Some(file)
        })
        .collect();
    CoverageMap::new(files)
}

/// `path` relative to the directory the tool ran in, `/`-separated; `None` outside it.
fn relative(path: &str, ran_in: &[&Path]) -> Option<String> {
    let relative = if Path::new(path).is_absolute() {
        let below = ran_in
            .iter()
            .find_map(|base| Path::new(path).strip_prefix(base).ok())?;
        below
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/")
    } else {
        path.trim_start_matches("./").replace('\\', "/")
    };
    (!relative.starts_with("../")).then_some(relative)
}

/// The numeric attribute `name` of an XML element of a `format` report.
fn xml_number<T: std::str::FromStr>(
    format: &'static str,
    node: roxmltree::Node<'_, '_>,
    name: &str,
) -> Result<T, ReportError> {
    node.attribute(name)
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| {
            ReportError::new(
                format,
                format!("a <{}> without a numeric `{name}`", node.tag_name().name()),
            )
        })
}
