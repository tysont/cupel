// ABOUTME: Known vulnerabilities from a directory of OSV advisories (`[lang.<id>] advisories`, ADR-103 §3): which
// ABOUTME: dependencies an advisory affects by ecosystem, name, and version, compared by dot-separated parts.
use std::cmp::Ordering;
use std::path::Path;

use cupel_core::{Dependency, ProduceCtx, Result, Vulnerability};
use serde_json::Value;

use super::{ADVISORIES_KEY, Ecosystem, unreadable};

/// The vulnerabilities of `dependencies` the configured advisories name; `None` when the
/// language configures none.
pub fn configured(
    ctx: &ProduceCtx,
    ecosystem: &Ecosystem,
    dependencies: &[Dependency],
) -> Result<Option<Vec<Vulnerability>>> {
    let Some(dir) = ctx.config.get(ADVISORIES_KEY).and_then(Value::as_str) else {
        return Ok(None);
    };
    let dir = ctx.ctx.root.join(dir);
    let mut found = Vec::new();
    for advisory in advisories(&dir)? {
        for dependency in dependencies {
            if let Some(hit) = affected(&advisory, ecosystem, dependency) {
                found.push(Vulnerability {
                    id: advisory["id"].as_str().unwrap_or_default().to_owned(),
                    package: dependency.name.clone(),
                    version: dependency.version.clone().unwrap_or_default(),
                    summary: summary(&advisory),
                    fixed: hit.fixed,
                });
            }
        }
    }
    Ok(Some(found))
}

/// Every `*.json` file in `dir`, by name, parsed.
fn advisories(dir: &Path) -> Result<Vec<Value>> {
    let entries = std::fs::read_dir(dir).map_err(|error| unreadable(dir, &error.to_string()))?;
    let mut paths: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|path| {
            let text = std::fs::read_to_string(path)
                .map_err(|error| unreadable(path, &error.to_string()))?;
            serde_json::from_str(&text).map_err(|error| unreadable(path, &error.to_string()))
        })
        .collect()
}

/// The advisory's summary, else the first line of its details.
fn summary(advisory: &Value) -> String {
    advisory["summary"]
        .as_str()
        .or_else(|| advisory["details"].as_str()?.lines().next())
        .unwrap_or_default()
        .to_owned()
}

/// An advisory affects a dependency.
#[derive(Debug, PartialEq, Eq)]
pub struct Hit {
    /// The first version that fixes it, when the advisory names one.
    pub fixed: Option<String>,
}

/// Whether `advisory` affects `dependency`, with the first fixed version of the range that
/// holds it; `None` when it is not affected. A dependency without a version never is.
pub fn affected(advisory: &Value, ecosystem: &Ecosystem, dependency: &Dependency) -> Option<Hit> {
    let version = dependency.version.as_deref()?;
    let name = (ecosystem.normalize)(&dependency.name);
    advisory["affected"]
        .as_array()?
        .iter()
        .filter(|affected| {
            affected["package"]["ecosystem"] == ecosystem.osv
                && affected["package"]["name"]
                    .as_str()
                    .is_some_and(|named| (ecosystem.normalize)(named) == name)
        })
        .find_map(|affected| {
            let listed = affected["versions"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|listed| listed.as_str() == Some(version));
            if listed {
                return Some(Hit {
                    fixed: first_fixed(affected),
                });
            }
            affected["ranges"]
                .as_array()?
                .iter()
                .find_map(|range| in_range(range, version))
        })
}

/// The first `fixed` event of any range of `affected`.
fn first_fixed(affected: &Value) -> Option<String> {
    affected["ranges"]
        .as_array()?
        .iter()
        .flat_map(|range| range["events"].as_array().into_iter().flatten())
        .find_map(|event| event["fixed"].as_str().map(str::to_owned))
}

/// A hit when `version` is in the range: after an `introduced` and before the next `fixed` (or
/// at most the next `last_affected`), or after the last `introduced` with nothing after it.
fn in_range(range: &Value, version: &str) -> Option<Hit> {
    let mut introduced: Option<&str> = None;
    for event in range["events"].as_array()? {
        if let Some(start) = event["introduced"].as_str() {
            introduced = Some(start);
        } else if let Some(start) = introduced.take()
            && let Some(hit) = ended(event, version, start)
        {
            return Some(hit);
        }
    }
    introduced
        .filter(|start| at_or_after(version, start))
        .map(|_| Hit { fixed: None })
}

/// A hit when the interval from `start` that `event` ends (a `fixed` or `last_affected`
/// event) holds `version`.
fn ended(event: &Value, version: &str, start: &str) -> Option<Hit> {
    if !at_or_after(version, start) {
        return None;
    }
    if let Some(fixed) = event["fixed"].as_str() {
        return (compare_versions(version, fixed) == Ordering::Less).then(|| Hit {
            fixed: Some(fixed.to_owned()),
        });
    }
    let last = event["last_affected"].as_str()?;
    (compare_versions(version, last) != Ordering::Greater).then_some(Hit { fixed: None })
}

