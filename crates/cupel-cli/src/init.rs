// ABOUTME: The `init` verb: detects project roots and writes `cupel.toml` and `.cupel/.gitignore` through
// ABOUTME: a `Writer`; without `--write` the writer only records, so `init` previews (ADR-021, ADR-037).
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use cupel_core::config::FILE_NAME;
use cupel_core::git::Git;
use cupel_core::syntax::{ProjectInfo, ProjectRoot};
use cupel_core::{ActionKind, Exclude, NextAction, Severity, Writer};
use serde_json::json;

use crate::error::{CupelError, Usage};
use crate::problem::Problem;
use crate::project;

/// Writes the configuration for the repository at `writer`'s root, refusing to replace an
/// existing `cupel.toml` unless `force` is set. Returns what the user should know about the
/// repository that the configuration leaves out.
pub fn write(writer: &Writer, force: bool) -> Result<Vec<Problem>, CupelError> {
    if writer.root().join(FILE_NAME).exists() && !force {
        return Err(exists());
    }
    let info = project::detect(writer, &Exclude::default());
    let text = config_text(
        writer.root(),
        &info,
        Git::open(writer.root())
            .and_then(|git| git.default_branch())
            .as_deref(),
    );
    writer
        .write(Path::new(FILE_NAME), text.as_bytes())
        .map_err(|error| unwritable(&error))?;
    writer.gitignore().map_err(|error| unwritable(&error))?;
    Ok(
        cupel_lang::unregistered_roots(writer.root(), &Exclude::default())
            .iter()
            .map(unsupported_root)
            .collect(),
    )
}

/// The `info` finding for a root of a language Cupel recognizes but has not registered, which
/// `cupel.toml` leaves out so that it validates (ADR-038).
fn unsupported_root(root: &ProjectRoot) -> Problem {
    let at = if root.path == "." {
        ".".to_owned()
    } else {
        format!("./{}", root.path)
    };
    Problem {
        check: "init",
        kind: "init.unsupported-root",
        subject: format!("{}:{}", root.lang, root.path),
        severity: Severity::Info,
        title: format!(
            "unsupported language root: {} at {at} — supported in a later release",
            root.lang
        ),
        detail: Some(
            "cupel.toml leaves this root out of [roots] until its language is registered."
                .to_owned(),
        ),
        evidence: json!({
            "lang": root.lang,
            "path": root.path,
            "build_files": root.build_files,
        }),
        action: ActionKind::Review,
        fix: format!(
            "Nothing to do now; {} roots join [roots] once Cupel supports {}.",
            root.lang, root.lang
        ),
    }
}

/// What to run to apply a preview or dry run of `init`.
pub fn apply(force: bool) -> NextAction {
    let command = if force {
        "cupel init --force --write"
    } else {
        "cupel init --write"
    };
    NextAction {
        priority: 1,
        finding: None,
        kind: ActionKind::Configure,
        summary: format!("Nothing was written; run `{command}` to write these files."),
        command: Some(command.to_owned()),
    }
}

/// The `cupel.toml` for `info`, the roots of the repository at `repo`, with each language's
/// commands chosen by the toolchain table for its first root (design §6.7, ADR-021 §4).
pub fn config_text(repo: &Path, info: &ProjectInfo, default_branch: Option<&str>) -> String {
    let mut text = String::from(
        "# Cupel configuration, written by `cupel init`; `cupel describe schema config` documents \
         every key.\n",
    );
    if let Some(branch) = default_branch {
        let _ = writeln!(text, "default_branch = {}", quoted(branch));
    }
    text.push_str("depth = \"standard\"\n");
    let single_root_at_top = matches!(info.roots.as_slice(), [root] if root.path == ".");
    if !info.roots.is_empty() && !single_root_at_top {
        text.push_str("\n[roots]\n");
        for (name, root) in names(&info.roots) {
            let _ = writeln!(
                text,
                "{} = {{ path = {}, lang = {} }}",
                key(&name),
                quoted(&root.path),
                quoted(root.lang.as_str())
            );
        }
    }
    let mut languages: BTreeMap<&str, &ProjectRoot> = BTreeMap::new();
    for root in &info.roots {
        languages.entry(root.lang.as_str()).or_insert(root);
    }
    for (lang, root) in languages {
        let commands = project::commands(repo, root);
        let commands = [
            ("test", &commands.test),
            ("coverage", &commands.coverage),
            ("bench", &commands.bench),
        ];
        if commands.iter().all(|(_, command)| command.is_none()) {
            continue;
        }
        let _ = write!(text, "\n[lang.{lang}]\n");
        for (name, command) in commands {
            if let Some(command) = command {
                let _ = writeln!(text, "{name} = {}", quoted(command));
            }
        }
    }
    text
}

