// ABOUTME: Detection rules of the toolchain table: what must be true of a project root on disk for a
// ABOUTME: launcher or tool to be chosen (a file, its text, a package mention, or a JSON value).
use std::fs;
use std::path::{Path, PathBuf};

use globset::Glob;
use serde::Deserialize;
use serde_json::Value;

/// One detection rule; see the crate docs for the TOML forms.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Rule {
    /// The file exists in the root directory.
    File { file: String },
    /// The root directory's file `in` contains the text.
    Contains {
        contains: String,
        #[serde(rename = "in")]
        file: String,
    },
    /// A file whose name matches one of the `in` globs, in the root or a member directory,
    /// mentions the package name (not as part of a longer name).
    Mentions {
        mentions: String,
        #[serde(rename = "in")]
        files: Vec<String>,
    },
    /// The JSON file has a value at `path` (a string starting with `prefix`, when given), in
    /// the root directory or, with `members`, in a member directory too.
    Json {
        json: String,
        path: Vec<String>,
        #[serde(default)]
        prefix: Option<String>,
        #[serde(default)]
        members: bool,
    },
    /// Every rule holds.
    All { all: Vec<Rule> },
}

/// A project root on disk as rules see it: its directory and its members' directories.
pub(crate) struct Dirs<'a> {
    pub root: &'a Path,
    pub members: &'a [PathBuf],
}

impl Dirs<'_> {
    fn all(&self) -> impl Iterator<Item = &Path> {
        std::iter::once(self.root).chain(self.members.iter().map(PathBuf::as_path))
    }
}

impl Rule {
    pub(crate) fn holds(&self, dirs: &Dirs<'_>) -> bool {
        match self {
            Self::File { file } => dirs.root.join(file).exists(),
            Self::Contains { contains, file } => {
                fs::read_to_string(dirs.root.join(file)).is_ok_and(|text| text.contains(contains))
            }
            Self::Mentions { mentions, files } => {
                let globs: Vec<_> = files
                    .iter()
                    .filter_map(|glob| Glob::new(glob).ok())
                    .map(|glob| glob.compile_matcher())
                    .collect();
                dirs.all().any(|dir| {
                    fs::read_dir(dir)
                        .into_iter()
                        .flatten()
                        .flatten()
                        .filter(|entry| {
                            let name = entry.file_name();
                            globs.iter().any(|glob| glob.is_match(&name))
                        })
                        .filter_map(|entry| fs::read_to_string(entry.path()).ok())
                        .any(|text| names(&text, mentions))
                })
            }
            Self::Json {
                json,
                path,
                prefix,
                members,
            } => {
                let found = |dir: &Path| {
                    let Some(value) = fs::read_to_string(dir.join(json))
                        .ok()
                        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                    else {
                        return false;
                    };
                    let at = path.iter().try_fold(&value, |value, key| value.get(key));
                    match (at, prefix) {
                        (None, _) => false,
                        (Some(_), None) => true,
                        (Some(value), Some(prefix)) => {
                            value.as_str().is_some_and(|text| text.starts_with(prefix))
                        }
                    }
                };
                if *members {
                    dirs.all().any(found)
                } else {
                    found(dirs.root)
                }
            }
            Self::All { all } => all.iter().all(|rule| rule.holds(dirs)),
        }
    }

    /// The rule in words, for `describe language`.
    pub fn describe(&self) -> String {
        match self {
            Self::File { file } => format!("file `{file}` exists"),
            Self::Contains { contains, file } => format!("`{file}` contains `{contains}`"),
            Self::Mentions { mentions, files } => format!(
                "{} in the root or a member mentions `{mentions}`",
                files
                    .iter()
                    .map(|file| format!("`{file}`"))
                    .collect::<Vec<_>>()
                    .join(" or ")
            ),
            Self::Json {
                json,
                path,
                prefix,
                members,
            } => {
                let place = if *members {
                    " in the root or a member"
                } else {
                    ""
                };
                let value = prefix
                    .as_ref()
                    .map_or_else(String::new, |prefix| format!(" starting `{prefix}`"));
                format!("`{json}`{place} has `{}`{value}", path.join("."))
            }
            Self::All { all } => all
                .iter()
                .map(Self::describe)
                .collect::<Vec<_>>()
                .join(", and "),
        }
    }
}

/// Whether `text` contains `name` as a whole package name: not preceded or followed by a
/// character that can be part of one, so `pytest-benchmark` does not mention `pytest`.
pub(crate) fn names(text: &str, name: &str) -> bool {
    let part_of_name = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
    text.match_indices(name).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + name.len()..].chars().next();
        !before.is_some_and(part_of_name) && !after.is_some_and(part_of_name)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_matches_whole_package_names_only() {
        assert!(names("dev = [\"pytest>=8\"]", "pytest"));
        assert!(names("[tool.pytest.ini_options]", "pytest"));
        assert!(!names("pytest-benchmark", "pytest"));
        assert!(!names("my_pytest", "pytest"));
        assert!(names("pytest-benchmark==4", "pytest-benchmark"));
        assert!(!names("", "pytest"));
    }

    #[test]
    fn rules_parse_from_their_toml_forms_and_unknown_keys_are_errors() {
        #[derive(Deserialize)]
        struct Rules {
            detect: Vec<Rule>,
        }
        let rules: Rules = toml::from_str(
            r#"detect = [
                { file = "a" },
                { contains = "x", in = "b" },
                { mentions = "m", in = ["c*"] },
                { json = "p.json", path = ["k"], prefix = "v@", members = true },
                { all = [{ file = "a" }, { file = "b" }] },
            ]"#,
        )
        .unwrap();
        assert_eq!(rules.detect.len(), 5);
        assert!(toml::from_str::<Rules>("detect = [{ file = \"a\", typo = 1 }]").is_err());
    }

    #[test]
    fn json_rules_read_paths_prefixes_and_optionally_members() {
        let dir = tempfile::tempdir().unwrap();
        let member = dir.path().join("m");
        fs::create_dir_all(&member).unwrap();
        fs::write(dir.path().join("p.json"), r#"{"pm": "pnpm@9"}"#).unwrap();
        fs::write(member.join("p.json"), r#"{"deps": {"x": "1"}}"#).unwrap();
        let members = [member];
        let dirs = Dirs {
            root: dir.path(),
            members: &members,
        };
        let json = |path: &[&str], prefix: Option<&str>, members: bool| Rule::Json {
            json: "p.json".to_owned(),
            path: path.iter().map(|key| (*key).to_owned()).collect(),
            prefix: prefix.map(str::to_owned),
            members,
        };
        assert!(json(&["pm"], Some("pnpm@"), false).holds(&dirs));
        assert!(!json(&["pm"], Some("yarn@"), false).holds(&dirs));
        assert!(!json(&["deps", "x"], None, false).holds(&dirs));
        assert!(json(&["deps", "x"], None, true).holds(&dirs));
        let all = Rule::All {
            all: vec![json(&["pm"], None, false), json(&["nope"], None, false)],
        };
        assert!(!all.holds(&dirs));
    }
}