/// Whether `version` is at or after `start`; `0` starts every range.
fn at_or_after(version: &str, start: &str) -> bool {
    start == "0" || compare_versions(version, start) != Ordering::Less
}

/// Compares two versions part by part, splitting on `.`, `-`, and `+` after a leading `v`:
/// numeric parts as numbers, others as text, a missing part first.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let parts = |version: &str| -> Vec<String> {
        version
            .trim_start_matches('v')
            .split(['.', '-', '+'])
            .map(str::to_owned)
            .collect()
    };
    let (a, b) = (parts(a), parts(b));
    for (x, y) in a.iter().zip(&b) {
        let ordering = match (x.parse::<u64>(), y.parse::<u64>()) {
            (Ok(x), Ok(y)) => x.cmp(&y),
            _ => x.cmp(y),
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    a.len().cmp(&b.len())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::footprint::cargo::CARGO;
    use crate::footprint::python::PYTHON;

    fn dependency(name: &str, version: &str) -> Dependency {
        Dependency {
            name: name.to_owned(),
            version: Some(version.to_owned()),
            direct: true,
            license: None,
            file: "Cargo.lock".to_owned(),
            line: 1,
        }
    }

    fn fixed(version: &str) -> Hit {
        Hit {
            fixed: Some(version.to_owned()),
        }
    }

    fn advisory(affected: Value) -> Value {
        let mut advisory = json!({"id": "RUSTSEC-2099-0001", "summary": "Bad"});
        advisory["affected"] = Value::Array(vec![affected]);
        advisory
    }

    #[test]
    fn versions_compare_by_numeric_parts_then_text() {
        assert_eq!(compare_versions("1.10.0", "1.9.9"), Ordering::Greater);
        assert_eq!(compare_versions("v1.2.0", "1.2.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.2", "1.2.0"), Ordering::Less);
        assert_eq!(
            compare_versions("1.0.0-alpha", "1.0.0-beta"),
            Ordering::Less
        );
    }

    #[test]
    fn a_range_affects_versions_from_introduced_up_to_fixed() {
        let ranged = advisory(json!({
            "package": {"ecosystem": "crates.io", "name": "smallvec"},
            "ranges": [{"type": "SEMVER", "events": [{"introduced": "0.6.3"}, {"fixed": "0.6.10"}, {"introduced": "1.0.0"}, {"fixed": "1.6.1"}]}],
        }));
        let at = |version| affected(&ranged, &CARGO, &dependency("smallvec", version));
        assert_eq!(at("0.6.2"), None);
        assert_eq!(at("0.6.3"), Some(fixed("0.6.10")));
        assert_eq!(at("0.6.10"), None);
        assert_eq!(at("1.6.0"), Some(fixed("1.6.1")));
        assert_eq!(at("1.6.1"), None);
        let open = advisory(json!({
            "package": {"ecosystem": "crates.io", "name": "smallvec"},
            "ranges": [{"type": "SEMVER", "events": [{"introduced": "0"}]}],
        }));
        assert_eq!(
            affected(&open, &CARGO, &dependency("smallvec", "9.9.9")),
            Some(Hit { fixed: None })
        );
        let last = advisory(json!({
            "package": {"ecosystem": "crates.io", "name": "smallvec"},
            "ranges": [{"type": "SEMVER", "events": [{"introduced": "0"}, {"last_affected": "1.2.0"}]}],
        }));
        assert_eq!(
            affected(&last, &CARGO, &dependency("smallvec", "1.2.0")),
            Some(Hit { fixed: None })
        );
        assert_eq!(
            affected(&last, &CARGO, &dependency("smallvec", "1.2.1")),
            None
        );
    }

    #[test]
    fn listed_versions_names_and_ecosystems_must_match() {
        let listed = advisory(json!({
            "package": {"ecosystem": "PyPI", "name": "Typing_Extensions"},
            "versions": ["4.0.0"],
            "ranges": [{"type": "ECOSYSTEM", "events": [{"introduced": "4.0.0"}, {"fixed": "4.0.1"}]}],
        }));
        assert_eq!(
            affected(&listed, &PYTHON, &dependency("typing-extensions", "4.0.0")),
            Some(fixed("4.0.1"))
        );
        assert_eq!(
            affected(&listed, &CARGO, &dependency("typing-extensions", "4.0.0")),
            None
        );
        let mut unversioned = dependency("typing-extensions", "4.0.0");
        unversioned.version = None;
        assert_eq!(affected(&listed, &PYTHON, &unversioned), None);
    }
}
