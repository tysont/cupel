// ABOUTME: The JavaScript and TypeScript ecosystem of `tool.footprint` (ADR-103): `package-lock.json`'s packages with their
// ABOUTME: licenses, else `package.json`'s declared dependencies without versions; direct when `package.json` declares them.
use std::collections::BTreeSet;

use cupel_core::Dependency;
use serde_json::Value;

use super::{Context, Ecosystem, Project, Root, line_after};

pub static NPM: Ecosystem = Ecosystem {
    langs: &["javascript", "typescript"],
    osv: "npm",
    discover,
    parse,
    normalize: str::to_owned,
};

const LOCKFILE: &str = "package-lock.json";
const MANIFEST: &str = "package.json";

/// The fields of `package.json` that declare dependencies.
const DECLARING: [&str; 4] = [
    "dependencies",
    "devDependencies",
    "optionalDependencies",
    "peerDependencies",
];

/// `package-lock.json` when the root has one, else `package.json`; the names `package.json`
/// declares are direct.
fn discover(root: &Root<'_>) -> Project {
    let Ok(text) = std::fs::read_to_string(root.path.join(MANIFEST)) else {
        return Project::default();
    };
    let direct = declared(&text).into_iter().map(|(name, _)| name).collect();
    let locked = root.path.join(LOCKFILE).is_file();
    Project {
        manifests: vec![root.file(if locked { LOCKFILE } else { MANIFEST })],
        context: Context {
            direct,
            licensed_lockfile: locked,
            ..Context::default()
        },
    }
}

/// Each name `package.json` declares with its version range, in field order, then name order.
fn declared(text: &str) -> Vec<(String, String)> {
    let Ok(manifest) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    DECLARING
        .iter()
        .filter_map(|field| manifest[*field].as_object())
        .flatten()
        .map(|(name, range)| (name.clone(), range.as_str().unwrap_or_default().to_owned()))
        .collect()
}

fn parse(file: &str, text: &str, context: &Context) -> Vec<Dependency> {
    if file.ends_with(LOCKFILE) {
        locked(file, text, context)
    } else {
        unlocked(file, text)
    }
}

/// `package-lock.json` v2 and v3: every entry of `packages` but the root and linked
/// workspace members, named by the last `node_modules/` segment of its key.
fn locked(file: &str, text: &str, context: &Context) -> Vec<Dependency> {
    let Ok(lock) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    let mut found = Vec::new();
    for (key, entry) in lock["packages"].as_object().into_iter().flatten() {
        let Some((_, name)) = key.rsplit_once("node_modules/") else {
            continue;
        };
        if entry["link"] == true || !seen.insert((name.to_owned(), entry["version"].to_string())) {
            continue;
        }
        let (line, _) = line_after(text, 0, |line| line.starts_with(&format!("\"{key}\":")));
        found.push(Dependency {
            name: name.to_owned(),
            version: entry["version"].as_str().map(str::to_owned),
            direct: !key.contains("/node_modules/") && context.direct.contains(name),
            license: entry["license"].as_str().map(str::to_owned),
            file: file.to_owned(),
            line,
        });
    }
    found
}

/// `package.json` without a lockfile: what it declares, without versions.
fn unlocked(file: &str, text: &str) -> Vec<Dependency> {
    declared(text)
        .into_iter()
        .map(|(name, _)| {
            let (line, _) = line_after(text, 0, |line| line.starts_with(&format!("\"{name}\":")));
            Dependency {
                name,
                version: None,
                direct: true,
                license: None,
                file: file.to_owned(),
                line,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK: &str = r#"{
  "name": "ledger",
  "lockfileVersion": 3,
  "packages": {
    "": {
      "name": "ledger",
      "dependencies": {"left-pad": "^1.3.0"}
    },
    "node_modules/left-pad": {
      "version": "1.3.0",
      "license": "WTFPL"
    },
    "node_modules/left-pad/node_modules/tiny": {
      "version": "0.1.0",
      "license": "GPL-3.0-only"
    },
    "node_modules/workspace-a": {
      "resolved": "packages/a",
      "link": true
    }
  }
}
"#;

    #[test]
    fn locked_packages_carry_versions_licenses_and_lines() {
        let context = Context {
            direct: BTreeSet::from(["left-pad".to_owned()]),
            ..Context::default()
        };
        let found: Vec<_> = locked("package-lock.json", LOCK, &context)
            .into_iter()
            .map(|d| (d.name, d.version, d.direct, d.license, d.line))
            .collect();
        assert_eq!(
            found,
            [
                (
                    "left-pad".to_owned(),
                    Some("1.3.0".to_owned()),
                    true,
                    Some("WTFPL".to_owned()),
                    9
                ),
                (
                    "tiny".to_owned(),
                    Some("0.1.0".to_owned()),
                    false,
                    Some("GPL-3.0-only".to_owned()),
                    13
                ),
            ]
        );
    }

    #[test]
    fn without_a_lockfile_the_declared_dependencies_have_no_version() {
        let manifest = "{\n  \"dependencies\": {\n    \"lodash\": \"^4.17.21\"\n  },\n  \"devDependencies\": {\n    \"vitest\": \"^2.1.0\"\n  }\n}\n";
        let found: Vec<_> = unlocked("package.json", manifest)
            .into_iter()
            .map(|d| (d.name, d.version, d.line))
            .collect();
        assert_eq!(
            found,
            [
                ("lodash".to_owned(), None, 3),
                ("vitest".to_owned(), None, 6)
            ]
        );
    }
}
