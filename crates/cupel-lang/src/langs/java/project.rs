// ABOUTME: Java roots for `ProjectInfo`: Maven's `pom.xml` and Gradle's build and settings scripts, with
// ABOUTME: build output and Gradle's state as generated directories (ADR-021, ADR-037, ADR-108).
use std::path::Path;

use roxmltree::{Document, Node};

use crate::project::Ecosystem;

/// Maven and Gradle roots. A module or subproject below a root, with a build file of its own, is
/// a member; Maven and Gradle select members by name, so no package patterns are reported.
pub const ECOSYSTEM: Ecosystem = Ecosystem {
    manifests: &[
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "settings.gradle",
        "settings.gradle.kts",
    ],
    builds,
    skip_dirs: &["target", "build", ".gradle"],
    describe: |candidate| candidate.root("java", Vec::new()),
};

/// Whether Maven builds the manifest at `path`: every Gradle script, and a `pom.xml` that names
/// its coordinates, its own `artifactId` and a `groupId` and `version` of its own or from its
/// `<parent>`. A POM template that another tool fills at release (tools.build's `write-pom`
/// over a Clojure project, say) has none, and Maven refuses it.
fn builds(path: &Path) -> bool {
    if path.file_name().is_none_or(|name| name != "pom.xml") {
        return true;
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(doc) = Document::parse(&text) else {
        return false;
    };
    let project = doc.root_element();
    let inherited = has_child(project, "parent");
    has_child(project, "artifactId")
        && (inherited || has_child(project, "groupId"))
        && (inherited || has_child(project, "version"))
}

/// Whether `node` has a child element named `name`.
fn has_child(node: Node<'_, '_>, name: &str) -> bool {
    node.children()
        .any(|child| child.is_element() && child.tag_name().name() == name)
}