/// Each root's name in `[roots]`: its directory's last component, or its language for `.`,
/// suffixed with the language when two roots would share a name.
fn names(roots: &[ProjectRoot]) -> Vec<(String, &ProjectRoot)> {
    let base = |root: &ProjectRoot| match root.path.rsplit('/').next() {
        Some(".") | None => root.lang.as_str().to_owned(),
        Some(last) => last.to_owned(),
    };
    let mut named: Vec<(String, &ProjectRoot)> = Vec::new();
    for root in roots {
        let mut name = base(root);
        if roots.iter().filter(|other| base(other) == name).count() > 1 {
            name = format!("{name}-{}", root.lang);
        }
        let mut unique = name.clone();
        let mut counter = 2;
        while named.iter().any(|(taken, _)| *taken == unique) {
            unique = format!("{name}-{counter}");
            counter += 1;
        }
        named.push((unique, root));
    }
    named
}

/// `name` as a TOML key: bare when it can be, quoted otherwise.
fn key(name: &str) -> String {
    let bare = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if bare { name.to_owned() } else { quoted(name) }
}

/// `value` as a TOML basic string.
fn quoted(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04X}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The usage error for a `cupel.toml` that exists while `--force` is absent.
fn exists() -> CupelError {
    Usage {
        subject: "--force".to_owned(),
        title: "`cupel.toml` already exists".to_owned(),
        fix: "Pass --force to replace it, or edit it by hand.".to_owned(),
        detail: None,
        argument: Some("--force".to_owned()),
        value: None,
        valid: Vec::new(),
    }
    .into()
}

