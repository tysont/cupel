// ABOUTME: Recognizing a coverage report's format from its text, reading it as that format, and placing its
// ABOUTME: paths in the repository relative to where the tool ran (ADR-079).
use std::path::Path;

use cupel_core::coverage::CoverageMap;

use super::error::ReportError;
use super::{cobertura, coverprofile, istanbul, jacoco, lcov};

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
            Self::Lcov => lcov::parse_lcov(text),
            Self::Cobertura => cobertura::parse_cobertura(text),
            Self::Istanbul => istanbul::parse_istanbul(text),
            Self::Coverprofile => coverprofile::parse_coverprofile(text),
            Self::Jacoco => jacoco::parse_jacoco(text),
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
