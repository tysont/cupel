// ABOUTME: Every build ecosystem Cupel recognizes: the registered languages' and Java's and Clojure's,
// ABOUTME: which are detected but not registered until P6 (ADR-038); project-root walks skip them all.
use std::path::Path;

use cupel_core::Exclude;
use cupel_core::syntax::ProjectRoot;

use crate::project::{Ecosystem, roots};

/// Maven and Gradle roots. Java is not registered until P6; `init` reports these (ADR-038).
const JAVA: Ecosystem = Ecosystem {
    manifests: &[
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "settings.gradle",
        "settings.gradle.kts",
    ],
    skip_dirs: &["target", "build", ".gradle"],
    describe: |candidate| candidate.root("java", Vec::new()),
};

/// `deps.edn` roots. Clojure is not registered until P6; `init` reports these (ADR-038).
const CLOJURE: Ecosystem = Ecosystem {
    manifests: &["deps.edn"],
    skip_dirs: &["target", ".cpcache"],
    describe: |candidate| candidate.root("clojure", Vec::new()),
};

/// Every known ecosystem, whose generated directories every project-root walk skips.
pub const KNOWN: [&Ecosystem; 6] = [
    &super::rust::project::ECOSYSTEM,
    &super::go::project::ECOSYSTEM,
    &super::ecma::project::ECOSYSTEM,
    &super::python::project::ECOSYSTEM,
    &JAVA,
    &CLOJURE,
];

/// Roots of languages that are recognized but not registered (Java, Clojure) outside
/// `exclude`, ordered by path, then language.
pub fn unregistered_roots(root: &Path, exclude: &Exclude) -> Vec<ProjectRoot> {
    let mut roots: Vec<ProjectRoot> = [&JAVA, &CLOJURE]
        .into_iter()
        .flat_map(|ecosystem| roots(root, ecosystem, &KNOWN, exclude))
        .collect();
    roots.sort_by(|a, b| (&a.path, &a.lang).cmp(&(&b.path, &b.lang)));
    roots
}