/// The usage error for a file `init` could not write.
fn unwritable(error: &cupel_core::Error) -> CupelError {
    Usage {
        subject: FILE_NAME.to_owned(),
        title: error.to_string(),
        fix: "Make the repository root and its .cupel/ directory writable.".to_owned(),
        detail: None,
        argument: None,
        value: None,
        valid: Vec::new(),
    }
    .into()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use cupel_core::LangId;
    use cupel_core::config::{Config, ConfigRequest};

    use super::*;

    fn fixture(case: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../cupel-fixtures/repos/detect")
            .join(case)
    }

    /// The registered roots of a detect fixture, through the `ProjectInfo` producers.
    fn detect(case: &str) -> ProjectInfo {
        project::detect(
            &Writer::new(&fixture(case), false).unwrap(),
            &Exclude::default(),
        )
    }

    /// Writes `text` as `cupel.toml` in a fresh directory and loads it.
    fn load(text: &str) -> Config {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(FILE_NAME), text).unwrap();
        Config::load(&ConfigRequest {
            root: dir.path().to_path_buf(),
            ..ConfigRequest::default()
        })
        .unwrap_or_else(|error| panic!("{error}\n{text}"))
    }

    #[test]
    fn every_generated_config_loads_with_its_roots_and_language_commands() {
        for case in [
            "cargo-single",
            "cargo-workspace",
            "go-module",
            "npm-js",
            "pnpm-ts-workspace",
            "python-pyproject",
            "python-requirements",
            "monorepo",
            "maven",
            "empty",
        ] {
            let info = detect(case);
            let config = load(&config_text(&fixture(case), &info, Some("main")));
            assert_eq!(
                config.file.default_branch.as_deref(),
                Some("main"),
                "{case}"
            );
            let roots: Vec<(String, String)> = config
                .file
                .roots
                .values()
                .map(|root| (root.path.clone(), root.lang.as_str().to_owned()))
                .collect();
            if case == "monorepo" {
                assert_eq!(roots.len(), 4, "{case}");
            } else {
                assert!(roots.is_empty(), "{case}: single root at `.`");
            }
            for root in &info.roots {
                let table = config.file.lang.get(&root.lang);
                let test = table.and_then(|table| table.get("test"));
                assert_eq!(
                    test.and_then(|value| value.as_str()),
                    project::commands(&fixture(case), root).test.as_deref(),
                    "{case}: {}",
                    root.lang
                );
            }
        }
    }

    #[test]
    fn the_monorepo_config_has_four_named_roots_and_one_table_per_language() {
        let text = config_text(&fixture("monorepo"), &detect("monorepo"), None);
        assert_eq!(
            text,
            "# Cupel configuration, written by `cupel init`; `cupel describe schema config` \
             documents every key.
depth = \"standard\"

[roots]
engine = { path = \"engine\", lang = \"rust\" }
ml = { path = \"ml\", lang = \"python\" }
svc = { path = \"svc\", lang = \"go\" }
web = { path = \"web\", lang = \"typescript\" }

[lang.go]
test = \"go test -json ./...\"
coverage = \"go test -coverprofile={out} ./...\"
bench = \"go test -run=^$ -bench=. ./...\"

[lang.python]
test = \"poetry run python -m pytest\"
coverage = \"poetry run python -m pytest --cov --cov-report=lcov:{out}\"

[lang.rust]
test = \"cargo test --workspace\"
coverage = \"cargo llvm-cov --lcov --output-path {out}\"

[lang.typescript]
test = \"npx vitest run\"
coverage = \"npx vitest run --coverage --coverage.reporter=lcov\"
"
        );
    }

    fn root(path: &str, lang: &str, packages: &[&str]) -> ProjectRoot {
        ProjectRoot {
            path: path.to_owned(),
            lang: LangId::from(lang),
            build_files: Vec::new(),
            members: Vec::new(),
            packages: packages.iter().map(|p| (*p).to_owned()).collect(),
            skip_dirs: Vec::new(),
        }
    }

    #[test]
    fn colliding_and_unusual_root_names_stay_unique_and_valid() {
        let info = ProjectInfo {
            roots: vec![
                root(".", "javascript", &[]),
                root(".", "rust", &[]),
                root("a/app", "go", &["./..."]),
                root("b/app", "go", &["./b/..."]),
                root("svc.v2", "python", &[]),
            ],
        };
        let repo = tempfile::tempdir().unwrap();
        let text = config_text(repo.path(), &info, Some("we\"ird"));
        let config = load(&text);
        let names: Vec<&str> = config.file.roots.keys().map(String::as_str).collect();
        assert_eq!(
            names,
            ["app-go", "app-go-2", "javascript", "rust", "svc.v2"],
            "{text}"
        );
        assert_eq!(config.file.default_branch.as_deref(), Some("we\"ird"));
        let go = &config.file.lang[&LangId::from("go")];
        assert_eq!(
            go["test"], "go test -json ./...",
            "the first root of a language wins"
        );
    }

    #[test]
    fn a_single_root_below_the_top_is_still_listed_in_roots() {
        let info = ProjectInfo {
            roots: vec![root("web", "javascript", &[])],
        };
        let repo = tempfile::tempdir().unwrap();
        let config = load(&config_text(repo.path(), &info, None));
        assert_eq!(config.file.roots["web"].path, "web");
    }

    #[test]
    fn quoted_escapes_what_toml_basic_strings_cannot_hold() {
        assert_eq!(quoted("a\"b\\c\nd\u{1}"), "\"a\\\"b\\\\c\\nd\\u0001\"");
    }
}
