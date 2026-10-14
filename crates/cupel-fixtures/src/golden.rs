// ABOUTME: Golden files: where they live, how they are compared and rewritten, and the failure
// ABOUTME: messages that show a unified diff and the update command (ADR-003 §3–6).
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use cupel_core::Envelope;
use serde_json::{Map, Value};
use similar::TextDiff;

/// Whether a golden check compares against the stored file or rewrites it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoldenMode {
    Compare,
    Update,
}

impl GoldenMode {
    /// The mode selected by an `UPDATE_GOLDENS` value: `1` updates, anything else compares.
    pub fn parse(value: Option<&str>) -> Self {
        if value == Some("1") {
            Self::Update
        } else {
            Self::Compare
        }
    }

    /// The mode selected by the process's `UPDATE_GOLDENS` variable.
    pub fn from_env() -> Self {
        Self::parse(std::env::var("UPDATE_GOLDENS").ok().as_deref())
    }
}

/// Why a golden check failed. Every message about a file names it and the update command.
#[derive(Debug, thiserror::Error)]
pub enum GoldenError {
    #[error("golden name {0:?} is not a relative, slash-separated path without an extension")]
    InvalidName(String),
    #[error(
        "golden {path} does not exist\nrun UPDATE_GOLDENS=1 make test to create it from the actual output:\n{actual}"
    )]
    Missing { path: String, actual: String },
    #[error(
        "golden {path} does not match the actual output\n{diff}\nrun UPDATE_GOLDENS=1 make test to accept the actual output"
    )]
    Mismatch { path: String, diff: String },
    #[error(
        "golden {path} is not valid JSON ({source})\nrun UPDATE_GOLDENS=1 make test to replace it with the actual output"
    )]
    Malformed {
        path: String,
        source: serde_json::Error,
    },
    #[error("golden {path}: {source}")]
    Io { path: String, source: io::Error },
}

/// A directory of golden files and the mode they are checked in.
#[derive(Debug, Clone)]
pub struct Goldens {
    dir: PathBuf,
    mode: GoldenMode,
}

impl Goldens {
    pub fn new(dir: impl Into<PathBuf>, mode: GoldenMode) -> Self {
        Self {
            dir: dir.into(),
            mode,
        }
    }

    /// This crate's `golden/` directory, in the mode `UPDATE_GOLDENS` selects.
    pub fn from_env() -> Self {
        Self::new(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("golden"),
            GoldenMode::from_env(),
        )
    }

    /// The directory the goldens live in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The file a golden name maps to: `<dir>/<name>.json`. Names are slash-separated relative
    /// paths such as `cli/describe` or `static.complexity/rust`.
    pub fn path(&self, name: &str) -> Result<PathBuf, GoldenError> {
        let valid = !name.is_empty()
            && name.split('/').all(|part| {
                !part.is_empty()
                    && part != "."
                    && part != ".."
                    && !Path::new(part)
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
            });
        if !valid {
            return Err(GoldenError::InvalidName(name.to_owned()));
        }
        Ok(self.dir.join(format!("{name}.json")))
    }

    /// Compares `actual` with the golden `name`, or rewrites the golden in update mode.
    pub fn check(&self, name: &str, actual: &Value) -> Result<(), GoldenError> {
        let path = self.path(name)?;
        let shown = path.display().to_string();
        let actual_text = canonical_json(actual);
        let stored = match fs::read_to_string(&path) {
            Ok(text) => Some(text),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(source) => {
                return Err(GoldenError::Io {
                    path: shown,
                    source,
                });
            }
        };
        let expected = stored.map(|text| serde_json::from_str::<Value>(&text));
        if let Some(Ok(expected)) = &expected
            && expected == actual
        {
            return Ok(());
        }
        if self.mode == GoldenMode::Update {
            return write(&path, &actual_text).map_err(|source| GoldenError::Io {
                path: shown,
                source,
            });
        }
        match expected {
            None => Err(GoldenError::Missing {
                path: shown,
                actual: actual_text,
            }),
            Some(Err(source)) => Err(GoldenError::Malformed {
                path: shown,
                source,
            }),
            Some(Ok(expected)) => {
                let expected_text = canonical_json(&expected);
                let diff = TextDiff::from_lines(&expected_text, &actual_text)
                    .unified_diff()
                    .context_radius(3)
                    .header(&shown, "actual")
                    .to_string();
                Err(GoldenError::Mismatch { path: shown, diff })
            }
        }
    }
}

fn write(path: &Path, text: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)
}

/// The canonical text of a golden: keys sorted recursively, two-space indentation, and a
/// trailing newline. Sorting here makes the bytes independent of `serde_json`'s map ordering.
pub fn canonical_json(value: &Value) -> String {
    format!("{:#}\n", sorted(value))
}

fn sorted(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by_key(|(key, _)| *key);
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, child)| (key.clone(), sorted(child)))
                    .collect::<Map<_, _>>(),
            )
        }
        Value::Array(items) => Value::Array(items.iter().map(sorted).collect()),
        other => other.clone(),
    }
}

/// Redacts `envelope` against `root` and checks it against the golden `name` in this crate's
/// `golden/` directory.
///
/// # Panics
///
/// When the golden is missing, unreadable, or different from the redacted envelope; the
/// message is the [`GoldenError`], which names the file and the update command.
#[track_caller]
pub fn assert_golden(name: &str, envelope: &Envelope, root: &Path) {
    let actual = crate::redact(envelope, root);
    if let Err(error) = Goldens::from_env().check(name, &actual) {
        panic!("{error}");
    }
}
