// ABOUTME: Scope resolution against real temporary git repositories: --paths, --changed, --base/--head,
// ABOUTME: --symbols, their intersection, and the usage errors for inputs that cannot be honored (ADR-040).
mod support;

use std::path::Path;

use cupel_core::syntax::{
    NormNode, NormTree, NormalizedSymbol, Span, SymbolDef, SymbolKind, Symbols, Visibility,
};
use cupel_core::{
    Error, LangId, ScopeMode, ScopeRequest, SymbolHash, SymbolSelector, select_symbols,
};
use support::GitRepo;

/// A repository on `feature` branched from `main`: `src/a.rs` and `src/b.rs` and `lib/c.rs`
/// on main; the branch edits `src/a.rs` and adds `lib/d.rs`, and the working tree adds the
/// untracked `src/e.rs`.
fn branched() -> Option<GitRepo> {
    let repo = GitRepo::new()?;
    repo.write("src/a.rs", "fn a() {}\n");
    repo.write("src/b.rs", "fn b() {}\n");
    repo.write("lib/c.rs", "fn c() {}\n");
    repo.commit("main");
    repo.git(&["checkout", "--quiet", "-b", "feature"]);
    repo.write("src/a.rs", "fn a() { 1; }\n");
    repo.write("lib/d.rs", "fn d() {}\n");
    repo.commit("feature");
    repo.write("src/e.rs", "fn e() {}\n");
    Some(repo)
}

fn tree() -> Vec<String> {
    ["lib/c.rs", "lib/d.rs", "src/a.rs", "src/b.rs", "src/e.rs"]
        .map(str::to_owned)
        .to_vec()
}

fn request() -> ScopeRequest {
    ScopeRequest::default()
}

fn files(request: &ScopeRequest, root: &Path) -> Result<Vec<String>, Error> {
    request
        .resolve_files(&tree(), root, None)
        .map(|resolved| resolved.files)
}

fn argument(error: Error) -> String {
    match error {
        Error::Argument(argument) => argument.argument,
        other => panic!("expected an argument error, got {other:?}"),
    }
}

#[test]
fn the_scope_matrix_intersects_every_flag_given() {
    let Some(repo) = branched() else { return };
    let root = repo.root();
    let paths = |patterns: &[&str]| ScopeRequest {
        paths: patterns.iter().map(|p| (*p).to_owned()).collect(),
        ..request()
    };
    let changed = |patterns: &[&str]| ScopeRequest {
        changed: true,
        ..paths(patterns)
    };
    let all = tree();
    let every: Vec<&str> = all.iter().map(String::as_str).collect();
    let cases: Vec<(&str, ScopeRequest, Vec<&str>)> = vec![
        ("tree", request(), every.clone()),
        (
            "dir",
            paths(&["src"]),
            vec!["src/a.rs", "src/b.rs", "src/e.rs"],
        ),
        (
            "dir/",
            paths(&["./src/"]),
            vec!["src/a.rs", "src/b.rs", "src/e.rs"],
        ),
        ("file", paths(&["lib/c.rs"]), vec!["lib/c.rs"]),
        ("glob", paths(&["*/[ab].rs"]), vec!["src/a.rs", "src/b.rs"]),
        ("deep glob", paths(&["**/d.rs"]), vec!["lib/d.rs"]),
        (
            "union",
            paths(&["lib/c.rs", "src/b.rs"]),
            vec!["lib/c.rs", "src/b.rs"],
        ),
        ("dot", paths(&["."]), every.clone()),
        (
            "changed",
            changed(&[]),
            vec!["lib/d.rs", "src/a.rs", "src/e.rs"],
        ),
        (
            "changed ∩ paths",
            changed(&["src"]),
            vec!["src/a.rs", "src/e.rs"],
        ),
        ("changed ∩ unchanged path", changed(&["src/b.rs"]), vec![]),
    ];
    for (name, request, expected) in cases {
        assert_eq!(files(&request, &root).unwrap(), expected, "{name}");
    }
}

#[test]
fn base_and_head_select_a_range_and_report_the_base() {
    let Some(repo) = branched() else { return };
    let root = repo.root();
    let base = repo.git(&["rev-parse", "main"]);
    let to_working_tree = ScopeRequest {
        base: Some("main".to_owned()),
        ..request()
    };
    let resolved = to_working_tree.resolve_files(&tree(), &root, None).unwrap();
    assert_eq!(resolved.files, ["lib/d.rs", "src/a.rs", "src/e.rs"]);
    assert_eq!(resolved.base.as_deref(), Some(base.as_str()));

    let between = ScopeRequest {
        base: Some("main".to_owned()),
        head: Some("feature".to_owned()),
        ..request()
    };
    assert_eq!(files(&between, &root).unwrap(), ["lib/d.rs", "src/a.rs"]);
}

#[test]
fn changed_reports_the_merge_base_and_honors_a_configured_default_branch() {
    let Some(repo) = branched() else { return };
    let root = repo.root();
    repo.git(&["branch", "release", "feature"]);
    let changed = ScopeRequest {
        changed: true,
        ..request()
    };
    let against_main = changed.resolve_files(&tree(), &root, None).unwrap();
    assert_eq!(against_main.base, Some(repo.git(&["rev-parse", "main"])));
    let against_release = changed
        .resolve_files(&tree(), &root, Some("release"))
        .unwrap();
    assert_eq!(
        against_release.files,
        ["src/e.rs"],
        "only the untracked file"
    );
}

