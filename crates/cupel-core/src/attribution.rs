// ABOUTME: Test attributions (ADR-151): `[lang.<id>.attribution.<name>]` as read and checked at load, naming tests by
// ABOUTME: ID glob and the files they are attributed to for staleness instead of every line their coverage holds.
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde_json::Value;

use crate::config::Table;
use crate::config::gate::is_segment;
use crate::exclude::PathGlobs;
use crate::id::LangId;

/// The `[lang.<id>]` key holding the language's attributions by name.
pub const ATTRIBUTION_KEY: &str = "attribution";

/// The keys an attribution takes, in the order errors list them.
const KEYS: [&str; 2] = ["tests", "files"];

/// Tests attributed, for staleness, to the files an author names rather than to the lines
/// their coverage holds: a test that spawns the repository's own binary covers most of it.
#[derive(Clone)]
pub struct Attribution {
    /// The attribution's name in `[lang.<id>.attribution.<name>]`.
    pub name: String,
    tests: GlobSet,
    files: PathGlobs,
}

impl Attribution {
    /// Whether the attribution names the test `id` (relative to the repository root, as the
    /// envelope shows it).
    pub fn names(&self, id: &str) -> bool {
        self.tests.is_match(id)
    }

    /// Whether `path` (relative to the repository root) is one of the attributed files.
    pub fn covers(&self, path: &str) -> bool {
        self.files.matches(path)
    }
}

/// The attributions of `lang`'s `[lang.<id>]` table, in name order; none when it has no
/// `attribution` key. The problem otherwise, naming the table (`[lang.rust.attribution]`).
pub fn attributions(lang: &LangId, table: &Table) -> Result<Vec<Attribution>, String> {
    let at = format!("[lang.{lang}.{ATTRIBUTION_KEY}]");
    let Some(value) = table.get(ATTRIBUTION_KEY) else {
        return Ok(Vec::new());
    };
    let Value::Object(by_name) = value else {
        return Err(format!(
            "invalid `{at}`: expected a table of attributions by name"
        ));
    };
    by_name
        .iter()
        .map(|(name, value)| {
            if !is_segment(name) {
                return Err(format!(
                    "invalid attribution name `{name}` in `{at}`: must match `[a-z][a-z0-9_-]*`"
                ));
            }
            Attribution::from_value(name, value)
                .map_err(|reason| format!("invalid attribution `{name}` in `{at}`: {reason}"))
        })
        .collect()
}

impl Attribution {
    fn from_value(name: &str, value: &Value) -> Result<Self, String> {
        let Value::Object(table) = value else {
            return Err("expected a table".to_owned());
        };
        if let Some(unknown) = table.keys().find(|key| !KEYS.contains(&key.as_str())) {
            let expected: Vec<String> = KEYS.iter().map(|key| format!("`{key}`")).collect();
            return Err(format!(
                "unknown key `{unknown}`; expected one of {}",
                expected.join(", ")
            ));
        }
        let mut tests = GlobSetBuilder::new();
        for pattern in strings(table, "tests")? {
            let glob = GlobBuilder::new(&pattern)
                .literal_separator(false)
                .build()
                .map_err(|error| format!("invalid test pattern `{pattern}`: {error}"))?;
            tests.add(glob);
        }
        Ok(Self {
            name: name.to_owned(),
            tests: tests
                .build()
                .map_err(|error| format!("invalid test patterns: {error}"))?,
            files: PathGlobs::new(strings(table, "files")?, "file")?,
        })
    }
}

/// The non-empty list of strings at `key`.
fn strings(table: &Table, key: &str) -> Result<Vec<String>, String> {
    let found: Option<Vec<String>> = match table.get(key) {
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| item.as_str().map(str::to_owned))
            .collect(),
        _ => None,
    };
    found
        .filter(|items| !items.is_empty())
        .ok_or_else(|| format!("`{key}` must be a non-empty list of strings"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn read(value: &Value) -> Result<Vec<Attribution>, String> {
        let Value::Object(table) = json!({ "attribution": value }) else {
            unreachable!()
        };
        attributions(&LangId::from("rust"), &table)
    }

    #[test]
    fn an_attribution_names_tests_by_id_glob_and_covers_the_files_under_its_paths() {
        let found = read(&json!({"binary": {
            "tests": ["cupel-cli::*", "tests/e2e/*"],
            "files": ["crates/cupel-cli", "fixtures/**/*.toml"],
        }}))
        .unwrap();
        let binary = &found[0];
        assert_eq!(
            (
                binary.name.as_str(),
                binary.names("cupel-cli::budget a_run_records_its_durations"),
                binary.names("tests/e2e/test_cli.py::test_runs[1]"),
                binary.names("cupel-core::scheduler a_plan"),
                binary.covers("crates/cupel-cli/src/main.rs"),
                binary.covers("fixtures/a/b/cupel.toml"),
                binary.covers("crates/cupel-core/src/lib.rs"),
            ),
            ("binary", true, true, false, true, true, false)
        );
    }

    #[test]
    fn a_table_without_an_attribution_has_none() {
        assert!(
            attributions(&LangId::from("rust"), &Table::new())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_malformed_attribution_is_refused_naming_its_table_and_the_problem() {
        let errors: Vec<String> = [
            json!(["binary"]),
            json!({"Binary": {"tests": ["a"], "files": ["b"]}}),
            json!({"binary": {"tests": ["a"]}}),
            json!({"binary": {"tests": [], "files": ["b"]}}),
            json!({"binary": {"tests": ["a"], "files": ["b"], "owns": ["c"]}}),
            json!({"binary": {"tests": ["a["], "files": ["b"]}}),
        ]
        .into_iter()
        .map(|value| read(&value).err().expect("the table is refused"))
        .collect();
        assert_eq!(
            errors,
            [
                "invalid `[lang.rust.attribution]`: expected a table of attributions by name",
                "invalid attribution name `Binary` in `[lang.rust.attribution]`: must match `[a-z][a-z0-9_-]*`",
                "invalid attribution `binary` in `[lang.rust.attribution]`: `files` must be a non-empty list of strings",
                "invalid attribution `binary` in `[lang.rust.attribution]`: `tests` must be a non-empty list of strings",
                "invalid attribution `binary` in `[lang.rust.attribution]`: unknown key `owns`; expected one of `tests`, `files`",
                "invalid attribution `binary` in `[lang.rust.attribution]`: invalid test pattern `a[`: error parsing glob 'a[': unclosed character class; missing ']'",
            ]
        );
    }
}
