// ABOUTME: The Python ecosystem of `tool.footprint` (ADR-103): `uv.lock`, else `poetry.lock`, else `requirements*.txt`;
// ABOUTME: direct names from `pyproject.toml`, and licenses from an in-project virtual environment's package metadata.
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use cupel_core::Dependency;

use super::{Context, Ecosystem, Project, Root, line_after};

pub static PYTHON: Ecosystem = Ecosystem {
    langs: &["python"],
    osv: "PyPI",
    discover,
    parse,
    normalize,
};

/// The lockfiles read, the first present winning.
const LOCKFILES: [&str; 2] = ["uv.lock", "poetry.lock"];

/// Where an in-project virtual environment lives.
const VENVS: [&str; 2] = [".venv", "venv"];

/// A name as PEP 503 compares it: lowercase, runs of `-`, `_`, `.` as one `-`.
fn normalize(name: &str) -> String {
    let mut normalized = String::new();
    for c in name.chars() {
        if "-_.".contains(c) {
            if !normalized.ends_with('-') {
                normalized.push('-');
            }
        } else {
            normalized.push(c.to_ascii_lowercase());
        }
    }
    normalized
}

fn discover(root: &Root<'_>) -> Project {
    let manifests = match LOCKFILES.iter().find(|lock| root.path.join(lock).is_file()) {
        Some(lock) => vec![root.file(lock)],
        None => requirements(&root.path)
            .into_iter()
            .map(|name| root.file(&name))
            .collect(),
    };
    let direct = std::fs::read_to_string(root.path.join("pyproject.toml"))
        .map(|text| declared(&text))
        .unwrap_or_default();
    Project {
        manifests,
        context: Context {
            direct,
            licenses: venv_licenses(&root.path),
            ..Context::default()
        },
    }
}

/// The `requirements*.txt` files of `dir`, by name.
fn requirements(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| {
            name.starts_with("requirements")
                && Path::new(name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("txt"))
        })
        .collect();
    names.sort();
    names
}

/// The normalized names `pyproject.toml` declares: `[project] dependencies` and
/// `[tool.poetry.dependencies]` (but `python`).
fn declared(text: &str) -> BTreeSet<String> {
    let Ok(pyproject) = text.parse::<toml::Table>() else {
        return BTreeSet::new();
    };
    let project = pyproject
        .get("project")
        .and_then(|project| project.get("dependencies"))
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .map(requirement_name);
    let poetry = pyproject
        .get("tool")
        .and_then(|tool| tool.get("poetry"))
        .and_then(|poetry| poetry.get("dependencies"))
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(|table| table.keys().cloned())
        .filter(|name| name != "python");
    project.chain(poetry).map(|name| normalize(&name)).collect()
}

/// The distribution name a PEP 508 requirement starts with.
fn requirement_name(requirement: &str) -> String {
    requirement
        .split(|c: char| "<>=!~;[( @".contains(c))
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned()
}

fn parse(file: &str, text: &str, context: &Context) -> Vec<Dependency> {
    let name = file.rsplit('/').next().unwrap_or(file);
    if LOCKFILES.contains(&name) {
        locked(file, text, context)
    } else {
        required(file, text, context)
    }
}

/// `uv.lock` and `poetry.lock`: every `[[package]]` but the project's own (an editable,
/// virtual, or workspace source).
fn locked(file: &str, text: &str, context: &Context) -> Vec<Dependency> {
    let Ok(lock) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    let mut from = 0;
    let mut found = Vec::new();
    for package in lock
        .get("package")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_table)
    {
        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let (line, next) = line_after(text, from, |line| line == format!("name = \"{name}\""));
        from = next;
        let own = package
            .get("source")
            .and_then(toml::Value::as_table)
            .is_some_and(|source| {
                ["editable", "virtual", "workspace"]
                    .iter()
                    .any(|key| source.contains_key(*key))
            });
        if own {
            continue;
        }
        let version = package
            .get("version")
            .and_then(toml::Value::as_str)
            .map(str::to_owned);
        found.push(Dependency {
            direct: context.direct.contains(&normalize(&name)),
            license: context.license(&normalize(&name), version.as_deref()),
            version,
            name,
            file: file.to_owned(),
            line,
        });
    }
    found
}

