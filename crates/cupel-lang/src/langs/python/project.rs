// ABOUTME: Python roots for `ProjectInfo`: `pyproject.toml`, `setup.cfg`, or `requirements.txt`, with virtual
// ABOUTME: environments and caches as generated directories, and the directory coverage measures (ADR-021, ADR-037, ADR-152).
use std::path::Path;

use crate::project::Ecosystem;

pub const ECOSYSTEM: Ecosystem = Ecosystem {
    manifests: &["pyproject.toml", "setup.cfg", "requirements.txt"],
    skip_dirs: &[".venv", "venv", "__pycache__", ".tox"],
    describe: |candidate| candidate.root("python", vec![source(&candidate.dir)]),
};

/// The directory the root's coverage command measures (`<packages>`, ADR-152): `src` for a src
/// layout, whose packages sit under it, else the root itself. coverage.py lists every file under
/// the directory it is given, loaded or not.
fn source(dir: &Path) -> String {
    if dir.join("src").is_dir() {
        "src".to_owned()
    } else {
        ".".to_owned()
    }
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