#[test]
fn a_clean_branch_has_an_empty_changed_scope() {
    let Some(repo) = GitRepo::new() else { return };
    repo.write("src/a.rs", "fn a() {}\n");
    repo.commit("main");
    let changed = ScopeRequest {
        changed: true,
        ..request()
    };
    assert!(files(&changed, &repo.root()).unwrap().is_empty());
}

#[test]
fn ranges_outside_a_repository_and_bad_refs_are_usage_errors() {
    let dir = tempfile::tempdir().unwrap();
    let changed = ScopeRequest {
        changed: true,
        ..request()
    };
    assert_eq!(
        argument(files(&changed, dir.path()).unwrap_err()),
        "--changed"
    );
    let base = ScopeRequest {
        base: Some("main".to_owned()),
        ..request()
    };
    assert_eq!(argument(files(&base, dir.path()).unwrap_err()), "--base");

    let Some(repo) = branched() else { return };
    let unknown = ScopeRequest {
        base: Some("no-such-ref".to_owned()),
        ..request()
    };
    assert_eq!(
        argument(files(&unknown, &repo.root()).unwrap_err()),
        "--base"
    );
    let head_alone = ScopeRequest {
        head: Some("feature".to_owned()),
        ..request()
    };
    assert_eq!(
        argument(files(&head_alone, &repo.root()).unwrap_err()),
        "--head"
    );
    let both = ScopeRequest {
        changed: true,
        base: Some("main".to_owned()),
        ..request()
    };
    assert_eq!(
        argument(files(&both, &repo.root()).unwrap_err()),
        "--changed"
    );
}

#[test]
fn a_path_that_matches_no_file_is_a_usage_error_naming_it() {
    let dir = tempfile::tempdir().unwrap();
    let typo = ScopeRequest {
        paths: vec!["scr".to_owned()],
        ..request()
    };
    let Err(Error::Argument(error)) = files(&typo, dir.path()) else {
        panic!("expected an argument error")
    };
    assert_eq!(error.argument, "--paths");
    assert_eq!(error.value.as_deref(), Some("scr"));
}

#[test]
fn the_mode_prefers_a_range_then_symbols_then_paths() {
    let mode = |request: ScopeRequest| request.mode();
    assert_eq!(mode(request()), ScopeMode::Tree);
    let paths = ScopeRequest {
        paths: vec!["src".to_owned()],
        ..request()
    };
    assert_eq!(mode(paths), ScopeMode::Paths);
    let symbols = ScopeRequest {
        paths: vec!["src".to_owned()],
        symbols: vec!["a".to_owned()],
        ..request()
    };
    assert_eq!(mode(symbols), ScopeMode::Symbols);
    let changed = ScopeRequest {
        changed: true,
        symbols: vec!["a".to_owned()],
        ..request()
    };
    assert_eq!(mode(changed), ScopeMode::Changed);
    let range = ScopeRequest {
        base: Some("main".to_owned()),
        ..request()
    };
    assert_eq!(mode(range), ScopeMode::Range);
}

fn def(display: &str, line: u32) -> SymbolDef {
    let (namespace, name) = display.rsplit_once('.').unwrap_or(("", display));
    SymbolDef {
        name: name.to_owned(),
        namespace: namespace.to_owned(),
        display: display.to_owned(),
        kind: SymbolKind::Function,
        visibility: Visibility::Public,
        lang: LangId::from("go"),
        file: "a.go".to_owned(),
        span: Span {
            line,
            col: 1,
            end_line: line + 1,
            end_col: 2,
        },
        body: None,
        parameters: 0,
    }
}

fn hash(n: u8) -> SymbolHash {
    format!("sh_{n:016x}").parse().unwrap()
}

#[test]
fn symbol_selectors_match_display_names_either_separator_and_hashes() {
    let symbols = Symbols {
        symbols: vec![def("ledger.Account.Deposit", 1), def("ledger.Balance", 5)],
    };
    let norm = NormTree {
        symbols: symbols
            .symbols
            .iter()
            .zip([1, 2])
            .map(|(symbol, n)| NormalizedSymbol {
                symbol: symbol.key(),
                hash: hash(n),
                root: NormNode {
                    span: None,
                    text: None,
                    label: "fn".to_owned(),
                    children: Vec::new(),
                },
            })
            .collect(),
    };
    let select = |selectors: &[&str]| {
        let selectors: Vec<SymbolSelector> = selectors
            .iter()
            .map(|s| SymbolSelector::parse(s).unwrap())
            .collect();
        let (keys, matched) = select_symbols(&selectors, &symbols, Some(&norm));
        let lines: Vec<u32> = keys.iter().map(|key| key.line).collect();
        (lines, matched)
    };
    assert_eq!(select(&["ledger::Account::Deposit"]), (vec![1], vec![true]));
    assert_eq!(select(&["ledger.Balance"]), (vec![5], vec![true]));
    assert_eq!(select(&[hash(2).as_str()]), (vec![5], vec![true]));
    assert_eq!(
        select(&["ledger.Balance", "nope"]),
        (vec![5], vec![true, false])
    );
    assert!(SymbolSelector::parse("sh_123").is_err(), "a malformed hash");
}
