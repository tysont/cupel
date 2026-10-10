// ABOUTME: The `init` verb: detects the project and writes `cupel.toml` and `.cupel/.gitignore` through
// ABOUTME: a `Writer`; without `--write` the writer only records, so `init` previews (ADR-021).
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use cupel_core::config::FILE_NAME;
use cupel_core::syntax::{ProjectInfo, ProjectRoot};
use cupel_core::{ActionKind, NextAction, Writer};

use crate::error::{CupelError, Usage};
use crate::{detect, git};

/// Writes the configuration for the repository at `writer`'s root, refusing to replace an
/// existing `cupel.toml` unless `force` is set.
pub fn write(writer: &Writer, force: bool) -> Result<(), CupelError> {
    if writer.root().join(FILE_NAME).exists() && !force {
        return Err(exists());
    }
    let info = detect::project(writer.root());
    let text = config_text(&info, git::default_branch(writer.root()).as_deref());
    writer
        .write(Path::new(FILE_NAME), text.as_bytes())
        .map_err(|error| unwritable(&error))?;
    writer.gitignore().map_err(|error| unwritable(&error))?;
    Ok(())
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

/// The `cupel.toml` for `info` (design §6.7, ADR-021 §4).
pub fn config_text(info: &ProjectInfo, default_branch: Option<&str>) -> String {
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
        let commands = [
            ("test", &root.commands.test),
            ("coverage", &root.commands.coverage),
            ("bench", &root.commands.bench),
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
    use cupel_core::syntax::DefaultCommands;

    use super::*;

    fn fixture(case: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../cupel-fixtures/repos/detect")
            .join(case)
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
            let info = detect::project(&fixture(case));
            let config = load(&config_text(&info, Some("main")));
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
                    root.commands.test.as_deref(),
                    "{case}: {}",
                    root.lang
                );
            }
        }
    }

    #[test]
    fn the_monorepo_config_has_four_named_roots_and_one_table_per_language() {
        let text = config_text(&detect::project(&fixture("monorepo")), None);
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

    fn root(path: &str, lang: &str, test: Option<&str>) -> ProjectRoot {
        ProjectRoot {
            path: path.to_owned(),
            lang: LangId::from(lang),
            build_files: Vec::new(),
            commands: DefaultCommands {
                test: test.map(str::to_owned),
                coverage: None,
                bench: None,
            },
            skip_dirs: Vec::new(),
        }
    }

    #[test]
    fn colliding_and_unusual_root_names_stay_unique_and_valid() {
        let info = ProjectInfo {
            roots: vec![
                root(".", "javascript", Some("node --test")),
                root(".", "rust", Some("cargo test --workspace")),
                root("a/app", "go", Some("go test ./...")),
                root("b/app", "go", Some("go test ./b/...")),
                root("svc.v2", "java", None),
            ],
        };
        let text = config_text(&info, Some("we\"ird"));
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
            go["test"], "go test ./...",
            "the first root of a language wins"
        );
        assert!(!config.file.lang.contains_key(&LangId::from("java")));
    }

    #[test]
    fn a_single_root_below_the_top_is_still_listed_in_roots() {
        let info = ProjectInfo {
            roots: vec![root("web", "javascript", Some("node --test"))],
        };
        let config = load(&config_text(&info, None));
        assert_eq!(config.file.roots["web"].path, "web");
    }

    #[test]
    fn quoted_escapes_what_toml_basic_strings_cannot_hold() {
        assert_eq!(quoted("a\"b\\c\nd\u{1}"), "\"a\\\"b\\\\c\\nd\\u0001\"");
    }
}
