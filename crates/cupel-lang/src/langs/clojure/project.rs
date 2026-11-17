// ABOUTME: Clojure roots for `ProjectInfo`: `deps.edn` and Leiningen's `project.clj`, with build output and the CLI's
// ABOUTME: classpath cache as generated directories (ADR-021, ADR-037, ADR-109, ADR-234, ADR-239).
use std::path::Path;

use crate::project::{Ecosystem, every_manifest};

/// `deps.edn` and `project.clj` roots. A directory below a root with a `deps.edn` (a
/// `:local/root` dependency) is a member; a Leiningen project below one (a subproject, built
/// apart from the aggregator above it) is a root of its own. Neither tool takes package
/// patterns.
pub const ECOSYSTEM: Ecosystem = Ecosystem {
    manifests: &["deps.edn", "project.clj"],
    builds: every_manifest,
    skip_dirs: &["target", ".cpcache"],
    describe: |candidate| candidate.root("clojure", Vec::new()),
    apart: leiningen,
};

/// Whether `dir` is a Leiningen project: a `project.clj` and no `deps.edn`, as ADR-234 tells
/// the two apart.
fn leiningen(dir: &Path) -> bool {
    dir.join("project.clj").is_file() && !dir.join("deps.edn").is_file()
}
