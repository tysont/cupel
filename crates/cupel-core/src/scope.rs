// ABOUTME: The scope a run considers and its resolution from --paths, --changed, --base/--head, and
// ABOUTME: --symbols: files first (tree ∩ paths ∩ range), then symbols from produced facts (ADR-040).
use std::collections::BTreeSet;
use std::path::Path;

use globset::{GlobBuilder, GlobMatcher};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{ArgumentError, Result};
use crate::git::{Change, Git};
use crate::hash::SymbolHash;
use crate::syntax::{NormTree, SymbolDef, SymbolKey, Symbols};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ScopeMode {
    Tree,
    Changed,
    Range,
    Paths,
    Symbols,
    SinceBaseline,
}

/// The files, and optionally the symbols, a run considers. Paths are relative to the
/// repository root and `/`-separated.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scope {
    pub files: Vec<String>,
    /// The symbols in scope; `None` means every symbol in `files`.
    pub symbols: Option<BTreeSet<SymbolKey>>,
}

impl Scope {
    /// Every symbol in `files`.
    pub fn of_files(files: Vec<String>) -> Self {
        Self {
            files,
            symbols: None,
        }
    }

    /// Whether the symbol starting at `key` is in scope.
    pub fn includes(&self, key: &SymbolKey) -> bool {
        self.symbols
            .as_ref()
            .map_or_else(|| self.files.contains(&key.file), |keys| keys.contains(key))
    }
}

/// The scope flags of design §6.4, as given.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScopeRequest {
    pub changed: bool,
    pub base: Option<String>,
    pub head: Option<String>,
    pub paths: Vec<String>,
    pub symbols: Vec<String>,
}

/// The files a request selects and the commit a range starts from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedFiles {
    pub files: Vec<String>,
    /// The merge base (`--changed`) or base commit (`--base`), for `run.git.base`.
    pub base: Option<String>,
    /// Every path the range changes that the paths select, including deleted files and files
    /// no language owns; empty without a range (ADR-056).
    pub changes: Vec<Change>,
}

impl ScopeRequest {
    /// Whether a range (`--changed` or `--base`) limits the files.
    pub fn has_range(&self) -> bool {
        self.changed || self.base.is_some()
    }

    /// The envelope's `scope.mode`: a range wins, then symbols, then paths (ADR-040).
    pub fn mode(&self) -> ScopeMode {
        if self.changed {
            ScopeMode::Changed
        } else if self.base.is_some() {
            ScopeMode::Range
        } else if !self.symbols.is_empty() {
            ScopeMode::Symbols
        } else if !self.paths.is_empty() {
            ScopeMode::Paths
        } else {
            ScopeMode::Tree
        }
    }

    /// The `--symbols` selectors, parsed.
    pub fn selectors(&self) -> Result<Vec<SymbolSelector>> {
        self.symbols
            .iter()
            .map(|text| SymbolSelector::parse(text))
            .collect()
    }

    /// The files of `tree` (the language walk, sorted) that the paths and the range select.
    /// `default_branch` is the configured branch `--changed` compares with; git decides when
    /// it is `None`.
    pub fn resolve_files(
        &self,
        tree: &[String],
        root: &Path,
        default_branch: Option<&str>,
    ) -> Result<ResolvedFiles> {
        let patterns = self.patterns(tree)?;
        let selected = |file: &str| {
            patterns.is_empty() || patterns.iter().any(|pattern| pattern.matches(file))
        };
        let mut files: Vec<String> = tree.iter().filter(|file| selected(file)).cloned().collect();
        let mut base = None;
        let mut changes = Vec::new();
        if let Some((changed, start)) = self.range(root, default_branch)? {
            let paths: BTreeSet<&str> = changed.iter().map(|change| change.path.as_str()).collect();
            files.retain(|file| paths.contains(file.as_str()));
            changes = changed
                .into_iter()
                .filter(|change| selected(&change.path))
                .collect();
            base = Some(start);
        }
        Ok(ResolvedFiles {
            files,
            base,
            changes,
        })
    }

