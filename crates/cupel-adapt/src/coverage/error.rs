// ABOUTME: The error every coverage report parser returns, naming the format it read the report as, and the
// ABOUTME: numeric-attribute reader the XML formats share.
use std::fmt;

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

/// The numeric attribute `name` of an XML element of a `format` report.
pub(super) fn xml_number<T: std::str::FromStr>(
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
