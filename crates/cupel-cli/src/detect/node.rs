// ABOUTME: npm, pnpm, and yarn roots (`package.json`, workspaces): JavaScript or TypeScript by
// ABOUTME: `tsconfig.json`, the runner by dependency name, the package manager by lockfile (ADR-021).
use serde_json::Value;

use cupel_core::syntax::{DefaultCommands, ProjectRoot};

use super::{Candidate, Ecosystem};

pub const ECOSYSTEM: Ecosystem = Ecosystem {
    manifests: &["package.json"],
    skip_dirs: &["node_modules", "dist", "coverage"],
    describe,
};

/// The package manager: how it executes a dependency's binary and how it runs a script.
struct Manager {
    exec: &'static str,
    run: &'static str,
}

const NPM: Manager = Manager {
    exec: "npx",
    run: "npm run",
};
const PNPM: Manager = Manager {
    exec: "pnpm",
    run: "pnpm run",
};
const YARN: Manager = Manager {
    exec: "yarn",
    run: "yarn run",
};

fn describe(candidate: &Candidate) -> ProjectRoot {
    let manifests: Vec<Value> = candidate
        .dirs()
        .filter_map(|dir| std::fs::read_to_string(dir.join("package.json")).ok())
        .filter_map(|text| serde_json::from_str(&text).ok())
        .collect();
    let root_manifest = manifests.first().cloned().unwrap_or(Value::Null);
    let manager = manager(candidate, &root_manifest);
    let depends_on = |name: &str| {
        manifests.iter().any(|manifest| {
            ["dependencies", "devDependencies"]
                .iter()
                .any(|section| manifest[section].get(name).is_some())
        })
    };
    let x = manager.exec;
    let (test, coverage) = if depends_on("vitest") {
        (
            format!("{x} vitest run"),
            format!("{x} vitest run --coverage --coverage.reporter=lcov"),
        )
    } else if depends_on("jest") {
        (
            format!("{x} jest"),
            format!("{x} jest --coverage --coverageReporters=lcov"),
        )
    } else if depends_on("mocha") {
        (
            format!("{x} mocha"),
            format!("{x} c8 --reporter=lcov mocha"),
        )
    } else {
        (
            "node --test".to_owned(),
            format!("{x} c8 --reporter=lcov node --test"),
        )
    };
    let bench = root_manifest["scripts"]
        .get("bench")
        .map(|_| format!("{} bench", manager.run));
    let typescript = candidate
        .dirs()
        .any(|dir| dir.join("tsconfig.json").is_file());
    candidate.root(
        if typescript {
            "typescript"
        } else {
            "javascript"
        },
        DefaultCommands {
            test: Some(test),
            coverage: Some(coverage),
            bench,
        },
    )
}

/// pnpm or yarn when its lockfile, workspace file, or `packageManager` field says so; else npm.
fn manager(candidate: &Candidate, manifest: &Value) -> &'static Manager {
    let declared = manifest["packageManager"].as_str().unwrap_or_default();
    if candidate.has("pnpm-lock.yaml")
        || candidate.has("pnpm-workspace.yaml")
        || declared.starts_with("pnpm@")
    {
        &PNPM
    } else if candidate.has("yarn.lock") || declared.starts_with("yarn@") {
        &YARN
    } else {
        &NPM
    }
}
