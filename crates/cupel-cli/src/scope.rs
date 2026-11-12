// ABOUTME: Resolves what a run's scope names: the request from the scope flags, the files and the whole walk, the
// ABOUTME: languages they hold, the lines a range changed, the project's roots, and the git state the envelope reports.

use std::collections::BTreeSet;
use std::path::Path;

use cupel_core::config::Config;
use cupel_core::git::{Git, Side};
use cupel_core::syntax::ProjectInfo;
use cupel_core::{ChangedLines, GitInfo, Lang, ResolvedFiles, ScopeRequest, Writer};
use cupel_lang::LanguageRegistry;

use crate::cli::ScopeArgs;
use crate::error::CupelError;
use crate::project;

/// The scope the flags in `args` name.
pub(crate) fn scope_request(args: &ScopeArgs) -> ScopeRequest {
    ScopeRequest {
        changed: args.changed,
        base: args.base.clone(),
        head: args.head.clone(),
        paths: args.paths.clone(),
        symbols: args.symbols.clone(),
    }
}

/// The lines a range scope changed per file; `None` without a range.
pub(crate) fn changed_lines(
    root: &Path,
    request: &ScopeRequest,
    resolved: &ResolvedFiles,
) -> Result<Option<ChangedLines>, CupelError> {
    let (Some(base), Some(git)) = (&resolved.base, Git::open(root)) else {
        return Ok(None);
    };
    let hunks = git
        .changed_lines(base, request.head.as_deref(), Side::New)
        .map_err(|error| {
            CupelError::bad_argument(
                if request.changed {
                    "--changed"
                } else {
                    "--base"
                },
                None,
                format!("cannot read the changed lines: {error}"),
                "Run in a git work tree with git installed, or scope with --paths instead."
                    .to_owned(),
            )
        })?;
    Ok(Some(hunks))
}

/// A resolved scope: the project roots outside `[scope] exclude`, every file of the walk, and
/// the files and range changes the request selects from it (ADR-065).
pub(crate) struct Scoped {
    pub info: ProjectInfo,
    pub tree: Vec<String>,
    pub files: ResolvedFiles,
}

/// Walks the repository and resolves `request` against the walk.
pub(crate) fn resolve(
    writer: &Writer,
    config: &Config,
    request: &ScopeRequest,
    languages: &LanguageRegistry,
) -> Result<Scoped, CupelError> {
    let root = writer.root();
    let exclude = &config.file.scope.exclude;
    let roots: Vec<_> = config.file.roots.values().cloned().collect();
    let info = project::detect(writer, exclude, &roots);
    let tree = cupel_lang::walk_excluding(root, languages, &info, exclude);
    let mut files = request.resolve_files(&tree, root, config.file.default_branch.as_deref())?;
    files
        .changes
        .retain(|change| !exclude.excludes(&change.path));
    Ok(Scoped { info, tree, files })
}

/// The languages that own at least one of `files`, in ID order.
pub(crate) fn languages_of<'a>(
    languages: &LanguageRegistry,
    files: impl Iterator<Item = &'a String>,
) -> Vec<Lang> {
    let owning: BTreeSet<_> = files
        .filter_map(|file| languages.for_path(file))
        .map(|language| language.id())
        .collect();
    languages
        .langs()
        .into_iter()
        .filter(|lang| owning.contains(&lang.id))
        .collect()
}

/// The detected project roots of the languages in scope; `.` when none is detected.
pub(crate) fn roots(info: &ProjectInfo, langs: &[Lang]) -> Vec<String> {
    let roots: BTreeSet<String> = info
        .roots
        .iter()
        .filter(|root| langs.iter().any(|lang| lang.id == root.lang))
        .map(|root| root.path.clone())
        .collect();
    if roots.is_empty() {
        vec![".".to_owned()]
    } else {
        roots.into_iter().collect()
    }
}

/// `run.git`: null outside a work tree or before the first commit.
pub(crate) fn git_info(root: &Path, resolved: &ResolvedFiles) -> Option<GitInfo> {
    let head = Git::open(root)?.head();
    Some(GitInfo {
        sha: head.sha?,
        dirty: head.dirty,
        base: resolved.base.clone(),
        branch: head.branch,
    })
}
