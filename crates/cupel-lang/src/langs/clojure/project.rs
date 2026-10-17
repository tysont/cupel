// ABOUTME: Clojure roots for `ProjectInfo`: `deps.edn`, with build output and the CLI's classpath cache as
// ABOUTME: generated directories (ADR-021, ADR-037, ADR-109).
use crate::project::Ecosystem;

/// `deps.edn` roots. A directory below a root with a `deps.edn` of its own (a `:local/root`
/// dependency, say) is a member; the Clojure CLI takes no package patterns.
pub const ECOSYSTEM: Ecosystem = Ecosystem {
    manifests: &["deps.edn"],
    skip_dirs: &["target", ".cpcache"],
    describe: |candidate| candidate.root("clojure", Vec::new()),
};
