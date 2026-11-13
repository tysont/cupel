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
/// the files and range changes the request selects from it (ADR-065), only under the configured
/// roots when there are any (ADR-177).
pub(crate) struct Scoped {
    pub info: ProjectInfo,
    pub tree: Vec<String>,
    pub files: ResolvedFiles,
}

/// Walks the repository and resolves `request` against the part of the walk under the configured
/// roots; the whole walk stays in `tree` for the import graphs that read past them.
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
    let dirs = configured_dirs(config);
    let in_roots = in_roots(&tree, &dirs);
    let default_branch = config.file.default_branch.as_deref();
    let mut files = request
        .resolve_files(&in_roots, root, default_branch)
        .map_err(|error| {
            outside_roots(request, (&tree, &in_roots), &dirs).unwrap_or_else(|| error.into())
        })?;
    files
        .changes
        .retain(|change| !exclude.excludes(&change.path) && in_roots_dir(&change.path, &dirs));
    Ok(Scoped { info, tree, files })
}

/// The paths of the configured roots; empty without `[roots]`.
fn configured_dirs(config: &Config) -> Vec<&str> {
    config
        .file
        .roots
        .values()
        .map(|root| root.path.as_str())
        .collect()
}

/// Whether `file` is in a run's scope by the configured roots `dirs`: under one, or anywhere
/// without any (ADR-177).
fn in_roots_dir(file: &str, dirs: &[&str]) -> bool {
    dirs.is_empty() || cupel_core::is_under(file, dirs)
}

/// The files of `tree` under one of the configured roots `dirs`, all of them without any.
fn in_roots(tree: &[String], dirs: &[&str]) -> Vec<String> {
    tree.iter()
        .filter(|file| in_roots_dir(file, dirs))
        .cloned()
        .collect()
}

/// The usage error for a `--paths` value that names files of the whole walk but none under the
/// configured roots `dirs`; `None` when every value names some file in scope.
fn outside_roots(
    request: &ScopeRequest,
    (tree, in_roots): (&[String], &[String]),
    dirs: &[&str],
) -> Option<CupelError> {
    let names = |files: &[String], path: &str| {
        let alone = ScopeRequest {
            paths: vec![path.to_owned()],
            ..ScopeRequest::default()
        };
        alone.resolve_files(files, Path::new("."), None).is_ok()
    };
    let path = request
        .paths
        .iter()
        .find(|path| !names(in_roots, path) && names(tree, path))?;
    Some(CupelError::bad_argument(
        "--paths",
        Some(path),
        format!(
            "`--paths {path}` names no source file under a configured root ({})",
            dirs.join(", ")
        ),
        "Pass paths under a root that `[roots]` in cupel.toml names, or add a root for them."
            .to_owned(),
    ))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn files(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|path| (*path).to_owned()).collect()
    }

    #[test]
    fn the_configured_roots_hold_the_scope_and_none_holds_all_of_it() {
        let tree = files(&[
            "apps/agent/a.ts",
            "packages/shared/b.ts",
            "packages/sharedx/c.ts",
        ]);
        assert_eq!(
            in_roots(&tree, &["packages/shared"]),
            ["packages/shared/b.ts"]
        );
        assert_eq!(in_roots(&tree, &[]), tree);
    }

    #[test]
    fn a_path_only_outside_the_roots_is_refused_by_name() {
        let tree = files(&["apps/agent/a.ts", "packages/shared/b.ts"]);
        let held = in_roots(&tree, &["packages/shared"]);
        let request = |path: &str| ScopeRequest {
            paths: vec!["packages/shared".to_owned(), path.to_owned()],
            ..ScopeRequest::default()
        };
        let refused = outside_roots(&request("apps"), (&tree, &held), &["packages/shared"]);
        let usage = refused.map(|error| error.to_string());
        assert_eq!(
            usage.as_deref(),
            Some("`--paths apps` names no source file under a configured root (packages/shared)")
        );
        let missing = outside_roots(&request("nowhere"), (&tree, &held), &["packages/shared"]);
        assert_eq!(missing.map(|error| error.to_string()), None);
    }
}
