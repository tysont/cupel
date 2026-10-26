// ABOUTME: Each analyzer `tool.analyzers` knows (ADR-102): its program, its command line for a project root, and
// ABOUTME: the reader that turns its JSON or text output into rule, level, message, and location records.
use std::path::{Path, PathBuf};

use cupel_core::{AnalyzerLevel, Captured};
use serde::Deserialize;
use serde_json::Value;

/// One analyzer: everything that differs between clippy, mypy, `ESLint`, and the rest.
pub struct Spec {
    /// Its ID in the toolchain table, such as `tool.mypy`.
    pub id: &'static str,
    /// Its name in the fact, such as `mypy`.
    pub tool: &'static str,
    /// The program that must be installed.
    pub program: &'static str,
    /// Whether the root's `node_modules/.bin` copy counts.
    pub node_bin: bool,
    /// The command that prints its version.
    pub version: &'static [&'static str],
    /// The languages of the row it applies to; empty for all of them.
    pub langs: &'static [&'static str],
    /// Its command line in a project root; `None` when the root has nothing for it to analyze.
    pub command: fn(&Target<'_>) -> Option<Vec<String>>,
    /// Variables added to its environment.
    pub env: &'static [(&'static str, &'static str)],
    /// Exit codes that mean it analyzed to the end, whether or not it reported anything.
    pub completed_codes: &'static [i32],
    /// Its results, or why its output cannot be read.
    pub read: fn(&Captured) -> Result<Vec<Raw>, String>,
    /// What to add when its detection rules hold in no root.
    pub unconfigured: &'static str,
}

/// A project root an analyzer runs in.
pub struct Target<'a> {
    /// The program to run: on `PATH`, or the root's `node_modules/.bin` copy.
    pub program: String,
    /// The root, absolute and canonical.
    pub dir: &'a Path,
    /// The files in scope under the root, relative to it.
    pub files: Vec<String>,
    /// Package patterns from `ProjectInfo` (`./...`); empty when the ecosystem takes none.
    pub packages: &'a [String],
    /// A directory under `.cupel/cache/` the analyzer may keep its cache in.
    pub cache: PathBuf,
}

/// One result as the tool reported it: its file is absolute, or relative to the root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raw {
    pub rule: String,
    pub level: AnalyzerLevel,
    pub message: String,
    pub file: String,
    pub line: u32,
    pub col: Option<u32>,
    pub end_line: Option<u32>,
    pub end_col: Option<u32>,
}

/// Every analyzer, by toolchain-table ID.
pub static SPECS: [Spec; 7] = [
    Spec {
        id: "tool.clippy",
        tool: "clippy",
        program: "cargo",
        node_bin: false,
        version: &["cargo", "clippy", "--version"],
        langs: &[],
        command: clippy_command,
        env: &[("CARGO_TERM_COLOR", "never")],
        completed_codes: &[0],
        read: clippy,
        unconfigured: "",
    },
    Spec {
        id: "tool.mypy",
        tool: "mypy",
        program: "mypy",
        node_bin: false,
        version: &["mypy", "--version"],
        langs: &[],
        command: mypy_command,
        env: &[("MYPY_FORCE_COLOR", "0")],
        completed_codes: &[0, 1],
        read: mypy,
        unconfigured: "",
    },
    Spec {
        id: "tool.ruff",
        tool: "ruff",
        program: "ruff",
        node_bin: false,
        version: &["ruff", "--version"],
        langs: &[],
        command: ruff_command,
        env: &[],
        completed_codes: &[0, 1],
        read: ruff,
        unconfigured: "",
    },
    Spec {
        id: "tool.tsc",
        tool: "tsc",
        program: "tsc",
        node_bin: true,
        version: &["tsc", "--version"],
        langs: &["typescript"],
        command: tsc_command,
        env: &[],
        completed_codes: &[0, 1, 2],
        read: tsc,
        unconfigured: "",
    },
    Spec {
        id: "tool.eslint",
        tool: "eslint",
        program: "eslint",
        node_bin: true,
        version: &["eslint", "--version"],
        langs: &[],
        command: eslint_command,
        env: &[],
        completed_codes: &[0, 1],
        read: eslint,
        unconfigured: "Add an ESLint flat config (eslint.config.js or .mjs) to the project root; \
                       ESLint does not run without one.",
    },
    Spec {
        id: "tool.go-vet",
        tool: "go-vet",
        program: "go",
        node_bin: false,
        version: &["go", "version"],
        langs: &[],
        command: go_vet_command,
        env: &[],
        completed_codes: &[0],
        read: go_vet,
        unconfigured: "",
    },
    Spec {
        id: "tool.staticcheck",
        tool: "staticcheck",
        program: "staticcheck",
        node_bin: false,
        version: &["staticcheck", "--version"],
        langs: &[],
        command: staticcheck_command,
        env: &[],
        completed_codes: &[0, 1],
        read: staticcheck,
        unconfigured: "",
    },
];

