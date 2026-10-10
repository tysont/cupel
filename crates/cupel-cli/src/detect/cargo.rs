// ABOUTME: Cargo roots (`Cargo.toml`, workspaces) and their default Rust commands: nextest when it is
// ABOUTME: configured, llvm-cov for coverage, criterion benches when a manifest names criterion (ADR-021).
use cupel_core::syntax::{DefaultCommands, ProjectRoot};

use super::{Candidate, Ecosystem};

pub const ECOSYSTEM: Ecosystem = Ecosystem {
    manifests: &["Cargo.toml"],
    skip_dirs: &["target"],
    describe,
};

fn describe(candidate: &Candidate) -> ProjectRoot {
    let test = if candidate.has(".config/nextest.toml") {
        "cargo nextest run --workspace"
    } else {
        "cargo test --workspace"
    };
    let bench = candidate
        .mentions("Cargo.toml", "criterion")
        .then(|| "cargo bench --bench {name} -- --save-baseline {id}".to_owned());
    candidate.root(
        "rust",
        DefaultCommands {
            test: Some(test.to_owned()),
            coverage: Some("cargo llvm-cov --lcov --output-path {out}".to_owned()),
            bench,
        },
    )
}
