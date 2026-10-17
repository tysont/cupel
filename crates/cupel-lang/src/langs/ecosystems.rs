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

/// Every known ecosystem, whose generated directories every project-root walk skips: the
/// `registered` languages' ecosystems as given (one shared by two languages appears twice), then
/// the unregistered ones (ADR-112).
pub(crate) fn known(
    registered: impl IntoIterator<Item = &'static Ecosystem>,
) -> Vec<&'static Ecosystem> {
    registered.into_iter().chain(UNREGISTERED).collect()
}

/// Roots of languages that are recognized but not registered outside `exclude`, ordered by
/// path, then language; the walks skip the generated directories of every `known` ecosystem.
pub(crate) fn unregistered_roots(
    root: &Path,
    exclude: &Exclude,
    known: &[&'static Ecosystem],
) -> Vec<ProjectRoot> {
    let mut roots: Vec<ProjectRoot> = UNREGISTERED
        .into_iter()
        .flat_map(|ecosystem| roots(root, ecosystem, known, exclude))
        .collect();
    roots.sort_by(|a, b| (&a.path, &a.lang).cmp(&(&b.path, &b.lang)));
    roots
}
