// ABOUTME: npm, pnpm, and yarn roots for `ProjectInfo`: `package.json`, TypeScript when the root or a
// ABOUTME: member has a `tsconfig.json` and JavaScript otherwise (ADR-021, ADR-037).
use cupel_core::syntax::ProjectRoot;

use crate::project::{Candidate, Ecosystem};

pub const ECOSYSTEM: Ecosystem = Ecosystem {
    manifests: &["package.json"],
    skip_dirs: &["node_modules", "dist", "coverage"],
    describe,
};

fn describe(candidate: &Candidate) -> ProjectRoot {
    let typescript = candidate
        .dirs()
        .any(|dir| dir.join("tsconfig.json").is_file());
    let lang = if typescript {
        "typescript"
    } else {
        "javascript"
    };
    candidate.root(lang, Vec::new())
}
