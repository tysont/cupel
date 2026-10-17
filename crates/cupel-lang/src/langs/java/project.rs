// ABOUTME: Java roots for `ProjectInfo`: Maven's `pom.xml` and Gradle's build and settings scripts, with
// ABOUTME: build output and Gradle's state as generated directories (ADR-021, ADR-037, ADR-108).
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
    skip_dirs: &["target", "build", ".gradle"],
    describe: |candidate| candidate.root("java", Vec::new()),
};
