// ABOUTME: Cargo roots for `ProjectInfo`: `Cargo.toml`, with nested manifests as workspace members and
// ABOUTME: `target/` as the generated directory (ADR-021, ADR-037).
use crate::project::Ecosystem;

pub const ECOSYSTEM: Ecosystem = Ecosystem {
    manifests: &["Cargo.toml"],
    skip_dirs: &["target"],
    describe: |candidate| candidate.root("rust", Vec::new()),
};
