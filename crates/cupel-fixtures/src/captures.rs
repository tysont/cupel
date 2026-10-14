// ABOUTME: Real runner output committed under `captures/<runner>/<case>/` (P3.1): the files a tool wrote and
// ABOUTME: the `capture.toml` saying where it ran, which parser tests read instead of running the tool.
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// One capture: a runner's output for one case (`green`, `red`, or `broken`).
pub struct Capture {
    dir: PathBuf,
    /// The directory the tool ran in, which its absolute paths start with.
    pub ran_in: PathBuf,
}

#[derive(Deserialize)]
struct Provenance {
    dir: PathBuf,
}

impl Capture {
    /// The capture of `runner` (such as `pytest`) for `case`.
    ///
    /// # Panics
    ///
    /// When the capture or its `capture.toml` is missing or malformed.
    pub fn of(runner: &str, case: &str) -> Self {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("captures")
            .join(runner)
            .join(case);
        let text = fs::read_to_string(dir.join("capture.toml"))
            .unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
        let provenance: Provenance =
            toml::from_str(&text).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
        Self {
            dir,
            ran_in: provenance.dir,
        }
    }

    /// The text of the captured file `name`, such as `junit.xml`.
    ///
    /// # Panics
    ///
    /// When the file is missing.
    pub fn file(&self, name: &str) -> String {
        let path = self.dir.join(name);
        fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    }
}
