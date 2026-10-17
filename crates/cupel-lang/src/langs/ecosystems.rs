// ABOUTME: Every build ecosystem Cupel recognizes, whose generated directories project-root walks skip,
// ABOUTME: and those whose language is detected but not registered, which `init` reports (ADR-038).
use std::path::Path;

use cupel_core::Exclude;
use cupel_core::syntax::ProjectRoot;

use crate::language::LanguageRegistry;
use crate::project::{Ecosystem, roots};

/// Ecosystems that are recognized but whose language is not registered; `init` reports their
/// roots rather than configuring them (ADR-038). Every recognized ecosystem's language is
/// registered, so there are none.
const UNREGISTERED: [&Ecosystem; 0] = [];

/// Every known ecosystem, whose generated directories every project-root walk skips: each of
/// `languages`' ecosystems in ID order (one shared by two languages appears twice), then the
/// unregistered ones (ADR-112).
pub(crate) fn known(languages: &LanguageRegistry) -> Vec<&'static Ecosystem> {
    languages
        .iter()
        .filter_map(|language| language.ecosystem())
        .chain(UNREGISTERED)
        .collect()
}

/// Roots of languages that are recognized but not registered outside `exclude`, ordered by
/// path, then language.
pub fn unregistered_roots(root: &Path, exclude: &Exclude) -> Vec<ProjectRoot> {
    let known = known(&LanguageRegistry::builtin());
    let mut roots: Vec<ProjectRoot> = UNREGISTERED
        .into_iter()
        .flat_map(|ecosystem| roots(root, ecosystem, &known, exclude))
        .collect();
    roots.sort_by(|a, b| (&a.path, &a.lang).cmp(&(&b.path, &b.lang)));
    roots
}