/// `program` followed by `args`, then `files`; `None` when there are no files.
fn on_files(target: &Target<'_>, args: &[&str]) -> Option<Vec<String>> {
    if target.files.is_empty() {
        return None;
    }
    Some(
        std::iter::once(target.program.clone())
            .chain(args.iter().map(|arg| (*arg).to_owned()))
            .chain(target.files.iter().cloned())
            .collect(),
    )
}

/// `program` followed by `args`, then the root's packages (`./...` when it names none), when
/// the root is a Go module.
fn on_packages(target: &Target<'_>, args: &[&str]) -> Option<Vec<String>> {
    if !target.dir.join("go.mod").is_file() {
        return None;
    }
    let packages = if target.packages.is_empty() {
        vec!["./...".to_owned()]
    } else {
        target.packages.to_vec()
    };
    Some(
        std::iter::once(target.program.clone())
            .chain(args.iter().map(|arg| (*arg).to_owned()))
            .chain(packages)
            .collect(),
    )
}

fn clippy_command(target: &Target<'_>) -> Option<Vec<String>> {
    target.dir.join("Cargo.toml").is_file().then(|| {
        [
            &target.program,
            "clippy",
            "--all-targets",
            "--message-format=json",
            "--quiet",
        ]
        .map(str::to_owned)
        .to_vec()
    })
}

/// mypy over the scope's files, less each module with a stub beside it (`more.py` beside
/// `more.pyi`): the stub is that module's types, which every import of it reads, and checking
/// its implementation instead grades code its project types elsewhere (ADR-138 §9).
fn mypy_command(target: &Target<'_>) -> Option<Vec<String>> {
    let cache = format!("--cache-dir={}", target.cache.display());
    let unstubbed = Target {
        program: target.program.clone(),
        dir: target.dir,
        files: target
            .files
            .iter()
            .filter(|file| {
                let path = target.dir.join(file);
                path.extension().is_none_or(|extension| extension != "py")
                    || !path.with_extension("pyi").is_file()
            })
            .cloned()
            .collect(),
        packages: target.packages,
        cache: target.cache.clone(),
    };
    on_files(
        &unstubbed,
        &["--strict", "--output", "json", "--no-error-summary", &cache],
    )
}

fn ruff_command(target: &Target<'_>) -> Option<Vec<String>> {
    on_files(
        target,
        &["check", "--output-format", "json", "--no-cache", "--quiet"],
    )
}

fn tsc_command(target: &Target<'_>) -> Option<Vec<String>> {
    let base = ["--noEmit", "--strict", "--pretty", "false"];
    if target.dir.join("tsconfig.json").is_file() {
        let mut args: Vec<String> = std::iter::once(target.program.clone())
            .chain(base.iter().map(|arg| (*arg).to_owned()))
            .collect();
        args.extend(["-p".to_owned(), ".".to_owned()]);
        Some(args)
    } else {
        on_files(target, &base)
    }
}

fn eslint_command(target: &Target<'_>) -> Option<Vec<String>> {
    on_files(
        target,
        &["--format", "json", "--no-error-on-unmatched-pattern"],
    )
}

fn go_vet_command(target: &Target<'_>) -> Option<Vec<String>> {
    on_packages(target, &["vet", "-json"])
}

fn staticcheck_command(target: &Target<'_>) -> Option<Vec<String>> {
    on_packages(target, &["-f", "json"])
}

// ---------------------------------------------------------------- readers

/// rustc's level as an analyzer level; `None` for notes and help attached to another message.
fn rustc_level(level: &str) -> Option<AnalyzerLevel> {
    match level {
        "error" => Some(AnalyzerLevel::Error),
        "warning" => Some(AnalyzerLevel::Warning),
        _ => None,
    }
}

