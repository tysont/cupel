// ABOUTME: Paths a run never considers: `[scope] exclude` globs (gitignore-style: a pattern without a
// ABOUTME: slash matches a name at any depth) plus directory names every run skips (ADR-065).
use std::borrow::Cow;
use std::fmt;

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};

/// Directory names excluded at any depth in every run: vendored dependencies, build output,
/// and Cupel's own working directory. Each language's generated directories below its project
/// roots are skipped by the walk as well (`ProjectInfo.skip_dirs`).
pub const ALWAYS_EXCLUDED: [&str; 4] = [".cupel", "node_modules", "target", "vendor"];

/// Compiled exclusion patterns. A path is excluded when it, or a directory above it, matches
/// a pattern or is named in [`ALWAYS_EXCLUDED`].
#[derive(Clone, Default)]
pub struct Exclude {
    globs: PathGlobs,
}

impl Exclude {
    /// Compiles `patterns` with [`PathGlobs`]' rules. The error names the first pattern that is
    /// not a valid glob.
    pub fn new(patterns: Vec<String>) -> Result<Self, String> {
        Ok(Self {
            globs: PathGlobs::new(patterns, "exclude")?,
        })
    }

    /// The patterns as written.
    pub fn patterns(&self) -> &[String] {
        self.globs.patterns()
    }

    /// Whether `path` (relative to the repository root, `/`-separated, a file or a directory)
    /// is excluded.
    pub fn excludes(&self, path: &str) -> bool {
        path.split('/').any(|part| ALWAYS_EXCLUDED.contains(&part)) || self.globs.matches(path)
    }
}

/// Gitignore-style path globs relative to the repository root, as `[scope] exclude` writes
/// them and attributions name files (ADR-151).
#[derive(Clone)]
pub struct PathGlobs {
    patterns: Vec<String>,
    set: GlobSet,
}

impl PathGlobs {
    /// Compiles `patterns`. A pattern containing `/` (other than a trailing one) is anchored at
    /// the repository root; any other pattern matches a name at any depth. `*` never crosses a
    /// `/`; `**` does. The error names the first pattern that is not a valid glob, as a `what`
    /// pattern.
    pub fn new(patterns: Vec<String>, what: &str) -> Result<Self, String> {
        let mut builder = GlobSetBuilder::new();
        for pattern in &patterns {
            let trimmed = pattern.trim_end_matches('/');
            let anchored = match trimmed.strip_prefix('/') {
                Some(rooted) => rooted.to_owned(),
                None if trimmed.contains('/') => trimmed.to_owned(),
                None => format!("**/{trimmed}"),
            };
            let glob = GlobBuilder::new(&anchored)
                .literal_separator(true)
                .build()
                .map_err(|error| format!("invalid {what} pattern `{pattern}`: {error}"))?;
            builder.add(glob);
        }
        let set = builder
            .build()
            .map_err(|error| format!("invalid {what} patterns: {error}"))?;
        Ok(Self { patterns, set })
    }

    /// The patterns as written.
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }

    /// Whether `path` (relative to the repository root, `/`-separated, a file or a directory),
    /// or a directory above it, matches a pattern.
    pub fn matches(&self, path: &str) -> bool {
        let mut prefix = String::new();
        for part in path.split('/') {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            if self.set.is_match(&prefix) {
                return true;
            }
        }
        false
    }
}

impl Default for PathGlobs {
    fn default() -> Self {
        Self {
            patterns: Vec::new(),
            set: GlobSet::empty(),
        }
    }
}

impl PartialEq for Exclude {
    fn eq(&self, other: &Self) -> bool {
        self.patterns() == other.patterns()
    }
}

impl Eq for Exclude {}

impl fmt::Debug for Exclude {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Exclude").field(&self.patterns()).finish()
    }
}

impl Serialize for Exclude {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.patterns().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Exclude {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(Vec::<String>::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

impl JsonSchema for Exclude {
    fn schema_name() -> Cow<'static, str> {
        "Exclude".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "array",
            "description": "Gitignore-style globs relative to the repository root: a pattern with a `/` is anchored at the root, one without matches a name at any depth; an excluded directory excludes everything below it.",
            "items": {"type": "string"}
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_exclusion_shows_and_compares_as_its_patterns() {
        let exclude = Exclude::new(vec!["crates/a/**".to_owned()]).unwrap();
        assert_eq!(
            (
                format!("{exclude:?}"),
                exclude == Exclude::new(vec!["crates/a/**".to_owned()]).unwrap(),
                exclude.excludes("crates/a/src/lib.rs"),
                exclude.excludes("crates/b/target/x"),
                exclude.excludes("crates/b/src"),
            ),
            (
                r#"Exclude(["crates/a/**"])"#.to_owned(),
                true,
                true,
                true,
                false
            )
        );
    }
}
