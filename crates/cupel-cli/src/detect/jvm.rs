// ABOUTME: Maven and Gradle roots (Java) and `deps.edn` roots (Clojure), recognized with a language
// ABOUTME: but no commands: those languages and their tools arrive in P6 (ADR-021).
use cupel_core::syntax::{DefaultCommands, ProjectRoot};

use super::{Candidate, Ecosystem};

pub const JAVA: Ecosystem = Ecosystem {
    manifests: &[
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "settings.gradle",
        "settings.gradle.kts",
    ],
    skip_dirs: &["target", "build", ".gradle"],
    describe: java,
};

pub const CLOJURE: Ecosystem = Ecosystem {
    manifests: &["deps.edn"],
    skip_dirs: &["target", ".cpcache"],
    describe: clojure,
};

fn java(candidate: &Candidate) -> ProjectRoot {
    candidate.root("java", no_commands())
}

fn clojure(candidate: &Candidate) -> ProjectRoot {
    candidate.root("clojure", no_commands())
}

fn no_commands() -> DefaultCommands {
    DefaultCommands {
        test: None,
        coverage: None,
        bench: None,
    }
}
