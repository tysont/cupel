// ABOUTME: The base side of a range scope (ADR-056): the range's changes, and the base commit's versions
// ABOUTME: of the changed source files written to the run's scratch directory for producers to parse.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::error::{ArgumentError, Result};
use crate::git::{Change, ChangeKind, Git};
use crate::writer::Writer;

/// What a range compares: the commit it starts from and every path it changes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Range {
    /// The base commit's SHA: the merge base (`--changed`) or `--base`.
    pub commit: String,
    /// Every changed path after `--paths`, sorted, whether or not a language owns it.
    pub changes: Vec<Change>,
}

/// A range's base side, ready for producers: the base versions of the changed source files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseTree {
    pub range: Range,
    /// The directory holding the base versions, `.cupel/work/<run>/base/`.
    pub root: PathBuf,
    /// The changed source files that exist at the base commit, relative to `root`.
    pub files: Vec<String>,
}

impl BaseTree {
    /// The directory under a scratch writer's area that holds the base versions.
    const DIR: &'static str = "base";

    /// Writes the base version of every changed path that existed at the base commit and that
    /// `owned` accepts (a registered language owns it) through `scratch`, the run's scratch
    /// writer, and of every such path `manifest` accepts (a package manifest a language reads,
    /// such as `package.json`), which is not one of `files` (ADR-192). A file git cannot read is
    /// a usage error on `--base`.
    pub fn checkout(
        range: Range,
        git: &Git,
        scratch: &Writer,
        owned: impl Fn(&str) -> bool,
        manifest: impl Fn(&str) -> bool,
    ) -> Result<Self> {
        let root = scratch.area().join(Self::DIR);
        let existed = || {
            range
                .changes
                .iter()
                .filter(|change| change.kind != ChangeKind::Added)
                .map(|change| change.path.clone())
        };
        let files: Vec<String> = existed().filter(|path| owned(path)).collect();
        let manifests: Vec<String> = existed()
            .filter(|path| !owned(path) && manifest(path))
            .collect();
        scratch.create_dir(&root)?;
        for file in files.iter().chain(&manifests) {
            let contents = git.show(&range.commit, file).map_err(|error| {
                ArgumentError::new(
                    "--base",
                    Some(&range.commit),
                    error.message.clone(),
                    format!("Fix the range; git said: {error}"),
                )
            })?;
            scratch.write(&root.join(file), &contents)?;
        }
        Ok(Self { range, root, files })
    }

    /// What producers of this side read beyond the files `root` holds (ADR-192): the
    /// repository at `repository`, for every path the range did not change.
    pub fn side(&self, repository: &Path) -> BaseSide {
        BaseSide {
            repository: repository.to_path_buf(),
            changed: self
                .range
                .changes
                .iter()
                .map(|change| change.path.clone())
                .collect(),
        }
    }

    /// The base side as `checkout` would lay it out, reading and writing nothing: what a dry
    /// run plans with.
    pub fn planned(range: Range, scratch: &Writer) -> Self {
        Self {
            range,
            root: scratch.area().join(Self::DIR),
            files: Vec::new(),
        }
    }
}

/// A range's base side as a producer that reads beyond the changed files sees it (ADR-192):
/// a path the range changed is as the base tree holds it (absent when the base tree does not),
/// and every other path is the same as in the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseSide {
    /// The repository root, which holds the files the range did not change.
    pub repository: PathBuf,
    /// Every path the range changed, whether or not the base tree holds it.
    pub changed: BTreeSet<String>,
}

impl BaseSide {
    /// The directory holding `path` at the base: `base_root` (the base tree) when the range
    /// changed it, else the repository.
    pub fn root_of<'a>(&'a self, base_root: &'a Path, path: &str) -> &'a Path {
        if self.changed.contains(path) {
            base_root
        } else {
            &self.repository
        }
    }
}
