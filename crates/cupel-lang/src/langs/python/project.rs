// ABOUTME: Python roots for `ProjectInfo`: `pyproject.toml`, `setup.cfg`, or `requirements.txt`, with
// ABOUTME: virtual environments and caches as generated directories (ADR-021, ADR-037).
use crate::project::Ecosystem;

pub const ECOSYSTEM: Ecosystem = Ecosystem {
    manifests: &["pyproject.toml", "setup.cfg", "requirements.txt"],
    skip_dirs: &[".venv", "venv", "__pycache__", ".tox"],
    describe: |candidate| candidate.root("python", Vec::new()),
};
