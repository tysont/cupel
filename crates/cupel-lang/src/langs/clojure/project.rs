// ABOUTME: Clojure roots for `ProjectInfo`: `deps.edn` and Leiningen's `project.clj`, with build output and the CLI's
// ABOUTME: classpath cache as generated directories (ADR-021, ADR-037, ADR-109, ADR-234).
use crate::project::{Ecosystem, every_manifest};

/// `deps.edn` and `project.clj` roots. A directory below a root with a manifest of its own (a
/// `:local/root` dependency, a Leiningen subproject) is a member; neither tool takes package
/// patterns.
pub const ECOSYSTEM: Ecosystem = Ecosystem {
    manifests: &["deps.edn", "project.clj"],
    builds: every_manifest,
    skip_dirs: &["target", ".cpcache"],
    describe: |candidate| candidate.root("clojure", Vec::new()),
};