    /// The `--paths` patterns; every pattern must match some file of `tree`.
    fn patterns(&self, tree: &[String]) -> Result<Vec<PathPattern>> {
        let patterns = self
            .paths
            .iter()
            .map(|text| PathPattern::parse(text))
            .collect::<Result<Vec<_>>>()?;
        if let Some(unmatched) = patterns
            .iter()
            .find(|pattern| !tree.iter().any(|file| pattern.matches(file)))
        {
            return Err(ArgumentError::new(
                "--paths",
                Some(&unmatched.text),
                format!("`--paths {}` matches no source file", unmatched.text),
                "Pass files, directories, or globs relative to the repository root that \
                 contain source files of a registered language."
                    .to_owned(),
            )
            .into());
        }
        Ok(patterns)
    }

    /// The paths the range changed and the commit it starts from; `None` without a range.
    fn range(
        &self,
        root: &Path,
        default_branch: Option<&str>,
    ) -> Result<Option<(Vec<Change>, String)>> {
        let Some(argument) = self.range_argument()? else {
            return Ok(None);
        };
        let git = Git::open(root).ok_or_else(|| {
            ArgumentError::new(
                argument,
                None,
                format!("`{argument}` needs a git repository"),
                "Run in a git work tree with git installed, or scope with --paths instead."
                    .to_owned(),
            )
        })?;
        let range = if self.changed {
            changed_range(&git, default_branch)?
        } else {
            self.explicit_range(&git)?
        };
        Ok(Some(range))
    }

    /// The flag that chooses the range, `--changed` or `--base`; `None` without one. A
    /// `--head` without `--base`, or both `--changed` and `--base`, is a usage error.
    fn range_argument(&self) -> Result<Option<&'static str>> {
        if self.head.is_some() && self.base.is_none() {
            return Err(ArgumentError::new(
                "--head",
                self.head.as_deref(),
                "`--head` needs `--base`".to_owned(),
                "Pass --base REF with --head REF, or drop --head to compare with the working tree."
                    .to_owned(),
            )
            .into());
        }
        if self.changed && self.base.is_some() {
            return Err(ArgumentError::new(
                "--changed",
                None,
                "`--changed` and `--base` both choose a range".to_owned(),
                "Pass --changed to compare with the default branch, or --base REF for an \
                 explicit range, not both."
                    .to_owned(),
            )
            .into());
        }
        Ok(if self.changed {
            Some("--changed")
        } else {
            self.base.as_ref().map(|_| "--base")
        })
    }

    /// `--base` to `--head`, or to the working tree without `--head`.
    fn explicit_range(&self, git: &Git) -> Result<(Vec<Change>, String)> {
        let base = self.base.as_deref().unwrap_or_default();
        let start = git
            .commit(base)
            .map_err(|error| git_failure("--base", Some(base), &error))?;
        let files = match &self.head {
            Some(head) => {
                let end = git
                    .commit(head)
                    .map_err(|error| git_failure("--head", Some(head), &error))?;
                git.changes_between(&start, &end)
            }
            None => git.changes_since(&start),
        }
        .map_err(|error| git_failure("--base", Some(base), &error))?;
        Ok((files, start))
    }
}

/// `--changed`: the merge base of `HEAD` with the default branch, to the working tree.
fn changed_range(git: &Git, default_branch: Option<&str>) -> Result<(Vec<Change>, String)> {
    let branch = default_branch
        .map(str::to_owned)
        .or_else(|| git.default_branch())
        .ok_or_else(|| {
            ArgumentError::new(
                "--changed",
                None,
                "no default branch to compare with".to_owned(),
                "Set default_branch in cupel.toml, or use --base REF.".to_owned(),
            )
        })?;
    let failure = |error: crate::git::GitError| git_failure("--changed", None, &error);
    let start = git
        .commit(&branch)
        .and_then(|_| git.merge_base(&branch, "HEAD"))
        .map_err(failure)?;
    let files = git.changes_since(&start).map_err(failure)?;
    Ok((files, start))
}

