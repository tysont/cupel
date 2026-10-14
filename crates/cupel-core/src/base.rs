// ABOUTME: The base side of a range scope (ADR-056): the range's changes, and the base commit's versions
// ABOUTME: of the changed source files written to the run's scratch directory for producers to parse.
use std::path::PathBuf;

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
    /// writer. A file git cannot read is a usage error on `--base`.
    pub fn checkout(
        range: Range,
        git: &Git,
        scratch: &Writer,
        owned: impl Fn(&str) -> bool,
    ) -> Result<Self> {
        let root = scratch.area().join(Self::DIR);
        let files: Vec<String> = range
            .changes
            .iter()
            .filter(|change| change.kind != ChangeKind::Added && owned(&change.path))
            .map(|change| change.path.clone())
            .collect();
        scratch.create_dir(&root)?;
        for file in &files {
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
