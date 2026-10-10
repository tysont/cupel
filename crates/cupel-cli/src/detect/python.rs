// ABOUTME: Python roots (`pyproject.toml`, `setup.cfg`, `requirements.txt`): uv, poetry, or pip by
// ABOUTME: lockfile and tool table; pytest or unittest by what the project names (ADR-021).
use std::fs;

use cupel_core::syntax::{DefaultCommands, ProjectRoot};

use super::{Candidate, Ecosystem, mentions};

pub const ECOSYSTEM: Ecosystem = Ecosystem {
    manifests: &["pyproject.toml", "setup.cfg", "requirements.txt"],
    skip_dirs: &[".venv", "venv", "__pycache__", ".tox"],
    describe,
};

fn describe(candidate: &Candidate) -> ProjectRoot {
    let pyproject = candidate.read("pyproject.toml").unwrap_or_default();
    let prefix = if candidate.has("uv.lock") || pyproject.contains("[tool.uv") {
        "uv run python -m"
    } else if candidate.has("poetry.lock") || pyproject.contains("[tool.poetry") {
        "poetry run python -m"
    } else {
        "python -m"
    };
    let declarations = declarations(candidate);
    let names = |name: &str| declarations.iter().any(|text| mentions(text, name));
    let pytest = names("pytest") || candidate.has("pytest.ini") || candidate.has("conftest.py");
    let (test, coverage) = if pytest {
        (
            format!("{prefix} pytest"),
            format!("{prefix} pytest --cov --cov-report=lcov:{{out}}"),
        )
    } else {
        (
            format!("{prefix} unittest"),
            format!("{prefix} coverage run -m unittest && {prefix} coverage lcov -o {{out}}"),
        )
    };
    let bench = (pytest && names("pytest-benchmark"))
        .then(|| format!("{prefix} pytest --benchmark-only --benchmark-json={{out}}"));
    candidate.root(
        "python",
        DefaultCommands {
            test: Some(test),
            coverage: Some(coverage),
            bench,
        },
    )
}

/// The text of every file in the root or a member that declares dependencies or tool settings:
/// `pyproject.toml`, `setup.cfg`, and `requirements*.txt`.
fn declarations(candidate: &Candidate) -> Vec<String> {
    candidate
        .dirs()
        .filter_map(|dir| fs::read_dir(dir).ok())
        .flat_map(|entries| entries.filter_map(Result::ok))
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name == "pyproject.toml"
                || name == "setup.cfg"
                || (name.starts_with("requirements") && name.ends_with(".txt"))
        })
        .filter_map(|entry| fs::read_to_string(entry.path()).ok())
        .collect()
}