/// The usage error for a range git could not follow, on `argument` with `value`.
fn git_failure(argument: &str, value: Option<&str>, error: &crate::git::GitError) -> ArgumentError {
    ArgumentError::new(
        argument,
        value,
        error.message.clone(),
        format!("Fix the range; git said: {error}"),
    )
}

/// One `--paths` value: a file, a directory, or a glob, relative to the root.
struct PathPattern {
    text: String,
    path: String,
    glob: Option<GlobMatcher>,
}

impl PathPattern {
    fn parse(text: &str) -> Result<Self> {
        let trimmed = text.trim_start_matches("./").trim_end_matches('/');
        let path = if trimmed.is_empty() { "." } else { trimmed }.to_owned();
        let glob = if path.contains(['*', '?', '[', '{']) {
            let glob = GlobBuilder::new(&path)
                .literal_separator(true)
                .build()
                .map_err(|error| {
                    ArgumentError::new(
                        "--paths",
                        Some(text),
                        format!("`--paths {text}` is not a valid glob: {error}"),
                        "Fix the glob; `*` matches within a directory, `**` across them."
                            .to_owned(),
                    )
                })?;
            Some(glob.compile_matcher())
        } else {
            None
        };
        Ok(Self {
            text: text.to_owned(),
            path,
            glob,
        })
    }

    fn matches(&self, file: &str) -> bool {
        match &self.glob {
            Some(glob) => glob.is_match(file),
            None => {
                self.path == "."
                    || file == self.path
                    || file
                        .strip_prefix(&self.path)
                        .is_some_and(|rest| rest.starts_with('/'))
            }
        }
    }
}

/// One `--symbols` value: a structural hash or a display name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolSelector {
    Hash(SymbolHash),
    /// `::` and `.` are the same separator.
    Name(String),
}

impl SymbolSelector {
    pub fn parse(text: &str) -> Result<Self> {
        if text.starts_with("sh_") {
            let hash = text.parse().map_err(|_| {
                ArgumentError::new(
                    "--symbols",
                    Some(text),
                    format!("`{text}` is not a structural hash"),
                    "Pass a hash as printed in a finding (`sh_` and 16 hex digits), or a name \
                     such as `module::Type::method`."
                        .to_owned(),
                )
            })?;
            Ok(Self::Hash(hash))
        } else {
            Ok(Self::Name(normalized(text)))
        }
    }

    /// Whether `symbol`, whose structural hash is `hash` when known, is selected.
    pub fn matches(&self, symbol: &SymbolDef, hash: Option<&SymbolHash>) -> bool {
        match self {
            Self::Hash(wanted) => hash == Some(wanted),
            Self::Name(name) => normalized(&symbol.display) == *name,
        }
    }

    /// Whether this selector needs structural hashes (`NormTree`).
    pub fn needs_hashes(&self) -> bool {
        matches!(self, Self::Hash(_))
    }
}

fn normalized(name: &str) -> String {
    name.replace("::", ".")
}

/// The symbols of `symbols` that some selector matches, and for each selector whether it
/// matched anything. `norm` supplies hashes for hash selectors.
pub fn select_symbols(
    selectors: &[SymbolSelector],
    symbols: &Symbols,
    norm: Option<&NormTree>,
) -> (BTreeSet<SymbolKey>, Vec<bool>) {
    let hash_of = |key: &SymbolKey| {
        norm.and_then(|norm| {
            norm.symbols
                .iter()
                .find(|entry| entry.symbol == *key)
                .map(|entry| &entry.hash)
        })
    };
    let mut matched = vec![false; selectors.len()];
    let mut keys = BTreeSet::new();
    for symbol in &symbols.symbols {
        let key = symbol.key();
        let hash = hash_of(&key);
        for (index, selector) in selectors.iter().enumerate() {
            if selector.matches(symbol, hash) {
                matched[index] = true;
                keys.insert(key.clone());
            }
        }
    }
    (keys, matched)
}
