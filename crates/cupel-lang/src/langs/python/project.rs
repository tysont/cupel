// ABOUTME: Python roots for `ProjectInfo`: `pyproject.toml`, `setup.cfg`, or `requirements.txt`, with virtual
// ABOUTME: environments and caches as generated directories, and the directory coverage measures (ADR-021, ADR-037, ADR-152, ADR-208).
use std::path::Path;

use crate::project::{Ecosystem, every_manifest};

pub const ECOSYSTEM: Ecosystem = Ecosystem {
    manifests: &["pyproject.toml", "setup.cfg", "requirements.txt"],
    builds: every_manifest,
    skip_dirs: &[".venv", "venv", "__pycache__", ".tox"],
    describe: |candidate| candidate.root("python", packages(&candidate.dir)),
};

/// The directories the root's coverage command measures (`<packages>`, ADR-152, ADR-208): none
/// when the project's coverage configuration chooses its files, as a source would make
/// coverage.py ignore its `include`; else `src` for a src layout, whose packages sit under it,
/// else the root itself. coverage.py lists every file under the directory it is given, loaded
/// or not.
fn packages(dir: &Path) -> Vec<String> {
    if chooses_files(dir) {
        Vec::new()
    } else if dir.join("src").is_dir() {
        vec!["src".to_owned()]
    } else {
        vec![".".to_owned()]
    }
}

/// The keys of coverage.py's run settings that choose the files it measures.
const FILE_KEYS: [&str; 3] = ["source", "source_pkgs", "include"];

/// Whether a file coverage.py reads in `dir` sets one of [`FILE_KEYS`]: `.coveragerc`'s `[run]`,
/// `setup.cfg`'s or `tox.ini`'s `[coverage:run]`, or `pyproject.toml`'s `[tool.coverage.run]`.
fn chooses_files(dir: &Path) -> bool {
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap_or_default();
    ini_sets(&read(".coveragerc"), "run")
        || ini_sets(&read("setup.cfg"), "coverage:run")
        || ini_sets(&read("tox.ini"), "coverage:run")
        || read("pyproject.toml")
            .parse::<toml::Table>()
            .ok()
            .and_then(|table| table.get("tool")?.get("coverage")?.get("run").cloned())
            .and_then(|run| run.as_table().cloned())
            .is_some_and(|run| FILE_KEYS.iter().any(|key| run.contains_key(*key)))
}

/// Whether the INI `text` sets one of [`FILE_KEYS`] in its `[section]`.
fn ini_sets(text: &str, section: &str) -> bool {
    let header = format!("[{section}]");
    let mut inside = false;
    text.lines().map(str::trim).any(|line| {
        if line.starts_with('[') {
            inside = line == header;
            return false;
        }
        inside
            && line
                .split(['=', ':'])
                .next()
                .is_some_and(|key| FILE_KEYS.contains(&key.trim()))
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use cupel_core::Exclude;

    use super::ECOSYSTEM;

    /// The one root of a repository at `dir` holding a `pyproject.toml`, with `dirs` made first.
    fn packages_of(dir: &Path, dirs: &[&str]) -> Vec<String> {
        for sub in dirs {
            fs::create_dir_all(dir.join(sub)).unwrap();
        }
        fs::write(dir.join("pyproject.toml"), "[project]\nname = \"shop\"\n").unwrap();
        let roots = crate::project::roots(dir, &ECOSYSTEM, &[], &Exclude::default());
        assert_eq!(roots.len(), 1, "{roots:?}");
        roots[0].packages.clone()
    }

    /// The packages of a flat root whose `pyproject.toml` holds `[project]` then `pyproject`,
    /// with `file` holding `text` beside it (none when `file` is empty).
    fn configured(pyproject: &str, file: &str, text: &str) -> Vec<String> {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("shop")).unwrap();
        let manifest = format!("[project]\nname = \"shop\"\n\n{pyproject}");
        fs::write(dir.path().join("pyproject.toml"), manifest).unwrap();
        if !file.is_empty() {
            fs::write(dir.path().join(file), text).unwrap();
        }
        let roots = crate::project::roots(dir.path(), &ECOSYSTEM, &[], &Exclude::default());
        roots[0].packages.clone()
    }

    /// A project whose coverage configuration chooses the files to measure (`source` or
    /// `include` in any file coverage.py reads) has no coverage source of Cupel's: a source
    /// would make coverage.py ignore its `include`.
    #[test]
    fn a_root_whose_coverage_configuration_chooses_its_files_has_no_coverage_source() {
        let none: Vec<String> = Vec::new();
        assert_eq!(
            [
                configured("[tool.coverage.run]\ninclude = [\"shop/*\"]\n", "", ""),
                configured("[tool.coverage.run]\nsource = [\"shop\"]\n", "", ""),
                configured("", ".coveragerc", "[run]\nsource = shop\n"),
                configured(
                    "",
                    "setup.cfg",
                    "[metadata]\nname = shop\n\n[coverage:run]\ninclude =\n    shop/*\n"
                ),
                configured("", "tox.ini", "[coverage:run]\nsource_pkgs = shop\n"),
            ],
            [none.clone(), none.clone(), none.clone(), none.clone(), none]
        );
    }

    /// Coverage configuration that leaves the files to the command keeps the root's source.
    #[test]
    fn a_root_whose_coverage_configuration_leaves_the_files_to_the_command_keeps_its_source() {
        assert_eq!(
            [
                configured(
                    "[tool.coverage.run]\nbranch = true\nomit = [\"shop/x.py\"]\n",
                    "",
                    ""
                ),
                configured("", ".coveragerc", "[report]\ninclude = shop/*\n"),
                configured("", "setup.cfg", "[tool:pytest]\naddopts = -q\n"),
            ],
            [
                vec![".".to_owned()],
                vec![".".to_owned()],
                vec![".".to_owned()]
            ]
        );
    }

    /// The root's coverage source is `src` for a src layout, and the root itself otherwise
    /// (ADR-152).
    #[test]
    fn a_roots_coverage_source_is_src_when_it_has_one_and_itself_otherwise() {
        let src_layout = tempfile::tempdir().unwrap();
        let flat = tempfile::tempdir().unwrap();
        assert_eq!(packages_of(src_layout.path(), &["src/shop"]), ["src"]);
        assert_eq!(packages_of(flat.path(), &["shop"]), ["."]);
    }
}