/// A requirements file: each requirement line, versioned when pinned with `==`.
fn required(file: &str, text: &str, context: &Context) -> Vec<Dependency> {
    text.lines()
        .enumerate()
        .filter_map(|(index, raw)| {
            let line = raw.split('#').next().unwrap_or_default().trim();
            if line.is_empty() || line.starts_with('-') {
                return None;
            }
            let spec = line.split(';').next().unwrap_or_default().trim();
            let name = requirement_name(spec);
            if name.is_empty() {
                return None;
            }
            let version = spec
                .split_once("==")
                .map(|(_, version)| version.trim().to_owned())
                .filter(|version| !version.contains(['*', ',']));
            Some(Dependency {
                license: context.license(&normalize(&name), version.as_deref()),
                name,
                version,
                direct: true,
                file: file.to_owned(),
                line: u32::try_from(index + 1).unwrap_or(u32::MAX),
            })
        })
        .collect()
}

/// Licenses of the packages installed in the root's `.venv/` or `venv/`, by normalized
/// `name@version` and name; `None` without one.
fn venv_licenses(root: &Path) -> Option<BTreeMap<String, String>> {
    let venv = VENVS
        .iter()
        .map(|dir| root.join(dir))
        .find(|dir| dir.is_dir())?;
    let mut licenses = BTreeMap::new();
    let lib = venv.join("lib");
    for python in std::fs::read_dir(&lib).into_iter().flatten().flatten() {
        let site = python.path().join("site-packages");
        for entry in std::fs::read_dir(&site).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "dist-info") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(path.join("METADATA")) else {
                continue;
            };
            if let Some((name, version, license)) = metadata(&text) {
                let name = normalize(&name);
                licenses.insert(format!("{name}@{version}"), license.clone());
                licenses.insert(name, license);
            }
        }
    }
    Some(licenses)
}

/// A `METADATA` file's name, version, and license: `License-Expression`, else a one-line
/// `License` that is not `UNKNOWN`.
fn metadata(text: &str) -> Option<(String, String, String)> {
    let headers = text.split("\n\n").next().unwrap_or_default();
    let field = |key: &str| {
        headers
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{key}: ")))
            .map(str::trim)
            .filter(|value| !value.is_empty() && *value != "UNKNOWN")
            .map(str::to_owned)
    };
    let license = field("License-Expression").or_else(|| field("License"))?;
    Some((field("Name")?, field("Version")?, license))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_normalize_as_pep_503_says() {
        let normalized: Vec<String> = ["Typing_Extensions", "zope.interface", "a__b"]
            .into_iter()
            .map(normalize)
            .collect();
        assert_eq!(normalized, ["typing-extensions", "zope-interface", "a-b"]);
    }

    #[test]
    fn requirements_are_versioned_only_when_pinned() {
        let text = "# comment\nrequests==2.31.0  # pinned\nflask>=2\n-r other.txt\nclick[extra]==8.1.7; python_version > '3'\n";
        let found: Vec<_> = required("requirements.txt", text, &Context::default())
            .into_iter()
            .map(|d| (d.name, d.version, d.line))
            .collect();
        assert_eq!(
            found,
            [
                ("requests".to_owned(), Some("2.31.0".to_owned()), 2),
                ("flask".to_owned(), None, 3),
                ("click".to_owned(), Some("8.1.7".to_owned()), 5),
            ]
        );
    }

    #[test]
    fn uv_lock_leaves_out_the_project_and_marks_what_pyproject_declares() {
        let lock = "version = 1\n\n[[package]]\nname = \"ledger\"\nversion = \"0.1.0\"\nsource = { editable = \".\" }\n\n[[package]]\nname = \"Requests\"\nversion = \"2.31.0\"\nsource = { registry = \"https://pypi.org/simple\" }\n\n[[package]]\nname = \"idna\"\nversion = \"3.7\"\nsource = { registry = \"https://pypi.org/simple\" }\n";
        let context = Context {
            direct: declared("[project]\nname = \"ledger\"\ndependencies = [\"requests>=2\"]\n"),
            ..Context::default()
        };
        let found: Vec<_> = locked("uv.lock", lock, &context)
            .into_iter()
            .map(|d| (d.name, d.direct, d.line))
            .collect();
        assert_eq!(
            found,
            [
                ("Requests".to_owned(), true, 9),
                ("idna".to_owned(), false, 14)
            ]
        );
    }

    #[test]
    fn metadata_prefers_the_license_expression() {
        let text = "Metadata-Version: 2.4\nName: Tiny\nVersion: 1.0\nLicense: GPL\nLicense-Expression: GPL-3.0-only\n\nBody\n";
        assert_eq!(
            metadata(text),
            Some((
                "Tiny".to_owned(),
                "1.0".to_owned(),
                "GPL-3.0-only".to_owned()
            ))
        );
        assert_eq!(metadata("Name: A\nVersion: 1\nLicense: UNKNOWN\n"), None);
    }
}