/// `cargo clippy --message-format=json`: one JSON message per line; compiler messages with a
/// code, at their primary span. The rule drops clippy's `clippy::` prefix.
fn clippy(captured: &Captured) -> Result<Vec<Raw>, String> {
    let mut raws = Vec::new();
    for line in captured.stdout.lines().filter(|line| line.starts_with('{')) {
        let message: Value = serde_json::from_str(line)
            .map_err(|error| format!("unreadable clippy JSON: {error}"))?;
        if message["reason"] != "compiler-message" {
            continue;
        }
        let diagnostic = &message["message"];
        let (Some(code), Some(level)) = (
            diagnostic["code"]["code"].as_str(),
            diagnostic["level"].as_str().and_then(rustc_level),
        ) else {
            continue;
        };
        let Some(span) = diagnostic["spans"]
            .as_array()
            .and_then(|spans| spans.iter().find(|span| span["is_primary"] == true))
        else {
            continue;
        };
        raws.push(Raw {
            rule: code.strip_prefix("clippy::").unwrap_or(code).to_owned(),
            level,
            message: diagnostic["message"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            file: span["file_name"].as_str().unwrap_or_default().to_owned(),
            line: number(&span["line_start"]),
            col: Some(number(&span["column_start"])),
            end_line: Some(number(&span["line_end"])),
            end_col: Some(number(&span["column_end"])),
        });
    }
    Ok(raws)
}

fn number(value: &Value) -> u32 {
    value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or_default()
}

#[derive(Deserialize)]
struct MypyLine {
    file: String,
    line: i64,
    column: i64,
    end_line: Option<i64>,
    end_column: Option<i64>,
    message: String,
    code: Option<String>,
    severity: String,
}

/// mypy's `--output json`: one JSON object per line, with 0-based columns; notes attached to an
/// error are kept as notes.
fn mypy(captured: &Captured) -> Result<Vec<Raw>, String> {
    let mut raws = Vec::new();
    for line in captured.stdout.lines().filter(|line| line.starts_with('{')) {
        let found: MypyLine =
            serde_json::from_str(line).map_err(|error| format!("unreadable mypy JSON: {error}"))?;
        let level = match found.severity.as_str() {
            "error" => AnalyzerLevel::Error,
            "note" => AnalyzerLevel::Note,
            _ => AnalyzerLevel::Warning,
        };
        raws.push(Raw {
            rule: found.code.unwrap_or_else(|| "misc".to_owned()),
            level,
            message: found.message,
            file: found.file,
            line: positive(found.line),
            col: Some(positive(found.column + 1)),
            end_line: found.end_line.map(positive),
            end_col: found.end_column.map(|column| positive(column + 1)),
        });
    }
    Ok(raws)
}

/// `value` as a 1-based number; at least 1.
fn positive(value: i64) -> u32 {
    u32::try_from(value.max(1)).unwrap_or(u32::MAX)
}

#[derive(Deserialize)]
struct RuffPoint {
    row: u32,
    column: u32,
}

#[derive(Deserialize)]
struct RuffViolation {
    code: Option<String>,
    message: String,
    filename: String,
    location: RuffPoint,
    end_location: RuffPoint,
}

/// `ruff check --output-format json`: one array of violations; a linter's, so warnings.
fn ruff(captured: &Captured) -> Result<Vec<Raw>, String> {
    let text = captured.stdout.trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let violations: Vec<RuffViolation> =
        serde_json::from_str(text).map_err(|error| format!("unreadable ruff JSON: {error}"))?;
    Ok(violations
        .into_iter()
        .map(|violation| Raw {
            rule: violation.code.unwrap_or_else(|| "syntax-error".to_owned()),
            level: AnalyzerLevel::Warning,
            message: violation.message,
            file: violation.filename,
            line: violation.location.row,
            col: Some(violation.location.column),
            end_line: Some(violation.end_location.row),
            end_col: Some(violation.end_location.column),
        })
        .collect())
}

/// `tsc --pretty false`: `file(line,col): error TS1234: message` lines; continuation lines of a
/// message are dropped. An error with no file (`error TS5023: Unknown compiler option`) means the
/// project could not be checked.
fn tsc(captured: &Captured) -> Result<Vec<Raw>, String> {
    let mut raws = Vec::new();
    for line in captured.stdout.lines() {
        if line.starts_with("error TS") {
            return Err(format!("tsc could not check the project: {line}"));
        }
        raws.extend(tsc_line(line));
    }
    Ok(raws)
}

/// One `file(line,col): severity code: message` line; `None` for any other line.
fn tsc_line(line: &str) -> Option<Raw> {
    let (place, rest) = line.split_once("): ")?;
    let (file, position) = place.rsplit_once('(')?;
    let (line_number, col) = position.split_once(',')?;
    let (severity, rest) = rest.split_once(' ')?;
    let (code, message) = rest.split_once(": ")?;
    let level = match severity {
        "error" => AnalyzerLevel::Error,
        "warning" => AnalyzerLevel::Warning,
        _ => AnalyzerLevel::Note,
    };
    Some(Raw {
        rule: code.to_owned(),
        level,
        message: message.to_owned(),
        file: file.to_owned(),
        line: line_number.parse().ok()?,
        col: Some(col.parse().ok()?),
        end_line: None,
        end_col: None,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EslintFile {
    file_path: String,
    messages: Vec<EslintMessage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EslintMessage {
    rule_id: Option<String>,
    severity: u8,
    message: String,
    line: Option<u32>,
    column: Option<u32>,
    end_line: Option<u32>,
    end_column: Option<u32>,
}

/// `eslint --format json`: one array of files with their messages; severity 2 is an error, 1 a
/// warning; a message without a rule (a parse error) is rule `parse-error`.
fn eslint(captured: &Captured) -> Result<Vec<Raw>, String> {
    let text = captured.stdout.trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let files: Vec<EslintFile> =
        serde_json::from_str(text).map_err(|error| format!("unreadable ESLint JSON: {error}"))?;
    Ok(files
        .into_iter()
        .flat_map(|file| {
            let path = file.file_path;
            file.messages.into_iter().map(move |message| Raw {
                rule: message.rule_id.unwrap_or_else(|| "parse-error".to_owned()),
                level: if message.severity >= 2 {
                    AnalyzerLevel::Error
                } else {
                    AnalyzerLevel::Warning
                },
                message: message.message,
                file: path.clone(),
                line: message.line.unwrap_or(1),
                col: message.column,
                end_line: message.end_line,
                end_col: message.end_column,
            })
        })
        .collect())
}

/// `go vet -json`: on standard error, `# package` lines and one JSON object per package of
/// `{package: {analyzer: [{posn: "file:line:col", message}]}}`; vet's findings are warnings.
fn go_vet(captured: &Captured) -> Result<Vec<Raw>, String> {
    let json: String = captured
        .stderr
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    let mut raws = Vec::new();
    for object in serde_json::Deserializer::from_str(&json).into_iter::<Value>() {
        let object = object.map_err(|error| format!("unreadable go vet JSON: {error}"))?;
        for analyzers in object.as_object().into_iter().flat_map(|o| o.values()) {
            for (analyzer, found) in analyzers.as_object().into_iter().flatten() {
                for diagnostic in found.as_array().into_iter().flatten() {
                    let posn = diagnostic["posn"].as_str().unwrap_or_default();
                    let (file, line, col) = position(posn);
                    raws.push(Raw {
                        rule: analyzer.clone(),
                        level: AnalyzerLevel::Warning,
                        message: diagnostic["message"]
                            .as_str()
                            .unwrap_or_default()
                            .to_owned(),
                        file,
                        line,
                        col,
                        end_line: None,
                        end_col: None,
                    });
                }
            }
        }
    }
    Ok(raws)
}

/// `file:line:col` (or `file:line`) as its parts.
fn position(posn: &str) -> (String, u32, Option<u32>) {
    if let Some((rest, col)) = posn.rsplit_once(':')
        && let Some((file, line)) = rest.rsplit_once(':')
        && let (Ok(line), Ok(col)) = (line.parse(), col.parse())
    {
        return (file.to_owned(), line, Some(col));
    }
    if let Some((file, line)) = posn.rsplit_once(':')
        && let Ok(line) = line.parse()
    {
        return (file.to_owned(), line, None);
    }
    (posn.to_owned(), 1, None)
}

#[derive(Deserialize)]
struct StaticcheckPoint {
    file: String,
    line: u32,
    column: u32,
}

#[derive(Deserialize)]
struct StaticcheckLine {
    code: String,
    severity: String,
    location: StaticcheckPoint,
    end: StaticcheckPoint,
    message: String,
}

/// `staticcheck -f json`: one JSON object per line; a `compile` problem has no file and is not
/// a result.
fn staticcheck(captured: &Captured) -> Result<Vec<Raw>, String> {
    let mut raws = Vec::new();
    for line in captured.stdout.lines().filter(|line| line.starts_with('{')) {
        let found: StaticcheckLine = serde_json::from_str(line)
            .map_err(|error| format!("unreadable staticcheck JSON: {error}"))?;
        if found.location.file.is_empty() {
            return Err(format!("staticcheck could not analyze: {}", found.message));
        }
        let level = match found.severity.as_str() {
            "error" => AnalyzerLevel::Error,
            "warning" => AnalyzerLevel::Warning,
            _ => AnalyzerLevel::Note,
        };
        raws.push(Raw {
            rule: found.code,
            level,
            message: found.message,
            file: found.location.file,
            line: found.location.line,
            col: Some(found.location.column),
            end_line: (found.end.line > 0).then_some(found.end.line),
            end_col: (found.end.column > 0).then_some(found.end.column),
        });
    }
    Ok(raws)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn captured(stdout: &str, stderr: &str) -> Captured {
        Captured {
            exit_code: Some(1),
            stdout: stdout.to_owned(),
            stderr: stderr.to_owned(),
        }
    }

    fn raw(rule: &str, level: AnalyzerLevel, message: &str, file: &str, at: [u32; 4]) -> Raw {
        Raw {
            rule: rule.to_owned(),
            level,
            message: message.to_owned(),
            file: file.to_owned(),
            line: at[0],
            col: Some(at[1]),
            end_line: Some(at[2]),
            end_col: Some(at[3]),
        }
    }

    #[test]
    fn clippy_keeps_coded_compiler_messages_at_their_primary_span() {
        let stdout = concat!(
            r#"{"reason":"compiler-artifact","target":{}}"#,
            "\n",
            r#"{"reason":"compiler-message","message":{"code":{"code":"clippy::needless_return"},"level":"warning","message":"unneeded `return` statement","spans":[{"file_name":"src/other.rs","is_primary":false,"line_start":9,"line_end":9,"column_start":1,"column_end":2},{"file_name":"src/lib.rs","is_primary":true,"line_start":2,"line_end":2,"column_start":5,"column_end":17}]}}"#,
            "\n",
            r#"{"reason":"compiler-message","message":{"code":null,"level":"warning","message":"1 warning emitted","spans":[]}}"#,
            "\n",
            r#"{"reason":"build-finished","success":true}"#,
        );
        assert_eq!(
            clippy(&captured(stdout, "")).unwrap(),
            [raw(
                "needless_return",
                AnalyzerLevel::Warning,
                "unneeded `return` statement",
                "src/lib.rs",
                [2, 5, 2, 17]
            )]
        );
    }

    #[test]
    fn mypy_checks_a_module_with_a_stub_through_the_stub() {
        let dir = tempfile::tempdir().unwrap();
        for file in ["typed.py", "typed.pyi", "plain.py"] {
            std::fs::write(dir.path().join(file), "").unwrap();
        }
        let target = Target {
            program: "mypy".to_owned(),
            dir: dir.path(),
            files: vec!["typed.py".to_owned(), "plain.py".to_owned()],
            packages: &[],
            cache: dir.path().join("cache"),
        };
        let args = mypy_command(&target).unwrap();
        assert_eq!(args.last().map(String::as_str), Some("plain.py"));
        assert!(!args.iter().any(|arg| arg == "typed.py"), "{args:?}");
        let stubbed = Target {
            files: vec!["typed.py".to_owned()],
            ..target
        };
        assert_eq!(mypy_command(&stubbed), None);
    }

    #[test]
    fn mypy_columns_become_one_based() {
        let stdout = r#"{"file": "ledger.py", "line": 9, "column": 18, "end_line": 9, "end_column": 21, "message": "Argument 2 has incompatible type", "hint": null, "code": "arg-type", "severity": "error"}"#;
        assert_eq!(
            mypy(&captured(stdout, "")).unwrap(),
            [raw(
                "arg-type",
                AnalyzerLevel::Error,
                "Argument 2 has incompatible type",
                "ledger.py",
                [9, 19, 9, 22]
            )]
        );
    }

    #[test]
    fn ruff_reports_warnings_from_its_array() {
        let stdout = r#"[{"code": "F401", "message": "`os` imported but unused", "filename": "/r/ledger.py", "location": {"row": 1, "column": 8}, "end_location": {"row": 1, "column": 10}, "fix": null}]"#;
        assert_eq!(
            ruff(&captured(stdout, "")).unwrap(),
            [raw(
                "F401",
                AnalyzerLevel::Warning,
                "`os` imported but unused",
                "/r/ledger.py",
                [1, 8, 1, 10]
            )]
        );
        assert_eq!(ruff(&captured("", "")).unwrap(), []);
        assert!(ruff(&captured("not json", "")).is_err());
    }

    #[test]
    fn tsc_reads_its_one_line_diagnostics() {
        let stdout = "ledger.ts(5,37): error TS2345: Argument of type 'string' is not assignable.\n  Continuation.\nsrc/a b.ts(1,2): error TS1005: ';' expected.\n";
        let found = tsc(&captured(stdout, "")).unwrap();
        assert_eq!(
            found
                .iter()
                .map(|raw| (raw.rule.as_str(), raw.file.as_str(), raw.line, raw.col))
                .collect::<Vec<_>>(),
            [
                ("TS2345", "ledger.ts", 5, Some(37)),
                ("TS1005", "src/a b.ts", 1, Some(2))
            ]
        );
        assert_eq!(
            found[0].message,
            "Argument of type 'string' is not assignable."
        );
        let unchecked = "error TS5023: Unknown compiler option 'strictest'.";
        assert!(tsc(&captured(unchecked, "")).is_err());
    }

    #[test]
    fn eslint_maps_severities_and_parse_errors() {
        let stdout = r#"[{"filePath":"/r/app.js","messages":[{"ruleId":"eqeqeq","severity":2,"message":"Expected '==='.","line":1,"column":23,"endLine":1,"endColumn":25},{"ruleId":null,"fatal":true,"severity":2,"message":"Parsing error","line":3,"column":1},{"ruleId":"no-console","severity":1,"message":"Unexpected console.","line":4,"column":1,"endLine":4,"endColumn":12}]}]"#;
        let found = eslint(&captured(stdout, "")).unwrap();
        assert_eq!(
            found
                .iter()
                .map(|raw| (raw.rule.as_str(), raw.level, raw.line))
                .collect::<Vec<_>>(),
            [
                ("eqeqeq", AnalyzerLevel::Error, 1),
                ("parse-error", AnalyzerLevel::Error, 3),
                ("no-console", AnalyzerLevel::Warning, 4)
            ]
        );
    }

    #[test]
    fn go_vet_reads_every_package_object_from_standard_error() {
        let stderr = "# example.com/a\n{\n\t\"example.com/a\": {\n\t\t\"printf\": [\n\t\t\t{\"posn\": \"/r/a.go:6:9\", \"message\": \"bad format\"}\n\t\t]\n\t}\n}\n# example.com/b\n{\n\t\"example.com/b\": {\n\t\t\"unreachable\": [{\"posn\": \"/r/b/b.go:3:2\", \"message\": \"unreachable code\"}]\n\t}\n}\n";
        let found = go_vet(&captured("", stderr)).unwrap();
        assert_eq!(
            found
                .iter()
                .map(|raw| (raw.rule.as_str(), raw.file.as_str(), raw.line, raw.col))
                .collect::<Vec<_>>(),
            [
                ("printf", "/r/a.go", 6, Some(9)),
                ("unreachable", "/r/b/b.go", 3, Some(2))
            ]
        );
        assert_eq!(go_vet(&captured("", "")).unwrap(), []);
    }

    #[test]
    fn positions_split_from_the_right() {
        assert_eq!(position("/r/a.go:6:9"), ("/r/a.go".to_owned(), 6, Some(9)));
        assert_eq!(position("a.go:6"), ("a.go".to_owned(), 6, None));
        assert_eq!(
            position("c:/r/a.go:6:9"),
            ("c:/r/a.go".to_owned(), 6, Some(9))
        );
    }

    #[test]
    fn staticcheck_refuses_a_compile_problem() {
        let found = r#"{"code":"SA5009","severity":"error","location":{"file":"/r/l.go","line":6,"column":21},"end":{"file":"/r/l.go","line":6,"column":28},"message":"Printf format %s reads arg #2"}"#;
        assert_eq!(
            staticcheck(&captured(found, "")).unwrap(),
            [raw(
                "SA5009",
                AnalyzerLevel::Error,
                "Printf format %s reads arg #2",
                "/r/l.go",
                [6, 21, 6, 28]
            )]
        );
        let compile = r#"{"code":"compile","severity":"error","location":{"file":"","line":0,"column":0},"end":{"file":"","line":0,"column":0},"message":"declared and not used: x"}"#;
        assert!(staticcheck(&captured(compile, "")).is_err());
    }
}
