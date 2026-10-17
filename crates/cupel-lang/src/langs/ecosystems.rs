// ABOUTME: Every build ecosystem Cupel recognizes, whose generated directories project-root walks skip,
// ABOUTME: and those whose language is detected but not registered, which `init` reports (ADR-038).
use std::path::Path;

use cupel_core::Exclude;
use cupel_core::syntax::ProjectRoot;

use crate::project::{Ecosystem, roots};

/// Ecosystems that are recognized but whose language is not registered; `init` reports their
/// roots rather than configuring them (ADR-038). Every recognized ecosystem's language is
/// registered, so there are none.
const UNREGISTERED: [&Ecosystem; 0] = [];

/// Every known ecosystem, whose generated directories every project-root walk skips.
pub const KNOWN: [&Ecosystem; 6] = [
    &super::rust::project::ECOSYSTEM,
    &super::go::project::ECOSYSTEM,
    &super::ecma::project::ECOSYSTEM,
    &super::python::project::ECOSYSTEM,
    &super::java::project::ECOSYSTEM,
    &super::clojure::project::ECOSYSTEM,
];

/// Roots of languages that are recognized but not registered outside `exclude`, ordered by
/// path, then language.
pub fn unregistered_roots(root: &Path, exclude: &Exclude) -> Vec<ProjectRoot> {
    let mut roots: Vec<ProjectRoot> = UNREGISTERED
        .into_iter()
        .flat_map(|ecosystem| roots(root, ecosystem, &KNOWN, exclude))
        .collect();
    roots.sort_by(|a, b| (&a.path, &a.lang).cmp(&(&b.path, &b.lang)));
    roots
}
