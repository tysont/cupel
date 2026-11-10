// ABOUTME: Detection rules of the toolchain table: what must be true of a project root on disk for a
// ABOUTME: launcher or tool to be chosen (a file, here or above, its text, a package mention, or a JSON value).
use std::fs;
use std::path::{Path, PathBuf};

use globset::Glob;
use serde::Deserialize;
use serde_json::Value;

/// One detection rule; see the crate docs for the TOML forms.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Rule {
    /// The file exists in the root directory or, with `above`, in it or any directory above it
    /// (as `ESLint` finds its flat config from where it runs).
    File {
        file: String,
        #[serde(default)]
        above: bool,
    },
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
    /// The JSON file has a value at `path` (a string starting with `prefix`, or a string that
    /// does not name the package `lacks`, when given), in the root directory or, with `members`,
    /// in a member directory too.
    Json {
        json: String,
        path: Vec<String>,
        #[serde(default)]
        prefix: Option<String>,
        #[serde(default)]
        lacks: Option<String>,
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
            Self::File { file, above: false } => dirs.root.join(file).exists(),
            Self::File { file, above: true } => {
                dirs.root.ancestors().any(|dir| dir.join(file).exists())
            }
            Self::Contains { contains, file } => {
                fs::read_to_string(dirs.root.join(file)).is_ok_and(|text| text.contains(contains))
            }
            Self::Mentions { mentions, files } => mentioned(dirs, mentions, files),
            Self::Json {
                json,
                path,
                prefix,
                lacks,
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
                    let Some(value) = at else {
                        return false;
                    };
                    let text = value.as_str();
                    prefix
                        .as_ref()
                        .is_none_or(|prefix| text.is_some_and(|text| text.starts_with(prefix)))
                        && lacks
                            .as_ref()
                            .is_none_or(|name| text.is_some_and(|text| !names(text, name)))
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
            Self::File { file, above: false } => format!("file `{file}` exists"),
            Self::File { file, above: true } => {
                format!("file `{file}` exists in the root or a directory above it")
            }
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
                lacks,
                members,
            } => {
                let place = if *members {
                    " in the root or a member"
                } else {
                    ""
                };
                let value = prefix
                    .as_ref()
                    .map(|prefix| format!(" starting `{prefix}`"))
                    .into_iter()
                    .chain(
                        lacks
                            .as_ref()
                            .map(|name| format!(" that does not name `{name}`")),
                    )
                    .collect::<String>();
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

/// Whether a file of the root or a member whose name matches one of `files` (globs) names the
/// package `mentions`.
fn mentioned(dirs: &Dirs<'_>, mentions: &str, files: &[String]) -> bool {
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
        let cases = [
            ("dev = [\"pytest>=8\"]", "pytest"),
            ("[tool.pytest.ini_options]", "pytest"),
            ("pytest-benchmark", "pytest"),
            ("my_pytest", "pytest"),
            ("pytest-benchmark==4", "pytest-benchmark"),
            ("", "pytest"),
        ];
        let named = cases.map(|(text, package)| names(text, package));
        assert_eq!(named, [true, true, false, false, true, false]);
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

    /// A root directory with `file` holding `text`, and an empty directory `member` under it.
    fn root_with(member: &str, file: &str, text: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let member = dir.path().join(member);
        fs::create_dir_all(&member).unwrap();
        fs::write(dir.path().join(file), text).unwrap();
        (dir, member)
    }

    #[test]
    fn a_file_rule_with_above_holds_for_the_file_in_a_directory_above_the_root() {
        let (dir, member) = root_with("apps/web", "eslint.config.js", "export default [];\n");
        let rules: Vec<Rule> = toml::from_str::<std::collections::BTreeMap<String, Vec<Rule>>>(
            r#"detect = [
                { file = "eslint.config.js" },
                { file = "eslint.config.js", above = true },
                { file = "eslint.config.mjs", above = true },
            ]"#,
        )
        .unwrap()
        .remove("detect")
        .unwrap();
        let at = |root: &Path| {
            let dirs = Dirs { root, members: &[] };
            rules
                .iter()
                .map(|rule| rule.holds(&dirs))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            (at(&member), at(dir.path())),
            (vec![false, true, false], vec![true, true, false])
        );
    }

    #[test]
    fn json_rules_read_paths_prefixes_and_optionally_members() {
        let (dir, member) = root_with("m", "p.json", r#"{"pm": "pnpm@9"}"#);
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
            lacks: None,
            members,
        };
        let all = Rule::All {
            all: vec![json(&["pm"], None, false), json(&["nope"], None, false)],
        };
        let rules = [
            json(&["pm"], Some("pnpm@"), false),
            json(&["pm"], Some("yarn@"), false),
            json(&["deps", "x"], None, false),
            json(&["deps", "x"], None, true),
            all,
        ];
        let held = rules.map(|rule| rule.holds(&dirs));
        assert_eq!(held, [true, false, false, true, false]);
    }

    #[test]
    fn a_json_rule_with_lacks_holds_for_a_string_that_does_not_name_the_package() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{"scripts": {"test": "pnpm -r test", "bench": 1}}"#,
        )
        .unwrap();
        let dirs = Dirs {
            root: dir.path(),
            members: &[],
        };
        let lacking = |key: &str, name: &str| Rule::Json {
            json: "package.json".to_owned(),
            path: vec!["scripts".to_owned(), key.to_owned()],
            prefix: None,
            lacks: Some(name.to_owned()),
            members: false,
        };
        let rules = [
            lacking("test", "vitest"),
            lacking("test", "pnpm"),
            lacking("lint", "vitest"),
            lacking("bench", "vitest"),
        ];
        let held = rules.map(|rule| rule.holds(&dirs));
        assert_eq!(held, [true, false, false, false]);
    }
}
